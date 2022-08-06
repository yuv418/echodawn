#include "../inc/edssInterface.h"
#include "../inc/edssCALInterface.h"
#include "../inc/edssCapture.h"
#include "../inc/edssInterfaceInternal.h"

#include <ck_ring.h>
#include <dlfcn.h>
#include <fcntl.h>
#include <libavcodec/avcodec.h>
#include <libavformat/avformat.h>
#include <libavutil/imgutils.h>
#include <libavutil/opt.h>
#include <libswscale/swscale.h>
#include <pthread.h>
#include <semaphore.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/mman.h>
#include <unistd.h>

static fbEncoderCtx_t fbEncoderCtx;
static captureCtx_t captureCtx;
static calConfig_t calCfg;
static pthread_t captureTh;
static calPlugin_t *calPlugin;

// Encoder variables
static AVCodecContext *cdcCtx;
static AVPacket *encPkt;
static AVStream *avS;
static AVFormatContext *fmtCtx;

EDSS_STATUS edssInterfaceSetupSwscale(edssConfig_t *edssCfg) {

    // avpicture_alloc(vgpuFbEncoderCtx->picToEncode, AV_PIX_FMT_YUV420P,
    // IMGBUF_WIDTH, IMGBUF_HEIGHT);
    fbEncoderCtx.picToEncode = av_frame_alloc();

    fbEncoderCtx.picToEncode->format = AV_PIX_FMT_YUV420P;
    fbEncoderCtx.picToEncode->width = calCfg.width;
    fbEncoderCtx.picToEncode->height = calCfg.height;

    int ret;
    ret = av_image_alloc(
        fbEncoderCtx.picToEncode->data, fbEncoderCtx.picToEncode->linesize,
        fbEncoderCtx.picToEncode->width, fbEncoderCtx.picToEncode->height,
        fbEncoderCtx.picToEncode->format, 1);
    if (ret < 0) {
        return EDSS_LIBAV_FAILURE;
    }

    fbEncoderCtx.swsCtx =
        sws_getContext(calCfg.width, calCfg.height,
                       calCfg.pixFmt, // input width and height
                       calCfg.width, calCfg.height,
                       AV_PIX_FMT_YUV420P, // output width and height
                       SWS_BICUBIC, NULL, NULL, NULL);

    return EDSS_OK;
}

int edssCaptureInit() {
    int ret;

    ret = sem_init(&captureCtx.bufferSem, 0,
                   0); // semaphore starts empty (3rd parameter)
    if (ret != 0) {
        perror("sem_init");
        return EDSS_ALLOCATION_FAILURE;
    }
    // TODO how to check whether this worked?
    ck_ring_init(&captureCtx.frameRing, 2); // TODO don't hardcode
    captureCtx.encodingFinished = false;

    return EDSS_OK;
}

int fbBgraToYuv() {
    int stride[1] = {calCfg.width * 4}; // for alpha channel
    // printf("beginning sws_scale\n");
    // printf("ending sws_scale\n");
    return sws_scale(fbEncoderCtx.swsCtx, (const uint8_t *const *)&calCfg.frame,
                     stride, 0, calCfg.height, fbEncoderCtx.picToEncode->data,
                     fbEncoderCtx.picToEncode->linesize);
}

// NOTE This should be called before anything else
EDSS_STATUS edssOpenCAL(char calPluginName[100], StrMap *calOptionDict) {
    // Setup CAL. edssCfg does not have to contain anything other than the CAL
    // plugin name.

    int ret;
    void *calHandle;

    calHandle = dlopen(calPluginName, RTLD_LAZY);
    if (!calHandle) {
        return EDSS_INVALID_CAL;
    }

    calPlugin = dlsym(calHandle, "calPlugin");
    if (!calPlugin) {
        return EDSS_INVALID_CAL;
    }

    if ((ret = calPlugin->calOptions(calOptionDict)) != EDSS_OK) {
        return ret;
    }
    return EDSS_OK;
}

EDSS_STATUS edssInitServer(edssConfig_t *edssCfg) {
    /*
     * SETUP SETCTION
     * -----------------------------------------------------------------------------------------------
     */

    // Copy options to already-initialized CAL. TODO check to make sure that CAL
    // has already been initialised.
    int ret;
    if ((ret = calPlugin->calInit(edssCfg->calOptionDict, &calCfg)) !=
        EDSS_OK) {
        return ret;
    }

    /*
     * ENCODER SETUP SETCTION
     * -----------------------------------------------------------------------------------------------
     */

    const AVCodec *cdc;

    const AVOutputFormat *rtpFmt;
    char rtpAddress[28]; // maximum ip:port length
    snprintf(rtpAddress, sizeof(rtpAddress), "srtp://%s:%d/",
             inet_ntoa(edssCfg->ip), edssCfg->port);

    rtpFmt = av_guess_format("rtp", NULL, NULL);
    if (!rtpFmt) {
        fprintf(stderr, "Failed to guess format srtp");
        return EDSS_LIBAV_FAILURE;
    }
    avformat_alloc_output_context2(&fmtCtx, rtpFmt, rtpFmt->name, rtpAddress);
    if (!fmtCtx) {
        fprintf(stderr, "Failed to allocate AVFormatContext\n");
        return EDSS_LIBAV_FAILURE;
    }
    av_dump_format(fmtCtx, 0, rtpAddress, 1);

    avS = avformat_new_stream(fmtCtx, NULL);
    if (!avS) {
        fprintf(stderr, "Failed to allocate AVStream\n");
        return EDSS_LIBAV_FAILURE;
    }

    cdc = avcodec_find_encoder_by_name(
        "libx264"); // So, how do we switch to hardware encoding?
    if (!cdc) {
        fprintf(stderr, "Failed to find AvCodec for encoding h264\n");
        return EDSS_LIBAV_FAILURE;
    }
    cdcCtx = avcodec_alloc_context3(cdc);
    if (cdcCtx < 0) {
        fprintf(stderr, "Failed to allocate AVCodecContext\n");
        return EDSS_LIBAV_FAILURE;
    }
    encPkt = av_packet_alloc();
    if (encPkt < 0) {
        fprintf(stderr, "Failed to allocate AVPacket\n");
        return EDSS_LIBAV_FAILURE;
    }

    cdcCtx->gop_size = 60; // TODO optimize these values.
    cdcCtx->max_b_frames = 0;
    cdcCtx->height = calCfg.height;
    cdcCtx->width = calCfg.width;
    cdcCtx->pix_fmt = AV_PIX_FMT_YUV420P; // TODO do we want to be able to
                                          // change this to YUV444 at runtime?
    cdcCtx->bit_rate = 10000000; // TODO this must be adjusted based on the
                                 // quality of the network.
    cdcCtx->framerate =
        (AVRational){calCfg.framerate, 1}; // start with 60fps for now I guess
    cdcCtx->time_base = (AVRational){
        1,
        calCfg
            .framerate}; // time_base = 1/framerate,
                         // https://stackoverflow.com/questions/12234949/ffmpeg-time-unit-explanation-and-av-seek-frame-method?
    av_opt_set(cdcCtx->priv_data, "preset", "ultrafast", 0);
    av_opt_set(cdcCtx->priv_data, "tune", "zerolatency", 0);

    ret = avcodec_open2(cdcCtx, cdc, NULL);
    if (ret < 0) {
        fprintf(stderr, "Failed to open the codec\n");
        return EDSS_LIBAV_FAILURE;
    }

    /*
     * END ENCODER SETUP SETCTION
     * -----------------------------------------------------------------------------------------------
     */

    // Initialize converter from BRGA to YUV and the YUV image buffer

    // Open the vGPU device and map the framebuffer to a pointer we can access.
    // The framebuffer continually changes as the user interacts with the
    // screen.

    ret = edssCaptureInit();
    if (ret < 0) {
        // The error message was already printed. (again again)
        return 1;
    }

    /*
     * END SETUP SETCTION
     * --------------------------------------------------------------------------------------------
     */

    /*
     * BEGIN ENCODING SECTION
     * ----------------------------------------------------------------------------------------
     */

    AVDictionary *opts;
    opts = NULL;

    // Set SRTP options. TODO verify these calls succeeded.
    av_dict_set(&opts, "srtp_out_suite", "AES_CM_128_HMAC_SHA1_80", 0);
    av_dict_set(&opts, "srtp_out_params", edssCfg->srtpOutParams, 0);

    avS->time_base = cdcCtx->time_base;
    ret = avcodec_parameters_from_context(avS->codecpar, cdcCtx);
    if (ret < 0) {
        fprintf(stderr,
                "Failed to copy AVCodecParameters into AVStream struct\n");
        return EDSS_LIBAV_FAILURE;
    }

    ret = avio_open2(&fmtCtx->pb, rtpAddress, AVIO_FLAG_WRITE, NULL, &opts);
    if (ret < 0) {
        fprintf(stderr, "Failed to open the output file for writing: %s\n",
                av_err2str(ret));
        return EDSS_LIBAV_FAILURE;
    }

    ret = avformat_write_header(fmtCtx,
                                NULL); // The muxer(?) options don't have to be
                                       // passed here, only to avio_open2
    if (ret < 0) {
        fprintf(stderr, "Failed to write header to output file\n");
        return EDSS_LIBAV_FAILURE;
    }

    // thanks stackoverflow
    // TODO figure out how SDP will get sent to the client
    char buf[20000];
    AVFormatContext *ac[] = {fmtCtx};
    av_sdp_create(ac, 1, buf, 20000);
    printf("sdp:\n%s\n", buf); // dump SDP for now

    // start the capture thread
    return EDSS_OK;
}

EDSS_STATUS edssInitStreaming() {

    struct captureThreadArgs ctArgs;
    int totalFramesEncoded;
    int ret;

    ctArgs.captureCtx = &captureCtx;
    ctArgs.fbEncoderCtx = &fbEncoderCtx;
    ctArgs.calPlugin = calPlugin;

    if (pthread_create(&captureTh, NULL, &edssCaptureThreadFunction,
                       (void *)&ctArgs) != 0) {
        perror("pthread_create");
        return EDSS_PTHREAD_FAILURE;
    }

    fbEncoderCtx.picToEncode->pts = 0;
    totalFramesEncoded = 0;
    captureData_t *copiedFbPointer;

    while (true) {
        // Wait for new frames. not a binary semaphore so it'll keep encoding
        // till we have no more frames to encode.

        sem_wait(&captureCtx.bufferSem);
        // TODO the max queue size is 2. Make sure it is actually 2.
        if (ck_ring_dequeue_spmc(
                &captureCtx.frameRing,
                (struct ck_ring_buffer *)&captureCtx.frameRingBuffer,
                &copiedFbPointer)) {

            pthread_mutex_lock(&copiedFbPointer->mutex);
            fbBgraToYuv();
            pthread_mutex_unlock(&copiedFbPointer->mutex);

            ret = avcodec_send_frame(cdcCtx, fbEncoderCtx.picToEncode);

            if (ret < 0) {
                fprintf(stderr, "Failed to send AVFrame to encoder\n");
                return EDSS_ENCODE_FAILURE;
            }

            while (ret >= 0) {
                ret = avcodec_receive_packet(cdcCtx, encPkt);
                // printf("encoding packet with return %d\n", ret);
                if (ret == AVERROR(EAGAIN) || ret == AVERROR_EOF) {
                    // fprintf(stderr, "EAGAIN or EOF from
                    // avcodec_receive_packet\n");
                    break;
                } else if (ret < 0) {
                    fprintf(stderr, "Failed to receive AVPacket\n");
                    return EDSS_ENCODE_FAILURE;
                }
                // encPkt->duration = avS->time_base.den / avS->time_base.num /
                // 60 * 1;
                av_packet_rescale_ts(encPkt, (AVRational){1, calCfg.framerate},
                                     avS->time_base);
                av_interleaved_write_frame(fmtCtx, encPkt);
                av_packet_unref(encPkt);
            }
            fbEncoderCtx.picToEncode->pts++;
            totalFramesEncoded++; // TODO why not just use pts?

        } else {
            printf("dequeue failed, continuing!!\n"); // FIX THIS!!
        }
    }
}

EDSS_STATUS edssCloseStreaming() {

    int ret;

    captureCtx.encodingFinished = true;
    ret = pthread_join(captureTh, NULL);
    if (ret != 0) {
        perror("pthread_join");
        return EDSS_PTHREAD_FAILURE;
    }

    av_write_trailer(fmtCtx);
    avio_closep(&fmtCtx->pb);

    /*
     * END ENCODING SECTION
     * -------------------------------------------------------------------------------------------
     */

    return EDSS_OK;
}

/// Not implemented for now
EDSS_STATUS edssUpdateStreaming() { return EDSS_OK; }
#include "../inc/edssInterface.h"
#include "../inc/edssCALInterface.h"
#include "../inc/edssCapture.h"
#include "../inc/edssInterfaceInternal.h"

#include "../inc/edssLog.h"

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

static fbEncoderCtx_t *fbEncoderCtx;
static captureCtx_t captureCtx;
static calConfig_t *calCfg;
static pthread_t captureTh;
static pthread_t streamTh;
static calPlugin_t *calPlugin;

// Encoder variables
static AVCodecContext *cdcCtx;
static AVPacket *encPkt;
static AVStream *avS;
static AVFormatContext *fmtCtx;
AVDictionary *opts;
char rtpAddress[28]; // maximum ip:port length

#define SDP_BUFLEN 3000

EDSS_STATUS
edssInterfaceSetupSwscale(edssConfig_t *edssCfg) {

    // avpicture_alloc(vgpuFbEncoderCtx->picToEncode, AV_PIX_FMT_YUV420P,
    // IMGBUF_WIDTH, IMGBUF_HEIGHT);
    EDSS_LOGD("Begin edssInterfaceSetupSwscale\n");
    fbEncoderCtx = malloc(sizeof(fbEncoderCtx_t));
    fbEncoderCtx->picToEncode = av_frame_alloc();

    fbEncoderCtx->picToEncode->format = AV_PIX_FMT_YUV420P;
    fbEncoderCtx->picToEncode->width = calCfg->width;
    fbEncoderCtx->picToEncode->height = calCfg->height;

    int ret;
    ret = av_image_alloc(
        fbEncoderCtx->picToEncode->data, fbEncoderCtx->picToEncode->linesize,
        fbEncoderCtx->picToEncode->width, fbEncoderCtx->picToEncode->height,
        fbEncoderCtx->picToEncode->format, 1);
    if (ret < 0) {
        return EDSS_LIBAV_FAILURE;
    }

    fbEncoderCtx->swsCtx = sws_getContext(
        calCfg->width, calCfg->height, calCfg->pixFmt, // input width and height
        calCfg->width, calCfg->height,
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
    captureCtx.encodingFinished = false;

    return EDSS_OK;
}

// NOTE This should be called before anything else
EDSS_STATUS edssOpenCAL(char calPluginName[100], StrMap **calOptionDict) {
    // Setup CAL. edssCfg does not have to contain anything other than the CAL
    // plugin name.

    int ret;
    void *calHandle;
    EDSS_LOGD("edssOpenCAL called with calPluginName %s\n", calPluginName);

    calHandle = dlopen(calPluginName, RTLD_LAZY);
    if (!calHandle) {
        // TODO macro these printfs
        EDSS_LOGE("edssOpenCAL dlopen failed, invalid handle\n");
        return EDSS_INVALID_CAL;
    }

    calPlugin = dlsym(calHandle, "calPlugin");
    if (!calPlugin) {
        EDSS_LOGE("edssOpenCAL dlsym failed. Check that the plugin exports the "
                  "calPlugin structure.\n");
        return EDSS_INVALID_CAL;
    }

    if ((ret = calPlugin->calOptions(calOptionDict)) != EDSS_OK) {
        EDSS_LOGE("edssOpenCAL failed to retreive calOptions\n");
        return ret;
    }
    return EDSS_OK;
}

EDSS_STATUS edssInitServer(edssConfig_t *edssCfg, char **sdpBuffer) {
    /*
     * SETUP SETCTION
     * -----------------------------------------------------------------------------------------------
     */

    EDSS_LOGD("Initialising server\n");

    // Copy options to already-initialized CAL. TODO check to make sure that CAL
    // has already been initialised.
    int ret;
    calCfg = malloc(sizeof(calConfig_t));
    if ((ret = calPlugin->calInit(edssCfg->calOptionDict, calCfg)) != EDSS_OK) {
        return ret;
    }

    /*
     * ENCODER SETUP SETCTION
     * -----------------------------------------------------------------------------------------------
     */

    const AVCodec *cdc;

    const AVOutputFormat *rtpFmt;
    snprintf(rtpAddress, sizeof(rtpAddress), "srtp://%s:%d/",
             inet_ntoa((struct in_addr){.s_addr = edssCfg->ip}), edssCfg->port);

    rtpFmt = av_guess_format("rtp", NULL, NULL);
    if (!rtpFmt) {
        EDSS_LOGE("Failed to guess format srtp\n");
        return EDSS_LIBAV_FAILURE;
    }
    avformat_alloc_output_context2(&fmtCtx, rtpFmt, rtpFmt->name, rtpAddress);
    if (!fmtCtx) {
        EDSS_LOGE("Failed to allocate AVFormatContext\n");
        return EDSS_LIBAV_FAILURE;
    }
    av_dump_format(fmtCtx, 0, rtpAddress, 1);

    avS = avformat_new_stream(fmtCtx, NULL);
    if (!avS) {
        EDSS_LOGE("Failed to allocate AVStream\n");
        return EDSS_LIBAV_FAILURE;
    }

    cdc = avcodec_find_encoder_by_name(
        "libx264"); // So, how do we switch to hardware encoding?
    if (!cdc) {
        fprintf(stderr, "Failed to find AvCodec for encoding h264\n");
        return EDSS_LIBAV_FAILURE;
    }
    cdcCtx = avcodec_alloc_context3(cdc);
    EDSS_LOGD("cdcCtx is %p\n", cdcCtx);
    if (cdcCtx < 0) {
        EDSS_LOGE("Failed to allocate AVCodecContext\n");
        return EDSS_LIBAV_FAILURE;
    }
    encPkt = av_packet_alloc();
    if (encPkt < 0) {
        EDSS_LOGE("Failed to allocate AVPacket\n");
        return EDSS_LIBAV_FAILURE;
    }

    ret = edssInterfaceSetupSwscale(edssCfg);
    if (ret > 0) {
        EDSS_LOGE("Failed to set up AVFrame for swscale\n");
        return EDSS_LIBAV_FAILURE;
    }

    cdcCtx->gop_size = 60; // TODO optimize these values.
    cdcCtx->max_b_frames = 0;
    cdcCtx->height = calCfg->height;
    cdcCtx->width = calCfg->width;
    cdcCtx->pix_fmt = AV_PIX_FMT_YUV420P; // TODO do we want to be able to
                                          // change this to YUV444 at runtime?
    cdcCtx->bit_rate = edssCfg->bitrate;  // TODO this must be adjusted based on
                                          // the quality of the network.
    cdcCtx->framerate =
        (AVRational){calCfg->framerate, 1}; // start with 60fps for now I guess
    cdcCtx->time_base = (AVRational){
        1,
        calCfg
            ->framerate}; // time_base = 1/framerate,
                          // https://stackoverflow.com/questions/12234949/ffmpeg-time-unit-explanation-and-av-seek-frame-method?
    av_opt_set(cdcCtx->priv_data, "preset", "ultrafast", 0);
    av_opt_set(cdcCtx->priv_data, "tune", "zerolatency", 0);

    ret = avcodec_open2(cdcCtx, cdc, NULL);
    if (ret < 0) {
        EDSS_LOGE("Failed to open the codec\n");
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
    if (ret > 0) {
        // The error message was already printed. (again again)
        return ret;
    }

    /*
     * END SETUP SETCTION
     * --------------------------------------------------------------------------------------------
     */

    /*
     * BEGIN ENCODING SECTION
     * ----------------------------------------------------------------------------------------
     */

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
        EDSS_LOGE("Failed to open the output file for writing: %s\n",
                  av_err2str(ret));
        return EDSS_LIBAV_FAILURE;
    }

    // thanks stackoverflow
    *sdpBuffer = malloc(SDP_BUFLEN);
    AVFormatContext *ac[] = {fmtCtx};
    av_sdp_create(ac, 1, *sdpBuffer, SDP_BUFLEN);

    return EDSS_OK;
}

int fbBgraToYuv(calConfig_t *calCfg, fbEncoderCtx_t *fbEncoderCtx) {
    int stride[1] = {calCfg->width * 4}; // for alpha channel
    // printf("beginning sws_scale\n");
    // printf("ending sws_scale\n");
    return sws_scale(fbEncoderCtx->swsCtx,
                     (const uint8_t *const *)&calCfg->frame, stride, 0,
                     calCfg->height, fbEncoderCtx->picToEncode->data,
                     fbEncoderCtx->picToEncode->linesize);
}

// For some reason, if you put this function in another file and create
// another set of thread parameters, the _second_ thread to get initialised
// will have a corrupted set of thread parameters, causing random segfaults.
// TODO: Move this function to another file. HOW?
void *edssStreamThreadFunction(void *threadArgs) {

    int ret;
    int totalFramesEncoded;
    captureData_t *copiedFbPointer;

    ret = avformat_write_header(fmtCtx,
                                NULL); // The muxer(?) options don't have to be
                                       // passed here, only to avio_open2
    if (ret < 0) {
        EDSS_LOGE("Failed to write header to output file\n");
        return (void *)EDSS_LIBAV_FAILURE;
    }

    fbEncoderCtx->picToEncode->pts = 0;
    totalFramesEncoded = 0;

    EDSS_LOGI("Stream thread main loop starting\n");
    while (true) {
        // Wait for new frames. not a binary semaphore so it'll keep encoding
        // till we have no more frames to encode.

        // TODO put this in a place where a deadlock doesn't happen

        sem_wait(&captureCtx.bufferSem);

        if (captureCtx.encodingFinished) {
            EDSS_LOGW("streaming finished\n");
            break;
        }

        // TODO the max queue size is 2. Make sure it is actually 2.
        if (ck_ring_dequeue_spmc(
                &captureCtx.frameRing,
                (struct ck_ring_buffer *)&captureCtx.frameRingBuffer,
                &copiedFbPointer)) {

            pthread_mutex_lock(&copiedFbPointer->mutex);
            fbBgraToYuv(calCfg, fbEncoderCtx);
            pthread_mutex_unlock(&copiedFbPointer->mutex);

            ret = avcodec_send_frame(cdcCtx, fbEncoderCtx->picToEncode);

            if (ret < 0) {
                EDSS_LOGE("Failed to send AVFrame to encoder\n");
                return (void *)EDSS_ENCODE_FAILURE;
            }

            while (ret >= 0) {
                ret = avcodec_receive_packet(cdcCtx, encPkt);
                // printf("encoding packet with return %d\n", ret);
                if (ret == AVERROR(EAGAIN) || ret == AVERROR_EOF) {
                    // fprintf(stderr, "EAGAIN or EOF from
                    // avcodec_receive_packet\n");
                    break;
                } else if (ret < 0) {
                    EDSS_LOGE("Failed to receive AVPacket\n");
                    return (void *)EDSS_ENCODE_FAILURE;
                }
                // encPkt->duration = avS->time_base.den / avS->time_base.num /
                // 60 * 1;
                av_packet_rescale_ts(encPkt, (AVRational){1, calCfg->framerate},
                                     avS->time_base);
                av_interleaved_write_frame(fmtCtx, encPkt);
                av_packet_unref(encPkt);
            }
            fbEncoderCtx->picToEncode->pts++;
            totalFramesEncoded++; // TODO why not just use pts?

        } else {
            EDSS_LOGW("dequeue failed, continuing!!\n"); // FIX THIS!!
        }
    }

    EDSS_LOGW("ENCODER THREAD EXIT\n");
    return (void *)EDSS_OK;
}

EDSS_STATUS edssInitStreaming() {

    struct captureThreadArgs ctArgs;

    ck_ring_init(&captureCtx.frameRing, 2);

    EDSS_LOGD("here\n");
    ctArgs.calPlugin = calPlugin;
    ctArgs.fbEncoderCtx = fbEncoderCtx;
    ctArgs.captureCtx = &captureCtx;
    ctArgs.calCfg = calCfg;

    if (pthread_create(&captureTh, NULL, &edssCaptureThreadFunction, &ctArgs) !=
        0) {
        EDSS_LOGE("capture thread pthread_create failed %s", strerror(errno));
        return EDSS_PTHREAD_FAILURE;
    }
    if (pthread_create(&streamTh, NULL, &edssStreamThreadFunction, NULL) != 0) {
        EDSS_LOGE("stream thread pthread_create failed %s", strerror(errno));
        return EDSS_PTHREAD_FAILURE;
    }

    return EDSS_OK;
}

EDSS_STATUS edssCloseStreaming() {

    int ret;

    captureCtx.encodingFinished = true;
    ret = pthread_join(captureTh, NULL);
    if (ret != 0) {
        EDSS_LOGE("capture thread pthread_join failed %s", strerror(errno));
        return EDSS_PTHREAD_FAILURE;
    }
    ret = pthread_join(streamTh, NULL);
    if (ret != 0) {
        EDSS_LOGE("stream thread pthread_join failed %s", strerror(errno));
        return EDSS_PTHREAD_FAILURE;
    }

    av_write_trailer(fmtCtx);
    avio_closep(&fmtCtx->pb);

    /*
     * END ENCODING SECTION
     * -------------------------------------------------------------------------------------------
     */

    // Free all the variables
    free(calCfg);
    free(fbEncoderCtx);
    EDSS_LOGD("here\n");
    calPlugin = NULL; // This is a pointer to a static variable in a shared
                      // library, so you can't really free it IIUC
    free(calPlugin);
    av_free(opts);
    av_free(fmtCtx);
    av_free(avS);
    av_free(cdcCtx);
    av_free(encPkt);

    return EDSS_OK;
}

/// Not implemented for now
EDSS_STATUS edssUpdateStreaming(edssConfig_t *cfg) { return EDSS_OK; }

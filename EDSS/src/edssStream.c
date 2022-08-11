#include "../inc/edssStream.h"
#include "../inc/edssCapture.h"
#include "../inc/edssLog.h"
#include "../inc/edssStatus.h"
#include <ck_ring.h>
#include <dlfcn.h>
#include <fcntl.h>
#include <libavcodec/avcodec.h>
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

int fbBgraToYuv(calConfig_t *calCfg, fbEncoderCtx_t *fbEncoderCtx) {
    int stride[1] = {calCfg->width * 4}; // for alpha channel
    // printf("beginning sws_scale\n");
    // printf("ending sws_scale\n");
    return sws_scale(fbEncoderCtx->swsCtx,
                     (const uint8_t *const *)&calCfg->frame, stride, 0,
                     calCfg->height, fbEncoderCtx->picToEncode->data,
                     fbEncoderCtx->picToEncode->linesize);
}

void *edssStreamThreadFunction(void *threadArgs) {
    int ret;
    int totalFramesEncoded;
    struct streamThreadArgs *args;
    captureCtx_t *captureCtx;
    fbEncoderCtx_t *fbEncoderCtx;
    AVCodecContext *cdcCtx;
    calConfig_t *calCfg;
    AVPacket *encPkt;
    AVFormatContext *fmtCtx;
    AVStream *avS;

    args = (struct streamThreadArgs *)threadArgs;
    fbEncoderCtx = args->fbEncoderCtx;
    captureCtx = args->captureCtx;
    cdcCtx = args->cdcCtx;
    calCfg = args->calCfg;
    encPkt = args->encPkt;
    avS = args->avS;
    fmtCtx = args->fmtCtx;

    fbEncoderCtx->picToEncode->pts = 0;
    totalFramesEncoded = 0;
    captureData_t *copiedFbPointer;

    ck_ring_init(&captureCtx->frameRing, 2); // TODO don't hardcode

    while (true) {
        // Wait for new frames. not a binary semaphore so it'll keep encoding
        // till we have no more frames to encode.

        sem_wait(&captureCtx->bufferSem);
        // TODO the max queue size is 2. Make sure it is actually 2.
        if (ck_ring_dequeue_spmc(
                &captureCtx->frameRing,
                (struct ck_ring_buffer *)&captureCtx->frameRingBuffer,
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

    return (void *)EDSS_OK;
}

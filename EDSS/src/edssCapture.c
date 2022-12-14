#include "../inc/edssCapture.h"
#include "../inc/edssCALInterface.h"
#include "../inc/edssInterfaceInternal.h"
#include "../inc/edssLog.h"

#include <stdio.h>

// Copy "back buffer" (framebuffer) to "front buffer" based on the frame rate of
// the captured video.
void *edssCaptureThreadFunction(void *threadArgs) {
    struct captureThreadArgs *args = (struct captureThreadArgs *)threadArgs;
    captureCtx_t *captureCtx = args->captureCtx;
    calConfig_t *calCfg = args->calCfg;
    calPlugin_t *calPlugin = args->calPlugin;

    int dataLen;

    // NOTE this 4 is hacky. It's just because BGRA is what was default in
    // testing. What if it's not this in the appropriate plugin. We have to
    // figure out the pixel size based on the pixel format provided in
    // calCfg.
    dataLen = calCfg->width * calCfg->height * 4;
    captureData_t data1 = {
        .mutex = PTHREAD_MUTEX_INITIALIZER,
        .buffer = malloc(dataLen),
    };

    captureData_t data2 = {
        .mutex = PTHREAD_MUTEX_INITIALIZER,
        .buffer = malloc(dataLen),
    };

    EDSS_LOGI("Capture thread main loop starting\n");
    while (true) {

        captureData_t *data;
        int ret;
        // .... Spinning is always a great idea. Here we choose
        // which data buffer to use.
        while (true) {
            if ((ret = pthread_mutex_trylock(&data1.mutex)) == 0) {
                data = &data1;
                break;
            } else if (pthread_mutex_trylock(&data2.mutex)) {
                data = &data2;
                break;
            }
        }

        // TODO Add something better(?) to exit the thread.
        if (captureCtx->encodingFinished) {
            EDSS_LOGW("encoding finished\n");
            break;
        }

        // Add the framebuffer data to the queue after retreiving and copying it
        if ((ret = calPlugin->calReadFrame()) != EDSS_OK) {
            fprintf(stderr, "EDSS_STATUS error in edssCapture: %d", ret);
        }
        memcpy(data->buffer, calCfg->frame, dataLen);
        pthread_mutex_unlock(&data->mutex);

        if (!ck_ring_enqueue_spmc(
                &captureCtx->frameRing,
                (struct ck_ring_buffer *)&captureCtx->frameRingBuffer, data)) {
            EDSS_LOGW("ck_ring_enqueue_spmc: %s\n", strerror(errno));
            EDSS_LOGW("Capture enqueue frame failed!!\n");
        }

        // tell the consumer that we have more data
        /*		int sval;
                        sem_getvalue(&captureCtx->bufferSem, &sval);*/
        sem_post(&captureCtx->bufferSem);
        nanosleep((const struct timespec[]){{0, 16600000L}},
                  NULL); // wait for next frame
    }
    // Post the semaphore so the streaming thread can also exit, otherwise it
    // will deadlock waiting for the semaphore.
    sem_post(&captureCtx->bufferSem);
    EDSS_LOGW("CAPTURE THREAD EXIT\n");
    return EDSS_OK;
}

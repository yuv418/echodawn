#include "edssCALInterface.h"

#include <ck_ring.h>
#include <semaphore.h>

typedef struct {
    sem_t bufferSem; // We use this sem to control whether the producer/consumer
                     // runs
    ck_ring_t frameRing;
    ck_ring_buffer_t frameRingBuffer[2]; // max 128 frame pointers stored in the
                                         // buffer for now
    bool encodingFinished;
} captureCtx_t;

typedef struct {
    struct SwsContext *swsCtx;
    AVFrame *picToEncode; // allocated once, used more than once...
} fbEncoderCtx_t;

struct captureThreadArgs {
    captureCtx_t *captureCtx;
    fbEncoderCtx_t *fbEncoderCtx;
    calPlugin_t *calPlugin;
    calConfig_t *calCfg;
};

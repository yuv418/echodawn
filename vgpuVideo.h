#pragma once

#include <sys/mman.h>
#include <stdio.h>
#include <stdlib.h>
#include <fcntl.h>
#include <string.h>
#include <unistd.h>
#include <stdint.h>
#include <semaphore.h>
#include <pthread.h>
#include <libswscale/swscale.h>
#include <libavcodec/avcodec.h>
#include <libavutil/imgutils.h>
#include <libavutil/opt.h>
#include <libavformat/avformat.h>
#include <ck_ring.h>

#define IMGBUF_WIDTH  1920
#define IMGBUF_HEIGHT 1080
#define IMGBUF_FRAMERATE 60
#define MMAP_CONSOLE_OFFSET 0x10000000000

#define AV_LOG_TRACE   56

typedef struct {
        uint8_t vgpuFd;
} vgpuFbHandle_t; // handle with fds that we use to get the framebuffer BGRA pointer

typedef struct {
        struct SwsContext *swsCtx;
        AVFrame *picToEncode; // allocated once, used more than once...
        uint8_t *fbPointer;
} vgpuFbEncoderCtx_t;

typedef struct {
	sem_t bufferSem; // We use this sem to control whether the producer/consumer runs
	ck_ring_t frameRing;
	ck_ring_buffer_t frameRingBuffer[2]; // max 128 frame pointers stored in the buffer for now
	bool encodingFinished;
} captureCtx_t;

struct captureThreadArgs {
 	captureCtx_t* captureCtx;
	vgpuFbEncoderCtx_t* vgpuFbEncoderCtx;
	int* randomValue;
};

#include "vgpuVideo.h"

#define DATA_LEN IMGBUF_WIDTH * IMGBUF_HEIGHT * 4  // x4 for RGBA 

// Copy "back buffer" (framebuffer) to "front buffer" based on the frame rate of the captured video.
void captureThreadFunction(void *threadArgs) {
	struct captureThreadArgs* args = (struct captureThreadArgs*) threadArgs; 
	captureCtx_t* captureCtx = args->captureCtx;
	vgpuFbEncoderCtx_t* vgpuFbEncoderCtx = args->vgpuFbEncoderCtx;
	/*int* randVal = args->randomValue;
	printf("randVal is %d", *randVal);*/

	int dataLen;
	while (true) {
		// TODO add something to exit the thread
		
		if (captureCtx->encodingFinished) {
			break;
		}

		// add the framebuffer data to the queue after copying it 
		void* fbData;
		fbData = malloc(DATA_LEN);
		memcpy(fbData, vgpuFbEncoderCtx->fbPointer, DATA_LEN); 

		if (!ck_ring_enqueue_spmc(&captureCtx->frameRing, &captureCtx->frameRingBuffer, fbData)) {
			perror("ck_ring_enqueue_spmc");
			printf("[capture thread] Capture enqueue frame failed!!\n");
		}

		// tell the consumer that we have more data
		sem_post(&captureCtx->bufferSem);
		nanosleep((const struct timespec[]){{0, 13000000L}}, NULL); // wait for next frame
	}
	printf("CAPTURE THREAD EXIT\n");
}

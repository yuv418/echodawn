#include "vgpuVideo.h"
#include "capture.h"

#define DATA_LEN IMGBUF_WIDTH * IMGBUF_HEIGHT * 4  // x4 for RGBA 

// Copy "back buffer" (framebuffer) to "front buffer" based on the frame rate of the captured video.
void captureThreadFunction(void *threadArgs) {
	struct captureThreadArgs* args = (struct captureThreadArgs*) threadArgs; 
	captureCtx_t* captureCtx = args->captureCtx;
	vgpuFbEncoderCtx_t* vgpuFbEncoderCtx = args->vgpuFbEncoderCtx;
	/*int* randVal = args->randomValue;
	printf("randVal is %d", *randVal);*/

	int dataLen;
	// two buffers
	// just use a loop smh
	captureData_t data1 = {
		.mutex = PTHREAD_MUTEX_INITIALIZER,
		.buffer = malloc(DATA_LEN),
	};
	captureData_t data2 = {
		.mutex = PTHREAD_MUTEX_INITIALIZER,
		.buffer = malloc(DATA_LEN),
	};



	while (true) {
		// TODO add something to exit the thread
		
		captureData_t* data;
		int a; 
		// spinning is always a great idea.
		while (true) {
			if ((a = pthread_mutex_trylock(&data1.mutex)) == 0) {
				data = &data1;
				break;
			}
			else if (pthread_mutex_trylock(&data2.mutex)) {
				data = &data2; 
				break;
			}
		}
		/*switch (bufferChoice) {
			case 1:
				fbData = fbData1;
				break;
			case 2:
				fbData = fbData2;
				break;
			case 3:
				fbData = fbData1;
				break;
			case 4:
				fbData = fbData2;
				break;
		}*/

		if (captureCtx->encodingFinished) {
			break;
		}



		// add the framebuffer data to the queue after copying it 
		memcpy(data->buffer, vgpuFbEncoderCtx->fbPointer, DATA_LEN); 
		pthread_mutex_unlock(&data->mutex);

		if (!ck_ring_enqueue_spmc(&captureCtx->frameRing, &captureCtx->frameRingBuffer, data)) {
			perror("ck_ring_enqueue_spmc");
			printf("[capture thread] Capture enqueue frame failed!!\n");
		}

		// tell the consumer that we have more data
/*		int sval;
		sem_getvalue(&captureCtx->bufferSem, &sval);*/
		sem_post(&captureCtx->bufferSem);
		nanosleep((const struct timespec[]){{0, 16600000L}}, NULL); // wait for next frame
	}
	printf("CAPTURE THREAD EXIT\n");
}

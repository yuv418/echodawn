#include "vgpuVideo.h"
#include "capture.h"

int vgpuFbInit(vgpuFbHandle_t* vgpuFbHandle, uint32_t vgpuId) {
        char gid_path[32]; // QEMU uses this as the max size for a vfio device, so.. we will too
                           // Why do we care about QEMU though?
        int vgpuFd;


        snprintf(gid_path, sizeof(gid_path), "/dev/nvidia-vgpu%d", vgpuId);

        vgpuFd = open(gid_path, O_RDWR);
        if (vgpuFd < 0) {
                perror("open /dev/nvidia-vgpu*");
                return -1;
        }

        vgpuFbHandle->vgpuFd = vgpuFd;

        return 0;

}

int vgpuEncoderCtxInit(vgpuFbEncoderCtx_t* vgpuFbEncoderCtx, vgpuFbHandle_t *vgpuFbHandle) {

        vgpuFbEncoderCtx->fbPointer = (uint8_t*) mmap(
                        0,
                        IMGBUF_WIDTH * IMGBUF_HEIGHT * 4,
                        PROT_READ, MAP_PRIVATE,
                        vgpuFbHandle->vgpuFd, MMAP_CONSOLE_OFFSET);

        if (vgpuFbEncoderCtx->fbPointer < 0) {
                return -1;
        }

        // avpicture_alloc(vgpuFbEncoderCtx->picToEncode, AV_PIX_FMT_YUV420P, IMGBUF_WIDTH, IMGBUF_HEIGHT);
        vgpuFbEncoderCtx->picToEncode = av_frame_alloc();

        vgpuFbEncoderCtx->picToEncode->format = AV_PIX_FMT_YUV420P;
        vgpuFbEncoderCtx->picToEncode->width = IMGBUF_WIDTH;
        vgpuFbEncoderCtx->picToEncode->height = IMGBUF_HEIGHT;

        int ret;
        ret = av_image_alloc(vgpuFbEncoderCtx->picToEncode->data,
                        vgpuFbEncoderCtx->picToEncode->linesize,
                        vgpuFbEncoderCtx->picToEncode->width,
                        vgpuFbEncoderCtx->picToEncode->height,
                        vgpuFbEncoderCtx->picToEncode->format,
                        1);
        if (ret < 0) {
                return -1;
        }

        printf("stride %d\n", vgpuFbEncoderCtx->picToEncode->linesize[3]);

        vgpuFbEncoderCtx->swsCtx = sws_getContext(IMGBUF_WIDTH,
                        IMGBUF_HEIGHT,
                        AV_PIX_FMT_BGRA,  // input width and height
                        IMGBUF_WIDTH,
                        IMGBUF_HEIGHT,
                        AV_PIX_FMT_YUV420P,  // output width and height
                        SWS_BICUBIC, NULL, NULL, NULL);


        return 0;
}

int captureCtxInit(captureCtx_t* ctx) {
	int ret;

	ret = sem_init(&ctx->bufferSem, 0, 0); // semaphore starts empty (3rd parameter)
	if (ret != 0) {
		perror("sem_init");
		return -1;
	}
	// TODO how to check whether this worked?
	ck_ring_init(&ctx->frameRing, 128); // TODO don't hardcode

	ctx->encodingFinished = false;
	return 0;
}

int vgpuFbBgraToYuv(vgpuFbEncoderCtx_t* ctx, uint8_t* fbPointer) {
        int stride[1] = { IMGBUF_WIDTH * 4 }; // for alpha channel
        // printf("beginning sws_scale\n");
	printf("ending sws_scale\n");
        return sws_scale(ctx->swsCtx, (const uint8_t* const*) &fbPointer, stride, 0,
                        IMGBUF_HEIGHT, ctx->picToEncode->data, ctx->picToEncode->linesize);

}

// https://stackoverflow.com/questions/35797904/writing-decoded-yuv420p-data-into-a-file-with-ffmpeg
// literally only for testing something
void saveAvFrame(AVFrame *avFrame, char* filename)
{
    FILE *fDump = fopen(filename, "ab");

    uint32_t pitchY = avFrame->linesize[0];
    uint32_t pitchU = avFrame->linesize[1];
    uint32_t pitchV = avFrame->linesize[2];

    uint8_t *avY = avFrame->data[0];
    uint8_t *avU = avFrame->data[1];
    uint8_t *avV = avFrame->data[2];

    for (uint32_t i = 0; i < avFrame->height; i++) {
        fwrite(avY, avFrame->width, 1, fDump);
        avY += pitchY;
    }

    for (uint32_t i = 0; i < avFrame->height/2; i++) {
        fwrite(avU, avFrame->width/2, 1, fDump);
        avU += pitchU;
    }

    for (uint32_t i = 0; i < avFrame->height/2; i++) {
        fwrite(avV, avFrame->width/2, 1, fDump);
        avV += pitchV;
    }

    fclose(fDump);
}

int main(int argc, char **argv) {
	if (argv[1] == NULL) {
		fprintf(stderr, "usage: ./program [vfio_group_id]\n");
		return 1;
	}
	/*
	 * SETUP SETCTION ----------------------------------------------------------------------------------------------- 
	 */

	/*
	 * ENCODER SETUP SETCTION ----------------------------------------------------------------------------------------------- 
	 */

	const AVCodec *cdc;
	AVFormatContext *fmtCtx;
	AVStream *avS;
	AVCodecContext *cdcCtx;
	AVCodecParameters *cdcPms;
	AVPacket *encPkt;
	int ret;
	int numStreams;
	int totalFramesEncoded;
	pthread_t captureTh;

	numStreams = 1;

	avformat_alloc_output_context2(&fmtCtx, NULL, NULL, "random.mp4");
	if (!fmtCtx) {
		fprintf(stderr, "Failed to allocate AVFormatContext\n");
		return -1;
	}
	av_dump_format(fmtCtx, 0, "random.mp4", 1);

	avS = avformat_new_stream(fmtCtx, NULL);
	if (!avS) {
		fprintf(stderr, "Failed to allocate AVStream\n");
		return -1;
	}


	cdc = avcodec_find_encoder_by_name("libx264"); // So, how do we switch to hardware encoding?
	if (!cdc) {
		fprintf(stderr, "Failed to find AvCodec for encoding h264\n");
		return -1;
	}
	cdcCtx = avcodec_alloc_context3(cdc);	
	if (cdcCtx < 0) {
		fprintf(stderr, "Failed to allocate AVCodecContext\n");
		return -1;
	}
	encPkt = av_packet_alloc();
	if (encPkt < 0) {
		fprintf(stderr, "Failed to allocate AVPacket\n");
		return -1;
	}

	cdcCtx->gop_size = 10; // TODO optimize these values.
	cdcCtx->max_b_frames = 1;
	cdcCtx->height = IMGBUF_HEIGHT;
	cdcCtx->width = IMGBUF_WIDTH;
	cdcCtx->pix_fmt = AV_PIX_FMT_YUV420P; // TODO do we want to be able to change this to YUV444 at runtime?
	cdcCtx->bit_rate = 10000000; // TODO this must be adjusted based on the quality of the network.
	cdcCtx->framerate = (AVRational){IMGBUF_FRAMERATE, 1}; // start with 60fps for now I guess
	cdcCtx->time_base = (AVRational){1, IMGBUF_FRAMERATE}; // time_base = 1/framerate, https://stackoverflow.com/questions/12234949/ffmpeg-time-unit-explanation-and-av-seek-frame-method?
	av_opt_set(cdcCtx->priv_data, "preset", "ultrafast", 0);

	ret = avcodec_open2(cdcCtx, cdc, NULL);
	if (ret < 0) {
		fprintf(stderr, "Failed to open the codec\n");
		return -1;
	}
	
	/*
	 * END ENCODER SETUP SETCTION ----------------------------------------------------------------------------------------------- 
	 */


	// Initialize converter from BRGA to YUV and the YUV image buffer                                                 
	

	// Open the vGPU device and map the framebuffer to a pointer we can access.                                       
	// The framebuffer continually changes as the user interacts with the screen.

	int vgpuId;
	vgpuId = atoi(argv[1]); // we don't know if zero is the actual groupid number or atoi failing
	if (vgpuId == 0) {
		fprintf(stderr, "warning, groupid is zero, which means that you could have provided an invalid groupid\n");
	}

	// This does NOT initialize the framebuffer. That is done by nvidia-vfio-vgpu and QEMU.

	vgpuFbHandle_t vgpuFbHandle; 
	ret = vgpuFbInit(&vgpuFbHandle, vgpuId);
	if (ret < 0) {
		// The error message was already printed.
		return 1;
	}

	// We now call mmap to map the framebuffer pointer, and initialize the encoder and pixel format converter
	vgpuFbEncoderCtx_t vgpuFbEncoderCtx;
	ret = vgpuEncoderCtxInit(&vgpuFbEncoderCtx, &vgpuFbHandle);
	if (ret < 0) {
		// The error message was already printed. (again)
		return 1;
	}

	captureCtx_t captureCtx; 
	ret = captureCtxInit(&captureCtx);
	if (ret < 0) {
		// The error message was already printed. (again again)
		return 1;
	}

	struct captureThreadArgs ctArgs;
	int randVal = 4;
	ctArgs.captureCtx = &captureCtx;
	ctArgs.vgpuFbEncoderCtx = &vgpuFbEncoderCtx;
	ctArgs.randomValue = &randVal;



	/*
	 * END SETUP SETCTION --------------------------------------------------------------------------------------------
	 */

	/*
	 * BEGIN ENCODING SECTION ----------------------------------------------------------------------------------------
	 */


	/*for (int i = 0; i < 1080; ++i) { // each row
		for (int j = 0; j < 1; ++j) { // each pix per roww
			 printf("writing pixel (x, y) -> (%d, %d)\n", j, i);
			pixel* pix = (pixel*) vgpuFbEncoderCtx.fbPointer[(i * 1920) + j];
			printf("pix (%d, %d, %d, %d)\n", pix->r, pix->g, pix->g, pix->a);
		}
	}*/

	// saveAvFrame(vgpuFbEncoderCtx.picToEncode, "test.yuv");
	
	/*FILE* fd;
	fd = fopen("encodedVideo.mp4", "wb");*/
	 
	AVDictionary* opts;
	opts = NULL;


	avS->time_base = cdcCtx->time_base;
	ret = avcodec_parameters_from_context(avS->codecpar, cdcCtx);
	if (ret < 0) {
		fprintf(stderr, "Failed to copy AVCodecParameters into AVStream struct\n");
		return 1;
	}
			    
	ret = avio_open(&fmtCtx->pb, "random.mp4", AVIO_FLAG_WRITE);
	if (ret < 0) {
		fprintf(stderr, "Failed to open the output file for writing\n");
		return 1;
	}

	ret = avformat_write_header(fmtCtx, &opts); 
	if (ret < 0) {
		fprintf(stderr, "Failed to write header to output file\n");
		return 1;
	}

	// start the capture thread
	
	if (pthread_create(&captureTh, NULL, &captureThreadFunction, (void*)&ctArgs) != 0) {
		perror("pthread_create");
		return 1;
	}

	vgpuFbEncoderCtx.picToEncode->pts = 0;
	totalFramesEncoded = 0;
	uint8_t* copiedFbPointer;

	while (true) {
		// 10 seconds of video 
		if (totalFramesEncoded == 600) { // we could just kill the thread haha
			// the other thread is never writing so no mutex is fine i think
			captureCtx.encodingFinished = true;
			break;
		}

		// wait for new frames. not a binary semaphore so it'll keep encoding
		// till we have no more frames to encode.



		sem_wait(&captureCtx.bufferSem);
		// TODO the max queue size is 2. Make sure it is actually 2
		if (ck_ring_dequeue_spmc(&captureCtx.frameRing, &captureCtx.frameRingBuffer, &copiedFbPointer)) {
			vgpuFbBgraToYuv(&vgpuFbEncoderCtx, copiedFbPointer);
			free(copiedFbPointer);
			
			printf("encoding frame %d\n", totalFramesEncoded + 1);
			ret = avcodec_send_frame(cdcCtx, vgpuFbEncoderCtx.picToEncode);

			if (ret < 0) {
				fprintf(stderr, "Failed to send AVFrame to encoder\n");
				return -1;
			}

			while (ret >= 0) {
				ret = avcodec_receive_packet(cdcCtx, encPkt);
				// printf("encoding packet\n");
				if (ret == AVERROR(EAGAIN) || ret == AVERROR_EOF) {
					break;
				} else if (ret < 0) {
					fprintf(stderr, "Failed to receive AVPacket\n");
					return -1;
				}
				// encPkt->duration = avS->time_base.den / avS->time_base.num / 60 * 1;
				av_packet_rescale_ts(encPkt, (AVRational){1, IMGBUF_FRAMERATE}, avS->time_base);
				av_interleaved_write_frame(fmtCtx, encPkt);
				av_packet_unref(encPkt);
			}
			vgpuFbEncoderCtx.picToEncode->pts++;
			totalFramesEncoded++; // TODO why not just use pts?

		}
		else {
			printf("dequeue failed, continuing!!\n"); // FIX THIS!!
		}
	}

	printf("on pthread_join\n");
	ret = pthread_join(captureTh, NULL);
	if (ret != 0) {
		perror("pthread_join");
		return 1;
	}

	av_write_trailer(fmtCtx);
	avio_closep(&fmtCtx->pb);

	/*
	 * END ENCODING SECTION -------------------------------------------------------------------------------------------
	 */

	printf("image captured successfully and written to test.bmp\n");

	return 0;

}

#include <linux/vfio.h>
#include <fcntl.h>
#include <sys/ioctl.h>
#include <sys/mman.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include <stdint.h>



#include <libswscale/swscale.h>
#include <libavcodec/avcodec.h>
#include <libavutil/imgutils.h>
#include <libavutil/opt.h>
#include <libavformat/avformat.h>

#define IMGBUF_WIDTH  1920
#define IMGBUF_HEIGHT 1080
#define IMGBUF_FRAMERATE 60
#define MMAP_CONSOLE_OFFSET 0x10000000000

#define AV_LOG_TRACE   56

typedef struct {
	uint8_t b;
	uint8_t g;
	uint8_t r;
	uint8_t a;
} pixel;

typedef struct {
	uint8_t vgpuFd;
} vgpuFbHandle_t; // handle with fds that we use to get the framebuffer BGRA pointer
		
typedef struct {
	struct SwsContext *swsCtx;
	AVFrame *picToEncode; // allocated once, used more than once...
	uint8_t *fbPointer;
} vgpuFbEncoderCtx_t;

		       
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

int vgpuFbBgraToYuv(vgpuFbEncoderCtx_t* ctx) {
	int stride[1] = { IMGBUF_WIDTH * 4 }; // for alpha channel
	// printf("beginning sws_scale\n");
	return sws_scale(ctx->swsCtx, (const uint8_t* const*) &ctx->fbPointer, stride, 0, 
			IMGBUF_HEIGHT, ctx->picToEncode->data, ctx->picToEncode->linesize);
	printf("ending sws_scale\n");

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
	cdcCtx->height = 1080;
	cdcCtx->width = 1920;
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


	// 10 seconds of video 
	vgpuFbEncoderCtx.picToEncode->pts = 0;
	for (int i = 0; i < 600; ++i) {
		printf("vgpuFbBgraToYuv returned %d\n", vgpuFbBgraToYuv(&vgpuFbEncoderCtx));
		printf("encoded frame %d\n", i);
		// vgpuFbEncoderCtx.picToEncode->pts = vgpuFbEncoderCtx.picToEncode->best_effort_timestamp;
		/* ret = av_frame_make_writable(vgpuFbEncoderCtx.picToEncode); // why does this not work?
		if (ret < 0) { 
			fprintf(stderr, "Failed to make AVFrame writeable: %s\n", av_err2str(ret));
			return -1;
		}*/

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
		nanosleep((const struct timespec[]){{0, 20000000L}}, NULL);


	}

	av_write_trailer(fmtCtx);
	avio_closep(&fmtCtx->pb);

	/*
	 * END ENCODING SECTION -------------------------------------------------------------------------------------------
	 */

	printf("image captured successfully and written to test.bmp\n");

}

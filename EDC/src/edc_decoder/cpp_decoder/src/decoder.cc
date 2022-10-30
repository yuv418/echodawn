#include "../inc/decoder.h"

#include <boost/lockfree/policies.hpp>
#include <boost/lockfree/spsc_queue.hpp>
#include <cstdint>
#include <memory>

// https://github.com/joncampbell123/composite-video-simulator/issues/5
#ifdef av_err2str
#undef av_err2str
#include <string>

int save_frame_as_jpeg(AVCodecContext *pCodecCtx, AVFrame *pFrame,
                       int FrameNo) {
    const AVCodec *jpegCodec = avcodec_find_encoder(AV_CODEC_ID_JPEG2000);
    if (!jpegCodec) {
        return -1;
    }
    AVCodecContext *jpegContext = avcodec_alloc_context3(jpegCodec);
    if (!jpegContext) {
        return -1;
    }

    jpegContext->pix_fmt = AV_PIX_FMT_YUV420P;
    jpegContext->height = pFrame->height;
    jpegContext->width = pFrame->width;
    jpegContext->time_base = AVRational{25, 1};

    if (avcodec_open2(jpegContext, jpegCodec, NULL) < 0) {
        return -1;
    }
    FILE *JPEGFile;
    char JPEGFName[256];

    AVPacket packet = {.data = NULL, .size = 0};
    int gotFrame;

    avcodec_send_frame(jpegContext, pFrame);
    avcodec_receive_packet(jpegContext, &packet);
    sprintf(JPEGFName, "dvr-%06d.jpg", FrameNo);
    JPEGFile = fopen(JPEGFName, "wb");
    fwrite(packet.data, 1, packet.size, JPEGFile);
    fclose(JPEGFile);

    avcodec_close(jpegContext);
    return 0;
}

av_always_inline std::string av_err2string(int errnum) {
    char str[AV_ERROR_MAX_STRING_SIZE];
    return av_make_error_string(str, AV_ERROR_MAX_STRING_SIZE, errnum);
}

#define av_err2str(err) av_err2string(err).c_str()
#endif // av_err2str

void av_logging_callback(void *ptr, int lvl, const char *fmt, va_list vargs) {
    vprintf(fmt, vargs);
}

// TODO better exception handling
namespace edc_decoder {

EdcDecoder::EdcDecoder(rust::Str sdp_str, uint32_t width, uint32_t height) {
    this->frame_ring =
        new boost::lockfree::spsc_queue<AVFrame *,
                                        boost::lockfree::capacity<1000>>();

    this->sdp_str_cpp = "data:appliation/sdp;charset=UTF-8,";
    this->sdp_str_cpp += sdp_str.data();
    this->inp_ctx = NULL;

    // We won't enable this until we can properly log this with colours and
    // everything
    // av_log_set_level(AV_LOG_TRACE);
    // av_log_set_callback(av_logging_callback);
}

void EdcDecoder::start_decoding() {
    int ret;

    ret = avformat_network_init();

    if (ret) {
        throw std::runtime_error("avformat_network_init failed");
    }

    std::cout << "SDP String: " << sdp_str_cpp.c_str() << std::endl;

    AVDictionary *d = NULL;
    av_dict_set(&d, "fflags", "nobuffer", 0);
    av_dict_set(&d, "probesize", "32", 0);
    av_dict_set(&d, "analyzeduration", "0", 0);
    av_dict_set(&d, "max_delay", "2", 0);
    av_dict_set(&d, "flags", "low_delay", 0);
    av_dict_set(&d, "framedrop", "1", 0);
    av_dict_set(&d, "strict", "experimental", 0);
    av_dict_set(&d, "vf", "setpts=0", 0);
    ret = avformat_open_input(&this->inp_ctx, this->sdp_str_cpp.c_str(), NULL,
                              &d);

    this->inp_ctx->flags = AVFMT_FLAG_NOBUFFER;

    std::cout << "Input format is " << this->inp_ctx->iformat << std::endl;
    if (ret) {
        throw std::runtime_error("avformat_open_input failed");
    }
    std::cout << "finding stream info" << std::endl;
    // NOTE this doesn't actually work.
    ret = avformat_find_stream_info(this->inp_ctx, NULL);
    if (ret) {
        throw std::runtime_error("avformat_find_stream_info failed");
    }
    for (unsigned int i = 0; i < this->inp_ctx->nb_streams; ++i) {
        // For now, there is only the video stream and
        // no audio stream, so we do not have to handle the audio stream.
        // Nonetheless, we will check for an audio stream.
        AVStream *v_stream = this->inp_ctx->streams[i];
        if (v_stream->codecpar->codec_type != AVMEDIA_TYPE_VIDEO) {
            throw new std::runtime_error(
                "The client sent an audio stream when there should only be a "
                "video stream.");
        }
        this->cdc_ctx = avcodec_alloc_context3(NULL);
        avcodec_parameters_to_context(this->cdc_ctx, v_stream->codecpar);
    }
    this->cdc_ctx->flags = AV_CODEC_FLAG_LOW_DELAY;
    this->cdc_ctx->thread_count = 1;
    av_opt_set(this->cdc_ctx->priv_data, "tune", "zerolatency", 0);
    av_opt_set(this->cdc_ctx->priv_data, "profile", "baseline", 0);

    const AVCodec *decodeCodec;
    decodeCodec = avcodec_find_decoder(this->cdc_ctx->codec_id);

    ret = avcodec_open2(this->cdc_ctx, decodeCodec, NULL);
    if (ret) {
        printf("avcodec_open2 returned %s\n", av_err2str(ret));
        throw std::runtime_error("avcodec_open2 failed");
    }

    this->decoding_finished = false;
    this->decode_thread =
        new std::thread([this] { this->DecodeFrameThread(); });
}

bool EdcDecoder::DecodeFrameThread() {
    AVPacket pkt;
    AVFrame *frame;
    SwsContext *sws_ctx;
    AVFrame *rgb_frame;
    int ret;

    std::cout << "height, width: " << this->cdc_ctx->height << " "
              << this->cdc_ctx->width << std::endl;

    this->cdc_ctx->pix_fmt = AV_PIX_FMT_YUV420P;
    frame = av_frame_alloc();

    // TODO check all these variables
    sws_ctx = sws_getContext(this->cdc_ctx->width, this->cdc_ctx->height,
                             this->cdc_ctx->pix_fmt, this->cdc_ctx->width,
                             this->cdc_ctx->height, AV_PIX_FMT_RGB24,
                             SWS_BICUBIC, NULL, NULL, NULL);

    int i = 0;
    while (true) {
        if (this->decoding_finished) {
            break;
        }
        if ((ret = av_read_frame(this->inp_ctx, &pkt)) < 0) {
            printf("av_read_frame returned %s\n", av_err2str(ret));
            av_packet_unref(&pkt);
            continue;
        }
        ret = avcodec_send_packet(this->cdc_ctx, &pkt);
        if (ret > 0 || ret == AVERROR(EAGAIN) || ret == AVERROR_EOF) {
            if (ret >= 0) {
                pkt.size = 0;
            }
            ret = avcodec_receive_frame(this->cdc_ctx, frame);
            printf("avcodec_recieve_frame returned %s\n", av_err2str(ret));
            if (ret < 0) {
                printf("avcodec_recieve_frame returned %s\n", av_err2str(ret));
                continue;
            }

            AVFrame *rgb_frame = av_frame_alloc();

            rgb_frame->format = AV_PIX_FMT_RGB24;
            rgb_frame->width = this->cdc_ctx->width;
            rgb_frame->height = this->cdc_ctx->height;
            ret = av_image_alloc(rgb_frame->data, rgb_frame->linesize,
                                 rgb_frame->width, rgb_frame->height,
                                 AV_PIX_FMT_RGB24, 1);
            if (ret < 0) {
                return false;
            }
            ret = sws_scale(sws_ctx, frame->data, frame->linesize, 0,
                            cdc_ctx->height, rgb_frame->data,
                            rgb_frame->linesize);
            if (ret < 0) {
                printf("sws_scale returned %s\n", av_err2str(ret));
                continue;
            }
            // This won't push if the ring is full.
            this->frame_ring->push(rgb_frame);
        }
    }
}

AVFrame *EdcDecoder::fetch_ring_frame() const {
    AVFrame *poppedFrame;
    if (this->frame_ring->pop(poppedFrame)) {
        printf("poppedFrame pointer %p\n", poppedFrame);
        return poppedFrame;
    }
    return NULL;
}

EdcDecoder::~EdcDecoder() {
    this->decoding_finished = true;
    this->decode_thread->join();
    delete this->decode_thread;
    delete this->frame_ring;

    // av_freep(&this->inp_ctx);
    // av_freep(&this->cdc_ctx);
}

std::unique_ptr<EdcDecoder> new_edc_decoder(rust::Str sdp, uint32_t width,
                                            uint32_t height) {
    auto ptr = std::make_unique<EdcDecoder>(sdp, width, height);
    std::cout << "The pointer to EdcDecoder from C++ is equal to " << ptr.get()
              << std::endl;
    return ptr;
}

}; // namespace edc_decoder

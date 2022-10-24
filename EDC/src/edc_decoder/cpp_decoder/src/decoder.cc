#include "../inc/decoder.h"

#include <boost/lockfree/policies.hpp>
#include <boost/lockfree/spsc_queue.hpp>
#include <cstdint>
#include <memory>

void av_logging_callback(void *ptr, int lvl, const char *fmt, va_list vargs) {
    vprintf(fmt, vargs);
}

// TODO better exception handling
namespace edc_decoder {

EdcDecoder::EdcDecoder(rust::Str sdp_str, uint32_t width, uint32_t height) {
    this->frame_ring =
        new boost::lockfree::spsc_queue<AVFrame *,
                                        boost::lockfree::capacity<2>>();

    this->sdp_str_cpp = "data:appliation/sdp;charset=UTF-8,";
    this->sdp_str_cpp += sdp_str.data();
    this->inp_ctx = NULL;

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
    ret = avformat_open_input(&this->inp_ctx, this->sdp_str_cpp.c_str(), NULL,
                              &d);

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
    this->decoding_finished = false;
    this->decode_thread =
        new std::thread([this] { this->DecodeFrameThread(); });
}

bool EdcDecoder::DecodeFrameThread() {
    AVPacket pkt;
    AVFrame *frame;
    SwsContext *sws_ctx;
    AVFrame *rgb_frame;

    std::cout << "height, width: " << this->cdc_ctx->height << " "
              << this->cdc_ctx->width << std::endl;
    this->cdc_ctx->pix_fmt = AV_PIX_FMT_YUV420P;

    sws_getContext(this->cdc_ctx->width, this->cdc_ctx->height,
                   this->cdc_ctx->pix_fmt, this->cdc_ctx->width,
                   this->cdc_ctx->height, AV_PIX_FMT_RGB24, SWS_BICUBIC, NULL,
                   NULL, NULL);
    int ret;

    while (true) {
        if (this->decoding_finished) {
            break;
        }
        if (av_read_frame(this->inp_ctx, &pkt) < 0) {
            av_packet_unref(&pkt);
            continue;
        }
        ret = avcodec_send_packet(this->cdc_ctx, &pkt);
        if (ret > 0 || ret == AVERROR(EAGAIN) || ret == AVERROR_EOF) {
            if (ret >= 0) {
                pkt.size = 0;
            }
            ret = avcodec_receive_frame(this->cdc_ctx, frame);
            if (ret >= 0) {
                continue;
            }
            std::cout << "linesize " << frame->linesize << std::endl;
            ret = sws_scale(sws_ctx, frame->data, frame->linesize, 0,
                            cdc_ctx->height, rgb_frame->data,
                            rgb_frame->linesize);
            if (!ret) {
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

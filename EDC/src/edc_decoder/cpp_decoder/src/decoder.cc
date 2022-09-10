#include "../inc/decoder.h"

#include <boost/lockfree/policies.hpp>
#include <boost/lockfree/spsc_queue.hpp>
#include <libavutil/mem.h>
#include <memory>

// TODO better exception handling
namespace edc_decoder {

EdcDecoder::EdcDecoder(rust::Str sdp_str) {
    int ret;

    this->frame_ring =
        new boost::lockfree::spsc_queue<AVFrame *,
                                        boost::lockfree::capacity<2>>();

    ret = avformat_network_init();
    if (ret) {
        throw std::runtime_error("avformat_network_init failed");
    }

    // FFmpeg should
    std::string sdp_str_cpp("data:appliation/sdp;");
    sdp_str_cpp += sdp_str.data();

    std::cout << "SDP String: " << sdp_str << std::endl;

    ret = avformat_open_input(&this->inp_ctx, sdp_str.data(), NULL, NULL);
    if (ret) {
        throw std::runtime_error("avformat_open_input failed");
    }
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
    AVPacket *pkt;
    AVFrame *frame;
    SwsContext *sws_ctx;
    AVFrame *rgb_frame;

    sws_getContext(this->cdc_ctx->width, this->cdc_ctx->height,
                   this->cdc_ctx->pix_fmt, this->cdc_ctx->width,
                   this->cdc_ctx->height, AV_PIX_FMT_RGB24, SWS_BICUBIC, NULL,
                   NULL, NULL);
    int ret;
    if (av_read_frame(this->inp_ctx, pkt) < 0) {
        av_packet_unref(pkt);
        return false;
    }
    ret = avcodec_send_packet(this->cdc_ctx, pkt);
    if (ret > 0 || ret == AVERROR(EAGAIN) || ret == AVERROR_EOF) {
        if (ret >= 0) {
            pkt->size = 0;
        }
        ret = avcodec_receive_frame(this->cdc_ctx, frame);
        if (ret >= 0) {
            return true;
        }
        ret = sws_scale(sws_ctx, frame->data, frame->linesize, 0,
                        cdc_ctx->height, rgb_frame->data, rgb_frame->linesize);
        if (!ret) {
            return false;
        }
        // This won't push if the ring is full.
        this->frame_ring->push(rgb_frame);
    }
    return true;
}

EdcDecoder::~EdcDecoder() {
    this->decoding_finished = true;
    this->decode_thread->join();
    delete this->decode_thread;
    delete this->frame_ring;

    // av_freep(&this->inp_ctx);
    // av_freep(&this->cdc_ctx);
}

std::unique_ptr<EdcDecoder> new_edc_decoder(rust::Str sdp) {
    return std::make_unique<EdcDecoder>(sdp);
}

}; // namespace edc_decoder

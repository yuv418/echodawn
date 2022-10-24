#pragma once

#include "rust/cxx.h"
#include <boost/lockfree/policies.hpp>
#include <boost/lockfree/spsc_queue.hpp>
#include <iostream>
#include <memory>
#include <stdexcept>
#include <string>
#include <thread>

extern "C" {
#include <libavcodec/avcodec.h>
#include <libavcodec/codec.h>
#include <libavformat/avformat.h>
#include <libavutil/avutil.h>
#include <libavutil/error.h>
#include <libavutil/frame.h>
#include <libavutil/imgutils.h>
#include <libavutil/log.h>
#include <libavutil/mem.h>
#include <libavutil/pixfmt.h>
#include <libswscale/swscale.h>
}

namespace edc_decoder {

using AVFramePublic = AVFrame;

class EdcDecoder {
  public:
    EdcDecoder(rust::Str sdp, uint32_t width, uint32_t height);
    ~EdcDecoder();
    // For now. Later, we will reimplement AVFrame or something since I do not
    // know how to return an AVFrame from this method and access it in Rust.
    AVFrame *fetch_ring_frame() const;
    void start_decoding();

  private:
    boost::lockfree::spsc_queue<AVFrame *, boost::lockfree::capacity<2>>
        *frame_ring;
    bool decoding_finished;
    std::string sdp_str_cpp;
    std::thread *decode_thread;
    AVFormatContext *inp_ctx;
    AVCodecContext *cdc_ctx;
    // Queue of frames
    bool DecodeFrameThread();
};

std::unique_ptr<EdcDecoder> new_edc_decoder(rust::Str sdp, uint32_t width,
                                            uint32_t height);
} // namespace edc_decoder

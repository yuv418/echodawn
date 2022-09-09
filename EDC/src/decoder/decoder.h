#include <boost/lockfree/policies.hpp>
#include <boost/lockfree/spsc_queue.hpp>
#include <iostream>
#include <libavutil/avutil.h>
#include <libavutil/frame.h>
#include <stdexcept>
#include <string>
#include <thread>

extern "C" {
#include <libavcodec/avcodec.h>
#include <libavcodec/codec.h>
#include <libavformat/avformat.h>
#include <libswscale/swscale.h>
}

namespace edc_decoder {
class EdcDecoder {
  public:
    EdcDecoder(std::string t);
    ~EdcDecoder();

  private:
    std::thread decode_thread;
    AVFormatContext *inp_ctx;
    AVCodecContext *cdc_ctx;
    // Queue of frames
    boost::lockfree::spsc_queue<AVFrame *, boost::lockfree::capacity<2>>
        frame_ring;
    bool DecodeFrameThread();
};

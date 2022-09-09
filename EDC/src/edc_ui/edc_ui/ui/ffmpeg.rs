use std::ptr::{null, null_mut};

use anyhow::anyhow;

use super::video_decoder::VideoDecoder;

#[derive(Debug)]
pub struct FFmpegCtx {
    cdc: ffmpeg_sys::AVCodec,
    cdc: ffmpeg_sys::AVCodec,
}

impl VideoDecoder for FFmpegCtx {
    fn new(
        window: &Window,
        width: u32,
        height: u32,
        debug: bool,
        sdp: String,
    ) -> anyhow::Result<MPVCtx> {
        // TODO support hardware encoding?
        let cdc: ffmpeg_sys::AVCodec = ffmpeg_sys::avcodec_find_encoder_by_name("x264");
        if (cdc == null()) {
            anyhow!("Failed to get AVCodec");
        }
        let in_fmt: ffmpeg_sys::AVInputFormat = ffmpeg_sys::av_guess_format("rtp", NULL, NULL);
        // TODO do we really have to check these things like this?
        if (in_fmt == null()) {
            anyhow!("Failed to get AVInputFormat")
        }

        let fmt_ctx: AVFormatContext = null_mut();
        ffmpeg_sys::avformat_open_input(
            &fmt_ctx,
            in_fmt,
            in_fmt.name,
            "data:application/sdp;".to_owned() + &sdp,
        );

        unimplemented!()
    }

    fn paint(&mut self, _window: &Window) {}

    fn handle_window_event(&self, _window_id: WindowId, event: WindowEvent) {
        todo!()
    }

    fn handle_user_event(&self, window: &Window, _ctrl_flow: &ControlFlow, event: &MPVEvent) {
        todo!()
    }

    fn needs_evloop_proxy(&mut self) -> bool {
        todo!()
    }
}

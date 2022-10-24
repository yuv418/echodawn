use std::ptr::{null, null_mut};

use anyhow::anyhow;
use cxx::UniquePtr;
use glutin::{
    event::WindowEvent,
    event_loop::ControlFlow,
    window::{Window, WindowId},
};
use log::trace;

use super::{mpv::MPVEvent, video_decoder::VideoDecoder};
use crate::edc_decoder::decoder_bridge::{self, EdcDecoder};

pub struct FFmpegCtx {
    decoder: UniquePtr<EdcDecoder>,
}

impl std::fmt::Debug for FFmpegCtx {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FFmpegCtx")
            .field("decoder -> {}", &"CPP impl")
            .finish()
    }
}

impl VideoDecoder for FFmpegCtx {
    fn new(
        window: &Window,
        width: u32,
        height: u32,
        debug: bool,
        sdp: String,
    ) -> anyhow::Result<Box<dyn VideoDecoder>> {
        std::fs::write("test.sdp", &sdp)?;
        let mut decoder = decoder_bridge::new_edc_decoder(&sdp, width, height);
        trace!("decoder pointer is {:p}", decoder.as_mut().unwrap());
        unsafe {
            let a = decoder.fetch_ring_frame();
            trace!("fetched ring frame address is {:p}", a);
            // println!("{:?}", (*a?/ ).);
        }
        Ok(Box::new(Self { decoder }))
    }

    fn paint(&mut self, _window: &Window) {}

    fn handle_window_event(&self, _window_id: WindowId, event: WindowEvent) {}

    fn handle_user_event(&self, window: &Window, _ctrl_flow: &ControlFlow, event: &MPVEvent) {}

    fn needs_evloop_proxy(&mut self) -> bool {
        false
    }

    fn give_evloop_proxy(
        &mut self,
        evloop_proxy: std::rc::Rc<glutin::event_loop::EventLoopProxy<MPVEvent>>,
    ) -> bool {
        true
    }

    fn start_decoding(&mut self) {
        // Start the stream.
        self.decoder.as_mut().unwrap().start_decoding();
    }
}

use std::ptr::{null, null_mut};

use anyhow::anyhow;
use glutin::{
    event::WindowEvent,
    event_loop::ControlFlow,
    window::{Window, WindowId},
};

use super::{mpv::MPVEvent, video_decoder::VideoDecoder};
use crate::edc_decoder::decoder_bridge;

#[derive(Debug)]
pub struct FFmpegCtx {}

impl VideoDecoder for FFmpegCtx {
    fn new(
        window: &Window,
        width: u32,
        height: u32,
        debug: bool,
        sdp: String,
    ) -> anyhow::Result<Box<dyn VideoDecoder>> {
        let decoder = decoder_bridge::new_edc_decoder(&sdp);
        unimplemented!()
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
}

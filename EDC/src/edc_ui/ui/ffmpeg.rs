use std::{
    ptr::{null, null_mut},
    rc::Rc,
};

use anyhow::anyhow;
use cxx::UniquePtr;
use ffmpeg_sys_next::AVFrame;
use glow::HasContext;
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
    width: u32,
    height: u32,
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
        let mut decoder = decoder_bridge::new_edc_decoder(&sdp, width, height);
        trace!("decoder pointer is {:p}", decoder.as_mut().unwrap());
        unsafe {
            let a = decoder.fetch_ring_frame();
            trace!("fetched ring frame address is {:p}", a);
            // println!("{:?}", (*a?/ ).);
        }
        Ok(Box::new(Self {
            decoder,
            width,
            height,
        }))
    }

    fn paint(&mut self, gl: Rc<glow::Context>, window: &Window) {
        let frame = self.decoder.fetch_ring_frame();
        if frame.is_null() {
            return;
        }
        unsafe {
            // I love overriding the Rust type system /s
            let frame: *mut AVFrame = std::mem::transmute(frame);
            let frame_length = ffmpeg_sys_next::av_image_get_buffer_size(
                ffmpeg_sys_next::AVPixelFormat::AV_PIX_FMT_RGB24,
                (*frame).width,
                (*frame).height,
                32,
            );
            let pixels_slice = std::slice::from_raw_parts((*frame).data[0], frame_length as usize);
            trace!("frame data address {:?}", pixels_slice);
            gl.tex_sub_image_2d(
                glow::TEXTURE_2D,
                0,
                0,
                0,
                self.width as i32,
                self.height as i32,
                glow::RGB,
                glow::UNSIGNED_BYTE,
                glow::PixelUnpackData::Slice(pixels_slice),
            );
        }
    }

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

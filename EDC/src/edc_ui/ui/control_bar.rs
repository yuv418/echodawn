use std::{cell::RefCell, rc::Rc};

use glutin::{event_loop::EventLoop, window::Window};

use crate::edcs_client::blocking_client::BlockingEdcsClient;

use super::{
    debug_area::DebugArea,
    mpv::{self, MPVEvent},
    ui_element::UIElement,
};

/// The bar that lets you control everything during an active connection
pub struct ControlBarUI {
    client: Rc<RefCell<BlockingEdcsClient>>,
    debug_area: Rc<RefCell<DebugArea>>,
    mpv_ctx: mpv::MPVCtx,
}
impl ControlBarUI {
    pub fn new(
        client: Rc<RefCell<BlockingEdcsClient>>,
        debug_area: Rc<RefCell<DebugArea>>,
        window: Rc<Window>,
        evloop: Rc<EventLoop<MPVEvent>>,
        sdp: String,
    ) -> Self
    where
        Self: Sized,
    {
        let inner_size = window.inner_size();
        Self {
            client,
            debug_area,
            mpv_ctx: mpv::MPVCtx::new(
                window,
                evloop,
                inner_size.width,
                inner_size.height,
                // TODO make this variable
                true,
                sdp,
            )
            .expect("Failed to start MPV"),
        }
    }
}

impl UIElement for ControlBarUI {
    fn render(
        &mut self,
        ui: &mut egui::Ui,
        ctrl_flow: &mut glutin::event_loop::ControlFlow,
    ) -> egui::InnerResponse<()> {
        todo!()
    }

    fn handle_messages(&mut self) {}

    fn next_element(&mut self, window: &Window) -> Option<Box<dyn UIElement>> {
        None
    }

    fn handle_window_event(
        &mut self,
        window: &Window,
        ctrl_flow: &mut glutin::event_loop::ControlFlow,
        window_id: glutin::window::WindowId,
        event: &glutin::event::WindowEvent,
    ) {
    }

    fn handle_user_event(
        &self,
        window: &Window,
        ctrl_flow: &glutin::event_loop::ControlFlow,
        event: &MPVEvent,
    ) {
        self.mpv_ctx.handle_user_event(window, ctrl_flow, event)
    }

    fn paint_before_egui(&mut self, window: &Window) {
        todo!()
    }

    fn paint_after_egui(&mut self, window: &Window) {
        todo!()
    }

    fn needs_evloop_proxy(&mut self) -> bool {
        self.mpv_ctx.needs_evloop_proxy()
    }

    fn give_evloop_proxy(&mut self, evloop_proxy: glutin::event_loop::EventLoopProxy<MPVEvent>) {
        if self.mpv_ctx.needs_evloop_proxy() {
            self.mpv_ctx.give_evloop_proxy(evloop_proxy);
        }
    }
}

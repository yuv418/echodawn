use std::{cell::RefCell, rc::Rc};

use egui::RichText;
use glutin::{
    dpi::PhysicalPosition,
    event::{ElementState, WindowEvent},
    event_loop::EventLoop,
    window::Window,
};
use log::{debug, info};

use crate::edcs_client::{
    blocking_client::{BlockingEdcsClient, ChannelEdcsRequest},
    edcs_proto::{EdcsKeyData, EdcsMouseButton},
    keyboard_event,
};

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
    stream_started: bool,
    prev_pos: PhysicalPosition<f64>,
}
impl ControlBarUI {
    pub fn new(
        client: Rc<RefCell<BlockingEdcsClient>>,
        debug_area: Rc<RefCell<DebugArea>>,
        window: &Window,
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
                inner_size.width,
                inner_size.height,
                // TODO make this variable
                true,
                sdp,
            )
            .expect("Failed to start MPV"),
            stream_started: false,
            prev_pos: PhysicalPosition { x: 0.0, y: 0.0 },
        }
    }
}

impl UIElement for ControlBarUI {
    fn render(
        &mut self,
        ui: &mut egui::Ui,
        ctrl_flow: &mut glutin::event_loop::ControlFlow,
    ) -> egui::InnerResponse<()> {
        self.handle_messages();
        egui::Frame::none()
            .fill(egui::Color32::DARK_GRAY)
            .inner_margin(10.0)
            .outer_margin(10.0)
            .show(ui, |ui| {
                ui.heading(RichText::new("Connection").strong());
            })
    }

    fn handle_messages(&mut self) {
        let needs_evloop_proxy = self.needs_evloop_proxy();
        if !self.stream_started && !needs_evloop_proxy {
            info!("Starting video stream");
            self.client
                .borrow()
                .push
                .send(ChannelEdcsRequest::StartStream)
                .expect("Failed to start video stream");
            self.stream_started = true;
        }
    }

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
        match event {
            WindowEvent::CursorMoved { position, .. } => {
                debug!("mouse move source {:?}", event);
                debug!(
                    "try send to self.client returns {:?}",
                    self.client
                        .borrow()
                        .push
                        .send(ChannelEdcsRequest::WriteMouseMove {
                            x: position.x as f64,
                            y: position.y as f64,
                        })
                );
            }
            WindowEvent::MouseInput { state, button, .. } => {
                debug!("mouse move source {:?}", event);
                self.client
                    .borrow()
                    .push
                    .send(ChannelEdcsRequest::WriteMouseButton {
                        button_typ: match button {
                            glutin::event::MouseButton::Left => EdcsMouseButton::MouseButtonLeft,
                            glutin::event::MouseButton::Right => EdcsMouseButton::MouseButtonRight,
                            glutin::event::MouseButton::Middle => {
                                EdcsMouseButton::MouseButtonMiddle
                            }
                            glutin::event::MouseButton::Other(_) => return, // ignore this
                        },
                        pressed: if let ElementState::Pressed = state {
                            true
                        } else {
                            false
                        },
                    });
            }
            WindowEvent::KeyboardInput {
                device_id,
                input,
                is_synthetic,
            } => {
                debug!("keyinput {:?}", input);
                self.client
                    .borrow()
                    .push
                    .send(ChannelEdcsRequest::WriteKeyboardEvent {
                        key_typ: keyboard_event::virtual_key_code_to_linux_input(
                            // Do we really want to crash the entire application because of this?
                            input
                                .virtual_keycode
                                .expect("Failed to get the input's virtual keycode"),
                        ),
                        pressed: input.state == ElementState::Pressed,
                    });
            }
            _ => {}
        }
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
        self.mpv_ctx.paint(window)
    }

    fn paint_after_egui(&mut self, _window: &Window) {}

    fn needs_evloop_proxy(&mut self) -> bool {
        self.mpv_ctx.needs_evloop_proxy()
    }

    fn give_evloop_proxy(
        &mut self,
        evloop_proxy: Rc<glutin::event_loop::EventLoopProxy<MPVEvent>>,
    ) {
        if self.mpv_ctx.needs_evloop_proxy() {
            self.mpv_ctx.give_evloop_proxy(evloop_proxy);
        }
    }
}

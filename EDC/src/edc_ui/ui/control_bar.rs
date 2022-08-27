use std::{cell::RefCell, rc::Rc};

use egui::RichText;
use glutin::{
    dpi::PhysicalPosition,
    event::{ElementState, VirtualKeyCode, WindowEvent},
    window::Window,
};
use log::{info, trace};

use crate::edcs_client::{
    blocking_client::{BlockingEdcsClient, ChannelEdcsRequest},
    edcs_proto::EdcsMouseButton,
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
        window.set_cursor_visible(false);
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
    fn render_egui(&mut self) -> bool {
        false
    }
    fn render(
        &mut self,
        ui: &mut egui::Ui,
        _ctrl_flow: &mut glutin::event_loop::ControlFlow,
    ) -> egui::InnerResponse<()> {
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

    fn next_element(&mut self, _window: &Window) -> Option<Box<dyn UIElement>> {
        None
    }

    fn handle_window_event(
        &mut self,
        _window: &Window,
        _ctrl_flow: &mut glutin::event_loop::ControlFlow,
        _window_id: glutin::window::WindowId,
        event: &glutin::event::WindowEvent,
    ) {
        match event {
            WindowEvent::CursorMoved { position, .. } => {
                trace!("mouse move source {:?}", event);
                let ret = self
                    .client
                    .borrow()
                    .push
                    .send(ChannelEdcsRequest::WriteMouseMove {
                        x: position.x as f64,
                        y: position.y as f64,
                    });
                trace!("try send to self.client returns {:?}", ret);
            }
            WindowEvent::MouseInput { state, button, .. } => {
                trace!("mouse move source {:?}", event);
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
                device_id: _,
                input,
                is_synthetic: _,
            } => {
                trace!("keyinput {:?}", input);
                let key_typ = if cfg!(linux) {
                    input.scancode as i32
                } else {
                    match input.virtual_keycode {
                        Some(vkeycd) => keyboard_event::virtual_key_code_to_linux_input(vkeycd),
                        None => 0,
                    }
                };
                self.client
                    .borrow()
                    .push
                    .send(ChannelEdcsRequest::WriteKeyboardEvent {
                        key_typ: key_typ,
                        pressed: input.state == ElementState::Pressed,
                    });
            }
            WindowEvent::ModifiersChanged(mod_state) => {
                if !cfg!(linux) {
                    let send_event = |pressed: bool, vkeycd: VirtualKeyCode| {
                        self.client
                            .borrow()
                            .push
                            .send(ChannelEdcsRequest::WriteKeyboardEvent {
                                key_typ: keyboard_event::virtual_key_code_to_linux_input(
                                    // Do we really want to crash the entire application because of this?
                                    vkeycd,
                                ),
                                pressed,
                            });
                    };
                    if mod_state.logo() {
                        send_event(true, VirtualKeyCode::LWin);
                    } else {
                        send_event(false, VirtualKeyCode::LWin);
                    }
                }
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

    fn paint_before_egui(&mut self, _gl: Rc<glow::Context>, window: &Window) {
        self.handle_messages();
        self.mpv_ctx.paint(window)
    }

    fn paint_after_egui(&mut self, _gl: Rc<glow::Context>, _window: &Window) {}

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

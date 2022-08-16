use std::{
    cell::RefCell,
    collections::HashMap,
    path::PathBuf,
    rc::Rc,
    sync::{Arc, RwLock},
    task::Poll,
};

use egui::{InnerResponse, RichText};
use egui_glow::EguiGlow;
use glutin::{
    event_loop::{ControlFlow, EventLoop},
    window::Window,
};
use log::{debug, error, info};

use crate::edcs_client::{
    blocking_client::{BlockingEdcsClient, ChannelEdcsRequest, ChannelEdcsResponse},
    edcs_proto::{edcs_response::Payload, EdcsStatus},
};

use super::{
    control_bar::ControlBarUI, debug_area::DebugArea, mpv::MPVEvent, ui_element::UIElement,
};

#[derive(PartialEq, Debug)]
enum ConnectionStage {
    Connect(bool),
    SetupEdcs,
    SetupStream,
    // We don't start the stream here
    Handoff,
}

pub struct ConnectUI {
    config_path: String,
    client: Rc<RefCell<BlockingEdcsClient>>,
    connection_stage: ConnectionStage,
    debug_area: Rc<RefCell<DebugArea>>,
    // Skip sending a request if one is pending
    pending_recv: bool,
    sdp: Option<String>,
}
impl ConnectUI {
    pub fn new(
        client: Rc<RefCell<BlockingEdcsClient>>,
        debug_area: Rc<RefCell<DebugArea>>,
    ) -> ConnectUI {
        ConnectUI {
            config_path: String::new(),
            debug_area,
            client,
            connection_stage: ConnectionStage::Connect(false),
            pending_recv: false,
            sdp: None,
        }
    }
}

impl UIElement for ConnectUI {
    fn render(&mut self, ui: &mut egui::Ui, ctrl_flow: &mut ControlFlow) -> InnerResponse<()> {
        // Don't send/recv messages if it's not necessary
        if self.connection_stage != ConnectionStage::Handoff {
            self.handle_messages();
        }
        egui::Frame::none()
            .fill(egui::Color32::DARK_GRAY)
            .inner_margin(10.0)
            .outer_margin(10.0)
            .show(ui, |ui| {
                ui.heading(RichText::new("Echodawn").strong());
                ui.text_edit_singleline(&mut self.config_path);
                if ui.button("Connect").clicked() {
                    info!("Starting up client!");
                    self.connection_stage = ConnectionStage::Connect(true);
                }
            })
    }

    /// Send and receive messages from the blocking client.
    fn handle_messages(&mut self) {
        let waker = futures::task::noop_waker();
        let mut cx = std::task::Context::from_waker(&waker);
        if let Poll::Ready(Some(msg)) = self.client.borrow_mut().recv.poll_recv(&mut cx) {
            self.pending_recv = false;
            debug!("set pending_recv to false");
            match msg {
                ChannelEdcsResponse::EdcsClientInitialised => {
                    self.connection_stage = ConnectionStage::SetupEdcs;
                }
                ChannelEdcsResponse::EdcsClientInitError(e) => {
                    self.debug_area
                        .borrow_mut()
                        .push(&format!("Failed to init client: {:?}", e));
                    self.connection_stage = ConnectionStage::Connect(false);
                }
                ChannelEdcsResponse::InvalidClient => self
                    .debug_area
                    .borrow_mut()
                    .push("Called RPC with invalid client!"),
                ChannelEdcsResponse::EdcsResponse(r) => match r {
                    Ok(resp) => {
                        if resp.status() != EdcsStatus::Ok {
                            self.debug_area
                                .borrow_mut()
                                .push(&format!("RPC call response was not ok! Resp: {:?}", resp));
                            // TODO figure out what the status is and handle it accordingly
                            self.connection_stage = ConnectionStage::Connect(false);
                        } else {
                            match resp.payload {
                                Some(p) => match p {
                                    Payload::SetupEdcsData(setup_edcs_data) => {
                                        debug!("connection stage setup stream");
                                        self.connection_stage = ConnectionStage::SetupStream;
                                        self.debug_area
                                            .borrow_mut()
                                            .push(&format!("SetupEdcsData {:?}", setup_edcs_data));
                                    }
                                    Payload::SetupStreamData(setup_stream_data) => {
                                        self.connection_stage = ConnectionStage::Handoff;
                                        self.sdp = Some(setup_stream_data.sdp.clone());
                                        self.debug_area.borrow_mut().push(&format!(
                                            "SetupStreamData {:?}",
                                            setup_stream_data
                                        ));
                                    }
                                    _ => {}
                                },
                                None => {
                                    // Some calls actually return no payload, it just depends on what connection stage we are on for
                                    // if we have to handle it. The only call that does this is if you start the stream (for relevant RPCs), which is not something
                                    // we care about here.
                                }
                            };
                        }
                    }
                    Err(err) => self
                        .debug_area
                        .borrow_mut()
                        .push(&format!("EDCS call failed with {:?}", err)),
                },
            }
        }

        if !self.pending_recv {
            let push = &self.client.borrow().push;
            self.pending_recv = true;
            match self.connection_stage {
                ConnectionStage::Connect(true) => {
                    push.blocking_send(ChannelEdcsRequest::NewClient(PathBuf::from(
                        &self.config_path,
                    )))
                    .expect("Failed to push NewClient");
                }
                ConnectionStage::SetupEdcs => {
                    push.blocking_send(ChannelEdcsRequest::SetupEdcs {
                        bitrate: 100000000,
                        framerate: 60,
                    })
                    .expect("Failed to push SetupEdcs");
                }
                ConnectionStage::SetupStream => {
                    push.blocking_send(ChannelEdcsRequest::SetupStream({
                        let mut x = HashMap::new();
                        x.insert("vgpuId".to_owned(), "2".to_owned());
                        x
                    }))
                    .expect("Failed to push SetupStream");
                }
                // Starting the stream should be done elsewhere after MPV has initialised.
                _ => {
                    // Never sent anything to the channel on ConnectionStage::Connect(false)
                    self.pending_recv = false;
                }
            }
        }
    }

    fn next_element(&mut self, window: &Window) -> Option<Box<dyn UIElement>> {
        if let ConnectionStage::Handoff = self.connection_stage {
            Some(Box::new(ControlBarUI::new(
                self.client.clone(),
                self.debug_area.clone(),
                window,
                // SDP should never be None here
                self.sdp
                    .as_ref()
                    .expect("No SDP set despite ConnectionStage::Handoff set")
                    .to_owned(),
            )))
        } else {
            None
        }
    }

    fn paint_before_egui(&mut self, window: &glutin::window::Window) {}
    fn paint_after_egui(&mut self, window: &glutin::window::Window) {}

    fn handle_window_event(
        &mut self,
        window: &Window,
        ctrl_flow: &mut ControlFlow,
        window_id: glutin::window::WindowId,
        event: &glutin::event::WindowEvent,
    ) {
    }

    fn handle_user_event(&self, window: &Window, ctrl_flow: &ControlFlow, event: &MPVEvent) {
        // Do nothing
    }

    fn needs_evloop_proxy(&mut self) -> bool {
        false
    }

    fn give_evloop_proxy(
        &mut self,
        evloop_proxy: Rc<glutin::event_loop::EventLoopProxy<MPVEvent>>,
    ) {
    }
}

use std::{
    cell::RefCell,
    collections::HashMap,
    path::PathBuf,
    rc::Rc,
    sync::{Arc, RwLock},
    task::Poll,
};

use egui::{InnerResponse, RichText};
use glutin::event_loop::ControlFlow;
use log::{debug, error, info};

use crate::edcs_client::{
    blocking_client::{BlockingEdcsClient, ChannelEdcsRequest, ChannelEdcsResponse},
    edcs_proto::{edcs_response::Payload, EdcsStatus},
};

use super::debug_area::DebugArea;

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
        }
    }

    pub fn render(&mut self, ui: &mut egui::Ui, ctrl_flow: &mut ControlFlow) -> InnerResponse<()> {
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
    pub fn handle_messages(&mut self) {
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
                        } else {
                            match resp.payload {
                                Some(p) => {
                                    match p {
                                        Payload::SetupEdcsData(_) => {
                                            self.connection_stage = ConnectionStage::SetupStream;
                                        }
                                        Payload::SetupStreamData(_) => {
                                            self.connection_stage = ConnectionStage::Handoff;
                                            self.debug_area.borrow_mut().push("ConnectionState set to handoff, starting control bar");
                                        }
                                        _ => {}
                                    }
                                }
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
}

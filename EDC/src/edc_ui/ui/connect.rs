// NOTE Rustfmt doesn't work on this file

use std::{
    cell::RefCell, collections::HashMap, net::Ipv4Addr, path::PathBuf, pin::Pin, rc::Rc,
    str::FromStr, task::Poll,
};

use egui::{InnerResponse, RichText};

use futures::Future;
use glow::HasContext;
use glutin::{event_loop::ControlFlow, window::Window};
use log::{debug, trace};

use crate::{
    edcs_client::{
        blocking_client::{BlockingEdcsClient, ChannelEdcsRequest, ChannelEdcsResponse},
        edcs_proto::{edcs_response::Payload, EdcsStatus},
    },
    edcs_config::{ClientConfig, ConnectionConfig, ConnectionFile, StreamConfig, UIConfig},
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

#[derive(PartialEq, Debug)]
enum AddClientStage {
    // These are for the two fields in ClientConfig that cannot just be wholesale plugged into an egui component
    ClientConfigStage((String, String)),
    StreamConfigStage,
    UIConfigStage,
}

pub struct ConnectUI {
    edit_connection: (bool, usize),
    connection_config: Option<ConnectionConfig>,
    client: Rc<RefCell<BlockingEdcsClient>>,
    connection_stage: ConnectionStage,
    add_client_stage: Option<AddClientStage>,
    debug_area: Rc<RefCell<DebugArea>>,
    // Skip sending a request if one is pending
    pending_recv: bool,
    connection_file: ConnectionFile,
    sdp: Option<String>,
}

impl ConnectUI {
    pub fn new(
        client: Rc<RefCell<BlockingEdcsClient>>,
        debug_area: Rc<RefCell<DebugArea>>,
    ) -> ConnectUI {
        ConnectUI {
            edit_connection: (false, 0),
            connection_config: None,
            debug_area,
            client,
            connection_stage: ConnectionStage::Connect(false),
            add_client_stage: None,
            pending_recv: false,
            // I think panicking here is fine
            connection_file: ConnectionFile::open().expect("Failed to open echodawn client config"),
            sdp: None,
        }
    }
}

impl UIElement for ConnectUI {
    fn render(&mut self, ui: &mut egui::Ui, _ctrl_flow: &mut ControlFlow) -> InnerResponse<()> {
        // Don't send/recv messages if it's not necessary
        if self.connection_stage != ConnectionStage::Handoff {
            self.handle_messages();
        }
        egui::Frame::none()
            .fill(egui::Color32::DARK_GRAY)
            .inner_margin(10.0)
            .outer_margin(10.0)
            .show(ui, |ui| {
                egui::Grid::new("edc.connection").spacing((0.0, 12.0)).show(ui, |ui| {
                    ui.heading(RichText::new("Echodawn").strong());
                    if let None = self.add_client_stage {
                        if ui.button("Add Connection").clicked() {
                            self.connection_config = Some(ConnectionConfig {
                                stream_config: StreamConfig {
                                    bitrate: 0,
                                    framerate: 0,
                                    cal_plugin_params: HashMap::new(),
                                },

                                client_config: ClientConfig {
                                    host: Ipv4Addr::new(0, 0, 0, 0),
                                    domain: "".into(),
                                    port: 0,
                                    cert: "".into(),
                                    disable_tls_verification: false,
                                },
                                ui_config: UIConfig { host_cursor: false },
                            });
                            self.add_client_stage = Some(AddClientStage::ClientConfigStage((
                                "".to_owned(),
                                "".to_owned(),
                            )));
                        }
                    }
                    ui.end_row();

                    if let Some(stage) = &mut self.add_client_stage {
                        match stage {
                            AddClientStage::ClientConfigStage((
                                                                  ref mut ipv4_addr_str,
                                                                  ref mut cert_path_str,
                                                              )) => {
                                let c = &mut self.connection_config.as_mut().unwrap().client_config;
                                ui.colored_label(
                                    egui::Color32::LIGHT_BLUE,
                                    egui::RichText::new(
                                        if self.edit_connection.0 {
                                            "Edit the connection"
                                        } else {
                                            "Add a connection"
                                        }),
                                );
                                ui.end_row();

                                ui.strong("Host: ");
                                // TODO this code is repeated; what can we do to make it DRY?
                                let invalid_ipv4 = match Ipv4Addr::from_str(&ipv4_addr_str) {
                                    Ok(ipv4) => {
                                        c.host = ipv4;
                                        false
                                    }
                                    Err(_) => true,
                                };
                                let ipv4_text_edit = egui::TextEdit::singleline(ipv4_addr_str);
                                let ipv4_text_edit = if invalid_ipv4 {
                                    ipv4_text_edit.text_color(egui::Color32::RED)
                                } else {
                                    ipv4_text_edit
                                };
                                ui.add(ipv4_text_edit);
                                ui.end_row();

                                ui.strong("Domain: ");
                                ui.text_edit_singleline(&mut c.domain);
                                ui.end_row();

                                ui.strong("Port: ");
                                // For other boxes
                                ui.add_sized([170.0, 20.0], egui::DragValue::new(&mut c.port));
                                ui.end_row();

                                let invalid_cert = match PathBuf::from_str(&cert_path_str) {
                                    Ok(p) => {
                                        // TODO add even more validation here, for mimetype/extension
                                        if p.exists() && p.is_file() {
                                            c.cert = p;
                                            false
                                        } else {
                                            true
                                        }
                                    }
                                    Err(_) => {
                                        // TODO We want to actually set the text edit border, not the text colour
                                        // … And use the error in a tooltip perhaps
                                        true
                                    }
                                };
                                let cert_text_edit = egui::TextEdit::singleline(cert_path_str);
                                let cert_text_edit = if invalid_cert {
                                    cert_text_edit.text_color(egui::Color32::RED)
                                } else {
                                    cert_text_edit
                                };
                                ui.strong("Certificate Path: ");
                                ui.add(cert_text_edit);
                                ui.end_row();

                                ui.strong("Disable TLS Verification: ");
                                ui.checkbox(&mut c.disable_tls_verification, "");
                                ui.end_row();

                                if ui
                                    .add_enabled(
                                        !invalid_cert && !invalid_ipv4 && !c.domain.is_empty(),
                                        egui::Button::new("Next"),
                                    )
                                    .clicked()
                                {
                                    if self.edit_connection.0 {
                                        self.add_client_stage = Some(AddClientStage::StreamConfigStage);
                                    } else {
                                        self.connection_stage =
                                            ConnectionStage::Connect(true)
                                    }
                                }
                            }
                            AddClientStage::StreamConfigStage => {
                                let s = &mut self.connection_config.as_mut().unwrap().stream_config;
                                ui.colored_label(
                                    egui::Color32::LIGHT_BLUE,
                                    egui::RichText::new(if self.edit_connection.0 {
                                        "Edit streaming parameters"
                                    } else {
                                        "Set streaming parameters"
                                    }),
                                );
                                ui.end_row();

                                ui.strong("Bitrate: ");
                                ui.add_sized([170.0, 20.0], egui::DragValue::new(&mut s.bitrate));
                                ui.end_row();

                                ui.strong("Framerate: ");
                                ui.add_sized([170.0, 20.0], egui::DragValue::new(&mut s.framerate));
                                ui.end_row();

                                // Loop through each CAL plugin option and give the option to set it here
                                let mut vals_blank = false;
                                for (k, v) in s.cal_plugin_params.iter_mut() {
                                    ui.strong(k.to_owned() + ": ");
                                    if v.is_empty() {
                                        vals_blank = true
                                    }
                                    ui.text_edit_singleline(v);
                                }

                                ui.end_row();
                                if ui
                                    .add_enabled(
                                        s.bitrate > 0 && s.framerate > 0 && !vals_blank,
                                        egui::Button::new("Next"),
                                    )
                                    .clicked()
                                {
                                    self.add_client_stage = Some(AddClientStage::UIConfigStage);
                                }
                            }
                            AddClientStage::UIConfigStage => {
                                let u = &mut self.connection_config.as_mut().unwrap().ui_config;
                                ui.colored_label(
                                    egui::Color32::LIGHT_BLUE,
                                    egui::RichText::new(if self.edit_connection.0 {
                                        "Edit connection UI parameters"
                                    } else {
                                        "Set connection UI parameters"
                                    }),
                                );
                                ui.end_row();

                                ui.strong("Host Cursor: ");
                                ui.checkbox(&mut u.host_cursor, "");
                                ui.end_row();

                                if ui.button(if self.edit_connection.0 { "Modify Connection" } else { "Add Connection" }).clicked() {
                                    trace!(
                                        "self.connection_config is {:#?}",
                                        self.connection_config
                                    );
                                    // TODO don't panic
                                    let config_ref = self
                                        .connection_file
                                        .connection_config_ref()
                                        .expect("Failed to get conn config ref");

                                    if self.edit_connection.0 {
                                        config_ref[self.edit_connection.1] = self.connection_config.as_ref().unwrap().clone();
                                    } else {
                                        config_ref
                                            .push(self.connection_config.as_ref().unwrap().clone());
                                    }

                                    self.connection_file
                                        .write_config()
                                        .expect("Failed to write conn config");
                                    debug!("Wrote config file");
                                    self.edit_connection = (false, 0);
                                    self.add_client_stage = None;
                                    self.connection_config = None;
                                    self.connection_stage = ConnectionStage::Connect(false);
                                }
                            }
                        }
                    } else {
                        let mut remove_index = None;
                        let cnx_list = self
                            .connection_file
                            .connection_config_ref()
                            .expect("Failed to get connection list");
                        for (i, connection) in cnx_list.iter().enumerate() {
                            egui::Frame::none()
                                .fill(egui::Color32::from_gray(20))
                                .inner_margin(egui::style::Margin::same(10.0))
                                .show(ui, |ui| {
                                    egui::Grid::new("edc.connection.host").show(ui, |ui| {
                                        ui.strong(format!(
                                            "Connection to {:?}",
                                            connection.client_config.host
                                        ));
                                        ui.end_row();
                                        ui.horizontal(|ui| {
                                            if ui.button("Connect").clicked() {
                                                self.connection_config = Some(connection.clone());
                                                self.connection_stage = ConnectionStage::Connect(true);
                                            }
                                            ui.add_space(7.0);
                                            if ui.add(egui::Button::new(RichText::new("Remove")
                                                .color(egui::Color32::WHITE))
                                                .fill(egui::Color32::DARK_RED))
                                                 .clicked()
                                            {
                                                remove_index = Some(i);
                                            }
                                            ui.add_space(7.0);
                                            if ui.add(egui::Button::new(RichText::new("Edit")
                                                .color(egui::Color32::WHITE))
                                                .fill(egui::Color32::from_rgb(183, 107, 0))).clicked() {
                                                self.edit_connection = (true, i);
                                                self.connection_config = Some(connection.clone());
                                                self.add_client_stage = Some(
                                                    AddClientStage::ClientConfigStage
                                                        ((connection.client_config.host.to_string().clone(),
                                                          connection.client_config.cert.to_str()
                                                                    .expect("Invalid connection cert path")
                                                                    .to_owned())
                                                        ))
                                            }
                                        });
                                    })
                                });
                            ui.end_row();
                        }
                        if cnx_list.is_empty() {
                            ui.strong(
                                "There are no connections added. Press the 'Add Connection' button to add one.",
                            );
                            ui.end_row()
                        }

                        if let Some(i) = remove_index {
                            cnx_list.remove(i);
                            self.connection_file.write_config().expect("Failed to remove connection config from config file");
                        }
                    }
                });
            })
    }

    /// Send and receive messages from the blocking client.
    fn handle_messages(&mut self) {
        // This polling is used to stop the channel recv from blocking the UI thread.
        let waker = futures::task::noop_waker();
        let mut cx = std::task::Context::from_waker(&waker);
        if let Poll::Ready(Ok(msg)) =
        Pin::new(&mut self.client.borrow_mut().recv.recv_async()).poll(&mut cx)
        {
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
                                        if let Some(AddClientStage::ClientConfigStage(_)) =
                                        &self.add_client_stage
                                        {
                                            self.connection_config
                                                .as_mut()
                                                .unwrap()
                                                .stream_config
                                                .cal_plugin_params =
                                                setup_edcs_data.cal_option_dict.clone();
                                            self.add_client_stage =
                                                Some(AddClientStage::StreamConfigStage);
                                        } else {
                                            self.connection_stage = ConnectionStage::SetupStream;
                                        }
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
            if let Some(cfg) = &self.connection_config {
                let push = &self.client.borrow().push;
                self.pending_recv = true;
                match &self.connection_stage {
                    ConnectionStage::Connect(true) => {
                        match push.send(ChannelEdcsRequest::NewClient(cfg.client_config.clone())) {
                            Err(e) => panic!("Err sending NewClient {}", e.to_string()),
                            _ => {}
                        }
                    }
                    ConnectionStage::SetupEdcs => {
                        push.send(ChannelEdcsRequest::SetupEdcs {
                            bitrate: cfg.stream_config.bitrate,
                            framerate: cfg.stream_config.framerate,
                        })
                            .expect("Failed to push SetupEdcs");
                    }
                    ConnectionStage::SetupStream => {
                        push.send(ChannelEdcsRequest::SetupStream(
                            cfg.stream_config.cal_plugin_params.clone(),
                        ))
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

    fn paint_before_egui(&mut self, gl: Rc<glow::Context>, _window: &glutin::window::Window) {
        unsafe {
            gl.clear_color(0.1, 0.1, 0.1, 1.0);
            gl.clear(glow::COLOR_BUFFER_BIT);
        }
    }
    fn paint_after_egui(&mut self, _gl: Rc<glow::Context>, _window: &glutin::window::Window) {}

    fn handle_window_event(
        &mut self,
        _window: &Window,
        _ctrl_flow: &mut ControlFlow,
        _window_id: glutin::window::WindowId,
        _event: &glutin::event::WindowEvent,
    ) {}

    fn handle_user_event(&self, _window: &Window, _ctrl_flow: &ControlFlow, _event: &MPVEvent) {
        // Do nothing
    }

    fn needs_evloop_proxy(&mut self) -> bool {
        false
    }

    fn give_evloop_proxy(
        &mut self,
        _evloop_proxy: Rc<glutin::event_loop::EventLoopProxy<MPVEvent>>,
    ) {}
}

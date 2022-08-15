use std::{
    cell::RefCell,
    path::PathBuf,
    rc::Rc,
    sync::{Arc, RwLock},
};

use egui::{InnerResponse, RichText};
use glutin::event_loop::ControlFlow;
use log::{debug, info};

use crate::edcs_client::blocking_client::{
    BlockingEdcsClient, ChannelEdcsRequest, ChannelEdcsResponse,
};

pub struct ConnectUI {
    config_path: String,
    client: Rc<RefCell<BlockingEdcsClient>>,
}

impl ConnectUI {
    pub fn new(client: Rc<RefCell<BlockingEdcsClient>>) -> ConnectUI {
        ConnectUI {
            config_path: String::new(),
            client,
        }
    }

    pub fn render(&mut self, ui: &mut egui::Ui, ctrl_flow: &mut ControlFlow) -> InnerResponse<()> {
        egui::Frame::none()
            .fill(egui::Color32::DARK_GRAY)
            .inner_margin(10.0)
            .outer_margin(10.0)
            .show(ui, |ui| {
                ui.heading(RichText::new("EchoDawn").strong());
                ui.text_edit_singleline(&mut self.config_path);
                if ui.button("Connect").clicked() {
                    info!("Starting up client!");
                    self.client
                        .borrow()
                        .push
                        .blocking_send(ChannelEdcsRequest::NewClient(PathBuf::from(
                            &self.config_path,
                        )));
                    /*let mut client = tokio::task::spawn_blocking(async {
                        EdcClient::new(PathBuf::from(&self.config_path))
                            .await
                            .expect("Failed to init EdcClient");
                    });*/
                    info!("Client connected to server");
                }
            })
    }
}

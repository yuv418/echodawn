use std::{cell::RefCell, collections::VecDeque, rc::Rc, task::Poll};

use egui::{InnerResponse, RichText};
use glutin::event_loop::ControlFlow;

use crate::edcs_client::blocking_client::BlockingEdcsClient;

pub struct DebugArea {
    debug_messages: VecDeque<String>,
    msg_limit: usize,
}

impl DebugArea {
    pub fn new(msg_limit: usize) -> Self {
        Self {
            debug_messages: VecDeque::with_capacity(msg_limit),
            msg_limit,
        }
    }
    pub fn render(
        &mut self,
        ctx: &egui::Context,
        ctrl_flow: &mut ControlFlow,
    ) -> InnerResponse<()> {
        egui::Area::new("debug_area")
            .fixed_pos(egui::pos2(100.0, 700.0))
            .show(ctx, |ui| {
                egui::Frame::none()
                    .fill(egui::Color32::DEBUG_COLOR)
                    .inner_margin(10.0)
                    .outer_margin(10.0)
                    .show(ui, |ui| {
                        ui.heading(egui::RichText::new("Debug Area").strong());

                        let display_text = if !self.debug_messages.is_empty() {
                            self.debug_messages
                                .iter()
                                .map(|i| "------\n".to_owned() + i + "\n------\n\n")
                                .collect::<String>()
                        } else {
                            "No debug messages".to_owned()
                        };

                        ui.label(RichText::new(display_text).code().strong())
                    });
            })
    }

    pub fn push(&mut self, msg: &str) {
        // Enforce capacity
        while self.msg_limit <= self.debug_messages.len() {
            self.debug_messages.pop_back();
        }
        self.debug_messages.push_back(msg.to_owned());
    }
}

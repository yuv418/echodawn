use std::collections::VecDeque;

use egui::{InnerResponse, RichText};
use glutin::{
    event::{ElementState, VirtualKeyCode, WindowEvent},
    event_loop::ControlFlow,
};
use log::trace;

pub struct DebugArea {
    debug_messages: VecDeque<String>,
    msg_limit: usize,
    // Control, Alt, 9
    disable_modifiers: (bool, bool, bool, bool),
}

impl DebugArea {
    pub fn new(msg_limit: usize) -> Self {
        Self {
            debug_messages: VecDeque::with_capacity(msg_limit),
            msg_limit,
            disable_modifiers: (false, false, false, false),
        }
    }
    pub fn render(
        &mut self,
        ctx: &egui::Context,
        _ctrl_flow: &mut ControlFlow,
    ) -> Option<InnerResponse<()>> {
        if self.disable_modifiers.3 {
            Some(
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
                    }),
            )
        } else {
            None
        }
    }

    pub fn push(&mut self, msg: &str) {
        // Enforce capacity
        while self.msg_limit <= self.debug_messages.len() {
            self.debug_messages.pop_back();
        }
        self.debug_messages.push_back(msg.to_owned());
    }

    pub fn handle_window_event(&mut self, event: &WindowEvent) {
        // TODO dry the match branches
        let disable_modifiers_save = self.disable_modifiers.clone();
        match event {
            WindowEvent::KeyboardInput {
                device_id: _,
                input,
                is_synthetic: _,
            } => match input.virtual_keycode {
                Some(VirtualKeyCode::LControl | VirtualKeyCode::RControl)
                    if input.state == ElementState::Pressed =>
                {
                    self.disable_modifiers.0 = if disable_modifiers_save.3 {
                        false
                    } else {
                        true
                    };
                    trace!("disabled modifiers {:?}", self.disable_modifiers);
                }
                Some(VirtualKeyCode::LAlt | VirtualKeyCode::RAlt) => {
                    if (self.disable_modifiers.3 && !self.disable_modifiers.0)
                        || (!self.disable_modifiers.3 && self.disable_modifiers.0)
                    {
                        self.disable_modifiers.1 = if disable_modifiers_save.3 {
                            false
                        } else {
                            true
                        };
                        trace!("disabled modifiers {:?}", self.disable_modifiers);
                    }
                }
                Some(VirtualKeyCode::Semicolon) => {
                    if (self.disable_modifiers.3 && !self.disable_modifiers.1)
                        || (!self.disable_modifiers.3 && self.disable_modifiers.1)
                    {
                        self.disable_modifiers.2 = if disable_modifiers_save.3 {
                            false
                        } else {
                            true
                        };
                        trace!("disabled modifiers {:?}", self.disable_modifiers);
                    }
                }
                _ => {}
            },
            _ => {}
        }
        if self.disable_modifiers.0 && self.disable_modifiers.1 && self.disable_modifiers.2 {
            self.disable_modifiers.3 = true;
        } else if !self.disable_modifiers.0
            && !self.disable_modifiers.1
            && !self.disable_modifiers.2
        {
            self.disable_modifiers.3 = false;
        }
    }
}

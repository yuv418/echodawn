use std::{cell::RefCell, rc::Rc, task::Poll};

use egui::{InnerResponse, RichText};
use glutin::event_loop::ControlFlow;

use crate::edcs_client::blocking_client::BlockingEdcsClient;

pub struct DebugArea {
    blocking_client: Rc<RefCell<BlockingEdcsClient>>,
    msg: String,
}

impl DebugArea {
    pub fn new(blocking_client: Rc<RefCell<BlockingEdcsClient>>) -> Self {
        Self {
            blocking_client,
            msg: "".to_owned(),
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

                        let waker = futures::task::noop_waker();
                        let mut cx = std::task::Context::from_waker(&waker);
                        /*if let Poll::Ready(Some(msg)) =
                            self.blocking_client.borrow_mut().recv.poll_recv(&mut cx)
                        {
                            self.msg = format!("{:?}", msg);
                        }*/

                        let display_text = if !self.msg.is_empty() {
                            &self.msg
                        } else {
                            "No debug messages"
                        };

                        ui.label(RichText::new(display_text).code().strong())
                    });
            })
    }
}

use std::{cell::RefCell, rc::Rc};

use egui::InnerResponse;
use glutin::event_loop::ControlFlow;

use crate::edcs_client::blocking_client::BlockingEdcsClient;

use super::debug_area::DebugArea;

pub trait UIElement {
    fn new(client: Rc<RefCell<BlockingEdcsClient>>, debug_area: Rc<RefCell<DebugArea>>) -> Self
    where
        Self: Sized;
    fn render(&mut self, ui: &mut egui::Ui, ctrl_flow: &mut ControlFlow) -> InnerResponse<()>;
    fn handle_messages(&mut self);
    fn next_element(&mut self) -> Option<Box<dyn UIElement>>;
}

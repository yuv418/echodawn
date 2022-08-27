use std::rc::Rc;

use egui::InnerResponse;
use glutin::{
    event::WindowEvent,
    event_loop::{ControlFlow, EventLoopProxy},
    window::{Window, WindowId},
};

use super::mpv::MPVEvent;

pub trait UIElement {
    fn render_egui(&mut self) -> bool;
    fn render(&mut self, ui: &mut egui::Ui, ctrl_flow: &mut ControlFlow) -> InnerResponse<()>;
    fn handle_messages(&mut self);
    fn next_element(&mut self, window: &Window) -> Option<Box<dyn UIElement>>;
    fn needs_evloop_proxy(&mut self) -> bool;
    fn give_evloop_proxy(&mut self, evloop_proxy: Rc<EventLoopProxy<MPVEvent>>);
    fn paint_before_egui(&mut self, gl: Rc<glow::Context>, window: &Window);
    fn paint_after_egui(&mut self, gl: Rc<glow::Context>, window: &Window);
    fn handle_window_event(
        &mut self,
        window: &Window,
        ctrl_flow: &mut ControlFlow,
        window_id: WindowId,
        event: &WindowEvent,
    );
    fn handle_user_event(&self, window: &Window, ctrl_flow: &ControlFlow, event: &MPVEvent);
}

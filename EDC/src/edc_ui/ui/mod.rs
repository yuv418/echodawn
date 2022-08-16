use std::{
    cell::RefCell,
    rc::Rc,
    sync::{Arc, RwLock},
    task::Poll,
    time::Duration,
};

use egui::RichText;
use egui_glow::EguiGlow;
use glutin::{
    event::WindowEvent,
    event_loop::{ControlFlow, EventLoop, EventLoopProxy},
    window::{Window, WindowId},
};
use log::debug;

use self::{connect::ConnectUI, mpv::MPVEvent};
use crate::edcs_client::blocking_client::{self, BlockingEdcsClient};

mod connect;
mod control_bar;
mod debug_area;
pub(crate) mod mpv;
mod ui_element;

use ui_element::UIElement;

pub struct UICtx {
    egui_ctx: EguiGlow,
    debug_area: Rc<RefCell<debug_area::DebugArea>>,
    ui_element: Box<dyn UIElement>,
    blocking_client: Rc<RefCell<BlockingEdcsClient>>,
}

impl UICtx {
    pub fn new(window: &Window, gl: Rc<glow::Context>) -> UICtx {
        let blocking_client = Rc::new(RefCell::new(BlockingEdcsClient::new()));
        let debug_area = Rc::new(RefCell::new(debug_area::DebugArea::new(2)));

        UICtx {
            ui_element: Box::new(ConnectUI::new(blocking_client.clone(), debug_area.clone())),
            debug_area,
            egui_ctx: egui_glow::winit::EguiGlow::new(&window, gl.clone()),
            blocking_client,
        }
    }
    // TODO can we get the window from egui_ctx
    pub fn setup_render(&mut self, ctrl_flow: &mut ControlFlow, window: &Window) -> bool {
        self.egui_ctx.run(window, |ctx| {
            ctx.set_visuals(egui::Visuals::dark());
            let mut style = (*ctx.style()).clone();
            style.spacing.item_spacing = egui::vec2(0.0, 7.0);
            ctx.set_style(style);

            self.debug_area.borrow_mut().render(ctx, ctrl_flow);
            egui::Area::new("control_area")
                .fixed_pos(egui::pos2(100.0, 100.0))
                .show(ctx, |ui| self.ui_element.render(ui, ctrl_flow));
        })
    }

    pub fn paint(&mut self, window: &Window) {
        self.ui_element.paint_before_egui(window);
        self.egui_ctx.paint(window);
        self.ui_element.paint_after_egui(window);

        // Replace the element for the next render. We need to pass window
        // to init a new element.
        if let Some(next_ui) = self.ui_element.next_element(window) {
            self.ui_element = next_ui;
        }
    }

    // TODO move this stuff into a trait
    pub fn handle_window_event(
        &mut self,
        window: &Window,
        ctrl_flow: &mut ControlFlow,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::CloseRequested => *ctrl_flow = ControlFlow::Exit,
            _ => {}
        }

        self.egui_ctx.on_event(&event);

        self.ui_element
            .handle_window_event(window, ctrl_flow, window_id, &event);
    }

    // UserEvents are only for MPV at the moment
    pub fn handle_user_event(&self, window: &Window, ctrl_flow: &ControlFlow, event: MPVEvent) {
        self.ui_element.handle_user_event(window, ctrl_flow, &event);
    }
    pub fn needs_evloop_proxy(&mut self) -> bool {
        self.ui_element.needs_evloop_proxy()
    }
    pub fn give_evloop_proxy(&mut self, evloop_proxy: Rc<EventLoopProxy<MPVEvent>>) {
        self.ui_element.give_evloop_proxy(evloop_proxy)
    }
}

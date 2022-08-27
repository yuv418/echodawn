use std::rc::Rc;

use glow::HasContext;
use glutin::{event::Event, event_loop::ControlFlow, window::Window, PossiblyCurrent};

use crate::edc_ui::ui::{mpv::MPVEvent, UICtx};

pub struct EVLoopCtx {
    evloop: glutin::event_loop::EventLoop<MPVEvent>,
    window: glutin::ContextWrapper<PossiblyCurrent, Window>,
    gl: Rc<glow::Context>,
    ui_ctx: UICtx,
}

impl EVLoopCtx {
    pub fn new(width: u32, height: u32) -> anyhow::Result<EVLoopCtx> {
        let evloop = glutin::event_loop::EventLoop::<MPVEvent>::with_user_event();
        let window_builder = glutin::window::WindowBuilder::new()
            .with_title("Echodawn Remote Desktop Client")
            .with_inner_size(glutin::dpi::LogicalSize::new(width, height));
        let (window, gl) = unsafe {
            let window = glutin::ContextBuilder::new()
                .with_vsync(true)
                .build_windowed(window_builder, &evloop)
                .expect("Failed to build glutin window")
                .make_current()
                .expect("Failed to make window current");
            let gl =
                glow::Context::from_loader_function(|l| window.get_proc_address(l) as *const _);
            (window, Rc::new(gl))
        };

        let ui_ctx = UICtx::new(window.window(), gl.clone());
        Ok(EVLoopCtx {
            evloop,
            window,
            gl,
            ui_ctx,
        })
    }

    // https://github.com/grovesNL/glow/blob/main/examples/hello/src/main.rs
    pub fn start_loop(mut self) {
        let evloop_proxy = Rc::new(self.evloop.create_proxy());

        self.evloop.run(move |event, _, ctrl_flow| {
            *ctrl_flow = ControlFlow::Wait;

            match event {
                Event::LoopDestroyed => {
                    return;
                }
                Event::MainEventsCleared => self.window.window().request_redraw(),
                Event::RedrawRequested(_) => {
                    self.ui_ctx.setup_render(ctrl_flow, self.window.window());
                    self.ui_ctx
                        .paint_before_egui(self.gl.clone(), self.window.window());
                    self.ui_ctx.paint(self.window.window());
                    self.ui_ctx
                        .paint_after_egui(self.gl.clone(), self.window.window());

                    if self.ui_ctx.needs_evloop_proxy() {
                        self.ui_ctx.give_evloop_proxy(evloop_proxy.clone())
                    }

                    // This is required because of MPV/egui colour space problems. Perhaps put this in UICtx::paint?
                    unsafe {
                        self.gl.disable(glow::FRAMEBUFFER_SRGB);
                        self.gl.disable(glow::BLEND);
                    }

                    self.window.swap_buffers().unwrap();
                }
                Event::WindowEvent { window_id, event } => {
                    self.ui_ctx.handle_window_event(
                        self.window.window(),
                        ctrl_flow,
                        window_id,
                        event,
                    );
                    self.window.window().request_redraw();
                }
                Event::UserEvent(ue) => {
                    self.ui_ctx
                        .handle_user_event(self.window.window(), &ctrl_flow, ue)
                }
                _ => {}
            }
        });
    }
}

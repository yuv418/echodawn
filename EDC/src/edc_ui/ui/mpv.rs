use glutin::{
    event::WindowEvent,
    event_loop::{ControlFlow, EventLoop, EventLoopProxy},
    window::{Window, WindowId},
    ContextWrapper, PossiblyCurrent,
};
use libmpv_sys::*;
use log::debug;
use std::{
    ffi::{c_void, CStr},
    mem,
    os::raw::c_char,
    ptr,
};

#[derive(Debug)]
pub enum MPVEvent {
    MPVRenderUpdate,
    MPVEventUpdate,
}
#[derive(Debug)]
pub struct MPVCtx {
    mpv: *mut mpv_handle,
    mpv_gl: *mut mpv_render_context,
    width: u32,
    height: u32,
}

pub unsafe extern "C" fn get_proc_addr(ctx: *mut c_void, name: *const c_char) -> *mut c_void {
    let rust_name = CStr::from_ptr(name).to_str().unwrap();
    let window: &ContextWrapper<PossiblyCurrent, Window> = std::mem::transmute(ctx);
    let addr = window.get_proc_address(rust_name) as *mut _;
    addr
}
pub unsafe extern "C" fn on_mpv_event(ctx: *mut c_void) {
    let event_proxy: &EventLoopProxy<MPVEvent> = mem::transmute(ctx);
    event_proxy
        .send_event(MPVEvent::MPVEventUpdate)
        .expect("Failed to send event update to render loop");
}
pub unsafe extern "C" fn on_mpv_render_update(ctx: *mut c_void) {
    let event_proxy: &EventLoopProxy<MPVEvent> = mem::transmute(ctx);
    event_proxy
        .send_event(MPVEvent::MPVRenderUpdate)
        .expect("Failed to send render update to render loop");
}

impl MPVCtx {
    pub fn new(
        window: &Window,
        evloop: &EventLoop<MPVEvent>,
        width: u32,
        height: u32,
        debug: bool,
        sdp: String,
    ) -> anyhow::Result<MPVCtx> {
        let mpv = unsafe { mpv_create() };
        assert!(!mpv.is_null(), "MPV failed to create!");

        let mut mpv_gl: *mut mpv_render_context = ptr::null_mut();

        // Only set this when debug is set to true

        unsafe {
            if debug {
                let mut loglv = "debug\0".to_owned();
                let c_loglv = loglv.as_mut_ptr() as *mut _;
                mpv_request_log_messages(mpv, c_loglv);
            }
            assert!(mpv_initialize(mpv) == 0, "MPV failed to initialise!");
            let mut mpv_render_params = vec![
                mpv_render_param {
                    type_: mpv_render_param_type_MPV_RENDER_PARAM_API_TYPE,
                    data: mem::transmute(MPV_RENDER_API_TYPE_OPENGL),
                },
                mpv_render_param {
                    type_: mpv_render_param_type_MPV_RENDER_PARAM_OPENGL_INIT_PARAMS,
                    data: mem::transmute(&mut mpv_opengl_init_params {
                        get_proc_address: Some(get_proc_addr),
                        get_proc_address_ctx: mem::transmute(&window),
                        extra_exts: ptr::null(),
                    }),
                },
                mpv_render_param {
                    type_: mpv_render_param_type_MPV_RENDER_PARAM_ADVANCED_CONTROL,
                    data: mem::transmute(&mut 1),
                },
                mpv_render_param {
                    // end of params??
                    type_: 0,
                    data: ptr::null_mut(),
                },
            ];

            assert!(
                mpv_render_context_create(&mut mpv_gl, mpv, mpv_render_params.as_mut_ptr()) == 0,
                "MPV failed to create the render context!"
            )
        };

        let event_proxy = evloop.create_proxy();

        // Setup wakeup callback

        unsafe {
            mpv_set_wakeup_callback(mpv, Some(on_mpv_event), std::mem::transmute(&event_proxy));
            // Setup update callback
            mpv_render_context_set_update_callback(
                mpv_gl,
                Some(on_mpv_render_update),
                std::mem::transmute(&event_proxy),
            )
        }

        // SDP handling goes here

        Ok(MPVCtx {
            mpv,
            mpv_gl,
            width,
            height,
        })
    }

    pub fn paint(&self, window: &Window) {
        let mut mpv_render_params = unsafe {
            vec![
                mpv_render_param {
                    type_: mpv_render_param_type_MPV_RENDER_PARAM_OPENGL_FBO,
                    data: mem::transmute(&mut mpv_opengl_fbo {
                        fbo: 0,
                        w: self.width as i32,
                        h: self.height as i32,
                        internal_format: 0,
                    }),
                },
                // Why does MPV render upside down by default ):
                mpv_render_param {
                    type_: mpv_render_param_type_MPV_RENDER_PARAM_FLIP_Y,
                    data: mem::transmute(&mut 1),
                },
                mpv_render_param {
                    type_: mpv_render_param_type_MPV_RENDER_PARAM_ADVANCED_CONTROL,
                    data: mem::transmute(&mut 1),
                },
                mpv_render_param {
                    // end??
                    type_: 0,
                    data: ptr::null_mut(),
                },
            ]
        };
        unsafe {
            mpv_render_context_render(self.mpv_gl, mpv_render_params.as_mut_ptr());
        }
    }
    pub fn handle_window_event(&self, window_id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => unsafe {
                mpv_render_context_free(self.mpv_gl);
                mpv_detach_destroy(self.mpv);
            },
            _ => {}
        }
    }

    pub fn handle_user_event(&self, window: &Window, ctrl_flow: &ControlFlow, event: MPVEvent) {
        match event {
            MPVEvent::MPVRenderUpdate => {
                unsafe {
                    mpv_render_context_update(self.mpv_gl);
                }
                window.request_redraw();
            }
            MPVEvent::MPVEventUpdate => loop {
                let mpv_event = unsafe { mpv_wait_event(self.mpv, 0.0) };
                match unsafe { (*mpv_event).event_id } {
                    mpv_event_id_MPV_EVENT_NONE => break,
                    mpv_event_id_MPV_EVENT_LOG_MESSAGE => {
                        let text: &mpv_event_log_message =
                            unsafe { std::mem::transmute((*mpv_event).data) };
                        debug!("mpv_log {}", unsafe {
                            CStr::from_ptr(text.text).to_str().unwrap()
                        });
                    }
                    _ => {}
                }
                unsafe {
                    debug!(
                        "mpv_event {}",
                        CStr::from_ptr(mpv_event_name((*mpv_event).event_id))
                            .to_str()
                            .unwrap()
                    )
                }
            },
        };
    }
}

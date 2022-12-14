use glutin::{
    event::WindowEvent,
    event_loop::{ControlFlow, EventLoopProxy},
    window::{Window, WindowId},
    ContextWrapper, PossiblyCurrent,
};
use libmpv_sys::*;
use log::debug;
use std::{
    ffi::{c_void, CStr, CString},
    mem,
    os::raw::c_char,
    ptr,
    rc::Rc,
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
    evloop_proxy: Option<Rc<EventLoopProxy<MPVEvent>>>,
    mpv_ogl_fbo: mpv_opengl_fbo,
    mpv_render_params: Vec<mpv_render_param>,
}

pub unsafe extern "C" fn get_proc_addr(ctx: *mut c_void, name: *const c_char) -> *mut c_void {
    let rust_name = CStr::from_ptr(name).to_str().unwrap();
    // I doubt this is going to work
    let window: &ContextWrapper<PossiblyCurrent, Window> = std::mem::transmute(ctx);
    let addr = window.get_proc_address(rust_name) as *mut _;
    addr
}
pub unsafe extern "C" fn on_mpv_event(ctx: *mut c_void) {
    let event_proxy: &EventLoopProxy<MPVEvent> = mem::transmute(ctx);
    debug!("render_event");
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
    pub fn mpv_set_opt(mpv: *mut mpv_handle, key: &str, val: &str) -> i32 {
        let prop = CString::new(key).expect("Failed to set mpv prop key");
        let val = CString::new(val).expect("Failed to set mpv prop val");

        unsafe { mpv_set_option_string(mpv, prop.as_ptr(), val.as_ptr()) }
    }
    pub fn new(
        window: &Window,
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
                let loglv = CString::new("debug").expect("Failed to create loglevel string");
                mpv_request_log_messages(mpv, loglv.as_ptr());
            }

            Self::mpv_set_opt(mpv, "profile", "low-latency");
            Self::mpv_set_opt(mpv, "rtsp-transport", "lavc");
            Self::mpv_set_opt(mpv, "video-latency-hacks", "yes");
            Self::mpv_set_opt(mpv, "drop-buffers", "yes");
            Self::mpv_set_opt(mpv, "vd-lavc-threads", "1");
            Self::mpv_set_opt(mpv, "no-cache", "yes");
            Self::mpv_set_opt(mpv, "untimed", "yes");
            Self::mpv_set_opt(mpv, "hwdec", "yes");

            assert!(mpv_initialize(mpv) == 0, "MPV failed to initialise!");
            let ogl_init_params = mpv_opengl_init_params {
                get_proc_address: Some(get_proc_addr),
                get_proc_address_ctx: mem::transmute(window),
                extra_exts: ptr::null(),
            };
            let mut ctr = [1];
            let mut mpv_render_params = vec![
                mpv_render_param {
                    type_: mpv_render_param_type_MPV_RENDER_PARAM_API_TYPE,
                    data: mem::transmute(MPV_RENDER_API_TYPE_OPENGL),
                },
                mpv_render_param {
                    type_: mpv_render_param_type_MPV_RENDER_PARAM_OPENGL_INIT_PARAMS,
                    data: mem::transmute(&ogl_init_params),
                },
                mpv_render_param {
                    type_: mpv_render_param_type_MPV_RENDER_PARAM_ADVANCED_CONTROL,
                    data: mem::transmute(ctr.as_mut_ptr()),
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
            );
        };
        let mut mpd_cmd_args: Vec<*const c_char> = vec![
            "loadfile\0".as_ptr() as _,
            CString::new("memory://".to_owned() + &sdp)
                .unwrap()
                .into_raw(),
            ptr::null(),
        ];
        unsafe { mpv_command_async(mpv, 0, mpd_cmd_args.as_mut_ptr() as *mut *const _) };

        let mpv_ogl_fbo = mpv_opengl_fbo {
            fbo: 0,
            w: width as i32,
            h: height as i32,
            internal_format: 0,
        };

        Ok(MPVCtx {
            mpv,
            mpv_gl,
            width,
            height,
            evloop_proxy: None,
            mpv_ogl_fbo,
            mpv_render_params: unsafe {
                vec![
                    mpv_render_param {
                        type_: mpv_render_param_type_MPV_RENDER_PARAM_OPENGL_FBO,
                        data: mem::transmute(&mpv_ogl_fbo),
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
            },
        })
    }

    pub fn paint(&mut self, _window: &Window) {
        unsafe {
            let mut y = [1];
            let mut ctr = [1];
            self.mpv_render_params[0].data = mem::transmute(&self.mpv_ogl_fbo);
            self.mpv_render_params[1].data = mem::transmute(y.as_mut_ptr());
            self.mpv_render_params[2].data = mem::transmute(ctr.as_mut_ptr());
            mpv_render_context_render(self.mpv_gl, self.mpv_render_params.as_mut_ptr());
        }
    }
    pub fn handle_window_event(&self, _window_id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => unsafe {
                mpv_render_context_free(self.mpv_gl);
                mpv_detach_destroy(self.mpv);
            },
            _ => {}
        }
    }

    pub fn handle_user_event(&self, window: &Window, _ctrl_flow: &ControlFlow, event: &MPVEvent) {
        match event {
            MPVEvent::MPVRenderUpdate => {
                let flags = unsafe { mpv_render_context_update(self.mpv_gl) };
                if (flags & mpv_render_update_flag_MPV_RENDER_UPDATE_FRAME as u64) != 0 {
                    window.request_redraw();
                }
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
    pub fn needs_evloop_proxy(&mut self) -> bool {
        self.evloop_proxy.is_none()
    }
    pub fn give_evloop_proxy(&mut self, evloop_proxy: Rc<EventLoopProxy<MPVEvent>>) {
        // Setup wakeup callback

        // This way, the proxy does not get dropped.
        // TODO is this really necessary if we are behind an Rc?
        self.evloop_proxy = Some(evloop_proxy);

        unsafe {
            mpv_set_wakeup_callback(
                self.mpv,
                Some(on_mpv_event),
                Rc::as_ptr(self.evloop_proxy.as_ref().unwrap()) as *mut _,
            );
            // Setup update callback
            mpv_render_context_set_update_callback(
                self.mpv_gl,
                Some(on_mpv_render_update),
                Rc::as_ptr(self.evloop_proxy.as_ref().unwrap()) as *mut _,
            );
        }
    }
}

use log::{debug, info, trace};
use rand::RngCore;

use super::edss_unsafe;
use crate::edcs_server::edcs_proto::{
    edcs_mouse_event, EdcsKeyData, EdcsKeyboardEvent, EdcsMouseButton, EdcsMouseEvent,
};
use rand::rngs::OsRng;
use std::collections::HashMap;
use std::ffi::CStr;
use std::net::SocketAddr;
use std::os::raw::{c_char, c_void};

pub struct EdssError(pub edss_unsafe::EDSS_STATUS);

#[derive(Debug)]
pub struct EdssAdapter {
    pub ip: SocketAddr,
    pub port: u16,
    pub bitrate: u32,
    pub framerate: u32,
    pub srtp_out_params: String, // Maximum length is 32
    pub cal_option_dict: HashMap<String, String>,
    pub sdp: Option<String>, // Only Some if init_server was called
    streaming: bool,
    stream_setup: bool,
}

union MouseData {
    key_code: u32,
    coords: (u32, u32),
}

impl EdssAdapter {
    // I'm sure there is some standardised method of doing this, but I guess
    // I'm not really doing that.

    // Prevent directly writing to this field, but allow access still.
    pub fn streaming(&self) -> bool {
        self.streaming
    }
    // Prevent directly writing to this field, but allow access still.
    pub fn stream_setup(&self) -> bool {
        self.stream_setup
    }

    fn to_c_struct(&self) -> edss_unsafe::edssConfig_t {
        // C requires the octets to be treated as little endian
        unsafe {
            let str_map = edss_unsafe::sm_new(self.cal_option_dict.len().try_into().unwrap());
            debug!("Begin conversion to C struct");
            for key in self.cal_option_dict.keys() {
                let key_c = key.to_owned() + "\0";
                let val_c = self.cal_option_dict[key].to_owned() + "\0";
                let ret = edss_unsafe::sm_put(
                    str_map,
                    key_c.as_ptr() as *const c_char,
                    val_c.as_ptr() as *const c_char,
                );
                debug!("sm_put returns {}, key_c {}, val_c {}", ret, key_c, val_c);
            }

            // Big brain: just loop through string chars and push it in
            let mut srtp_out_params_c: [c_char; 41] = ['\0' as c_char; 41];
            for (i, ch) in self.srtp_out_params[..40].chars().enumerate() {
                srtp_out_params_c[i] = ch as c_char;
            }

            let mut socket_addr_c: [c_char; 46] = ['\0' as c_char; 46];
            for (i, ch) in self.ip.to_string().chars().enumerate() {
                socket_addr_c[i] = ch as c_char;
            }

            edss_unsafe::edssConfig_t {
                socketAddr: socket_addr_c,
                port: self.port,
                bitrate: self.bitrate,
                framerate: self.framerate,
                srtpOutParams: srtp_out_params_c,
                calOptionDict: str_map,
            }
        }
    }

    extern "C" fn strmap_enum_callback(
        key: *const c_char,
        value: *const c_char,
        p_hash_map: *const c_void,
    ) {
        trace!("In strmap_enum_callback");
        let hash_map: &mut HashMap<String, String> = unsafe { std::mem::transmute(p_hash_map) };
        let key_rs = unsafe { CStr::from_ptr(key) }
            .to_str()
            .expect("Failed to get strmap key")
            .to_owned();
        let value_rs = unsafe { CStr::from_ptr(value) }
            .to_str()
            .expect("Failed to get strmap value")
            .to_owned();
        hash_map.insert(key_rs, value_rs);
    }

    fn strmap_to_hashmap(
        in_map: *mut edss_unsafe::StrMap,
    ) -> anyhow::Result<HashMap<String, String>, EdssError> {
        trace!("In strmap_to_hashmap");
        let hash_map: HashMap<String, String> = HashMap::new();
        let hash_map_void: *const c_void = unsafe { std::mem::transmute(&hash_map) };
        unsafe {
            if edss_unsafe::sm_enum(in_map, Some(Self::strmap_enum_callback), hash_map_void) < 1 {
                return Err(EdssError(edss_unsafe::EDSS_STATUS_EDSS_STRMAP_FAILURE));
                // TODO, is this really the best error for us to return?
            }
        };
        Ok(hash_map)
    }

    pub fn new(
        mut plugin_name: String,
        ip: SocketAddr,
        port: u16,
        bitrate: u32,
        framerate: u32,
    ) -> anyhow::Result<Self, EdssError> {
        let config = unsafe {
            plugin_name += "\0"; // If you don't do this, the strings will become garbled.

            let mut config: *mut edss_unsafe::StrMap = std::ptr::null_mut();

            trace!("strmap address is {:p}", config);
            let cal_open_result =
                edss_unsafe::edssOpenCAL(plugin_name.as_ptr() as *mut c_char, &mut config);
            if cal_open_result != edss_unsafe::EDSS_STATUS_EDSS_OK {
                return Err(EdssError(cal_open_result));
            }
            trace!("strmap address is {:p}", config);
            config
        };

        // Generate a random 40-digit base64;
        let mut srtp_out_params_buf = [0u8; 40];
        OsRng.fill_bytes(&mut srtp_out_params_buf);
        let srtp_out_params = base64::encode(&srtp_out_params_buf);

        Ok(Self {
            ip,
            port,
            bitrate,
            framerate,
            srtp_out_params,
            cal_option_dict: Self::strmap_to_hashmap(config)?,
            sdp: None,
            streaming: false,
            stream_setup: false,
        })
    }
    // TODO implement more robust error handling from these functions
    pub fn init_server(&mut self) -> Result<(), EdssError> {
        unsafe {
            let mut sdp_cstr: *mut c_char = std::ptr::null_mut();
            let result =
                edss_unsafe::edssInitServer(&mut self.to_c_struct() as *mut _, &mut sdp_cstr);
            if result != edss_unsafe::EDSS_STATUS_EDSS_OK {
                return Err(EdssError(result));
            }
            self.sdp = Some(
                CStr::from_ptr(sdp_cstr)
                    .to_str()
                    .expect("Invalid SDP from EDSS")
                    .to_owned(),
            );
            self.stream_setup = true;

            trace!("EdcsAdapter SDP field:\n{}", self.sdp.as_ref().unwrap());
        }
        Ok(())
    }
    pub fn init_streaming(&mut self) -> Result<(), EdssError> {
        unsafe {
            edss_unsafe::edssInitStreaming();
            self.streaming = true;
        }
        Ok(())
    }
    pub fn close_streaming(&mut self) -> Result<(), EdssError> {
        unsafe {
            edss_unsafe::edssCloseStreaming();
            self.streaming = false;
            self.stream_setup = false;
            // TODO destroy all stream variables
        }
        Ok(())
    }
    pub fn write_mouse_event(&mut self, ev: EdcsMouseEvent) -> Result<(), EdssError> {
        let mut edss_event = match ev.payload {
            Some(edcs_mouse_event::Payload::Button(EdcsKeyData { btn_typ, pressed })) => {
                edss_unsafe::edssMouseEvent_t {
                    type_: edss_unsafe::edssMouseEventType_t_CLICK,
                    payload: edss_unsafe::edssMouseEvent_t__bindgen_ty_1 {
                        button: edss_unsafe::edssKeyData_t {
                            pressed,
                            button: match EdcsMouseButton::from_i32(btn_typ) {
                                Some(EdcsMouseButton::MouseButtonLeft) => {
                                    input_event_codes::BTN_LEFT!()
                                }
                                Some(EdcsMouseButton::MouseButtonRight) => {
                                    input_event_codes::BTN_RIGHT!()
                                }
                                Some(EdcsMouseButton::MouseButtonMiddle) => {
                                    input_event_codes::BTN_MIDDLE!()
                                }
                                _ => {
                                    return Err(EdssError(
                                        edss_unsafe::EDSS_STATUS_EDSS_INVALID_MOUSE_DATA,
                                    ))
                                }
                            },
                        },
                    },
                }
            }
            Some(edcs_mouse_event::Payload::Move(m)) => edss_unsafe::edssMouseEvent_t {
                type_: edss_unsafe::edssMouseEventType_t_MOVE,

                payload: edss_unsafe::edssMouseEvent_t__bindgen_ty_1 {
                    move_: edss_unsafe::edssMouseEvent_t__bindgen_ty_1_move { x: m.x, y: m.y },
                },
            },
            _ => return Err(EdssError(edss_unsafe::EDSS_STATUS_EDSS_INVALID_MOUSE_DATA)),
        };
        unsafe {
            edss_unsafe::edssWriteMouseEvent(&mut edss_event as *mut _);
            // TODO destroy all stream variables
        }
        Ok(())
    }

    pub fn write_keyboard_event(&mut self, kev: EdcsKeyboardEvent) -> Result<(), EdssError> {
        let key_dat = kev.key_dat.unwrap();
        let mut kev_c = edss_unsafe::edssKeyboardEvent_t {
            keyData: edss_unsafe::edssKeyData_t {
                button: key_dat.btn_typ,
                pressed: key_dat.pressed,
            },
        };
        unsafe {
            edss_unsafe::edssWriteKeyboardEvent(&mut kev_c as *mut _);
        }
        Ok(())
    }

    pub fn update_streaming(&self) -> Result<(), EdssError> {
        unsafe {
            edss_unsafe::edssUpdateStreaming(&mut self.to_c_struct() as *mut _);
        }
        Ok(())
    }
}

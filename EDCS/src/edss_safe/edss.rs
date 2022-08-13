use log::{debug, info, trace};
use rand::RngCore;

use super::edss_unsafe;
use rand::rngs::OsRng;
use std::collections::HashMap;
use std::ffi::CStr;
use std::net::Ipv4Addr;
use std::os::raw::{c_char, c_void};

pub struct EdssError(pub edss_unsafe::EDSS_STATUS);

#[derive(Debug)]
pub struct EdssAdapter {
    pub ip: Ipv4Addr,
    pub port: u16,
    pub bitrate: u32,
    pub framerate: u32,
    pub srtp_out_params: String, // Maximum length is 32
    pub cal_option_dict: HashMap<String, String>,
    pub sdp: Option<String>, // Only Some if init_server was called
    streaming: bool,
    stream_setup: bool,
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
        let ip_int = u32::from_le_bytes(self.ip.octets());
        unsafe {
            let str_map = edss_unsafe::sm_new(self.cal_option_dict.len().try_into().unwrap());
            debug!("Begin conversion to C struct");
            for key in self.cal_option_dict.keys() {
                let key_c = key.to_owned() + "\0";
                let val_c = self.cal_option_dict[key].to_owned() + "\0";
                debug!(
                    "sm_put returns {}, key_c {}, val_c {}",
                    edss_unsafe::sm_put(
                        str_map,
                        key_c.as_ptr() as *const c_char,
                        val_c.as_ptr() as *const c_char,
                    ),
                    key_c,
                    val_c
                );
            }

            // Big brain: just loop through string chars and push it in
            let mut srtp_out_params_c: [c_char; 41] = ['\0' as c_char; 41];
            for (i, ch) in self.srtp_out_params[..40].chars().enumerate() {
                srtp_out_params_c[i] = ch as c_char;
            }

            edss_unsafe::edssConfig_t {
                ip: ip_int,
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
        ip: Ipv4Addr,
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
            // TODO destroy all stream variables
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

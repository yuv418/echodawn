use super::edss_unsafe;
use std::collections::HashMap;
use std::net::Ipv4Addr;

struct EdssError(edss_unsafe::EDSS_STATUS);

pub struct EdssConfig {
    ip: Ipv4Addr,
    port: u16,
    bitrate: u32,
    srtp_out_params: String, // Maximum length is 32
    cal_option_dict: HashMap<String, String>,
}

fn edss_init_server(cfg: &EdssConfig) -> Result<(), EdssError> {
    // TODO convert EdssConfig to edssConfig_t
    unsafe {
        edss_unsafe::edssInitServer();
    }
    Ok(())
}
fn edss_init_streaming() -> Result<(), EdssError> {
    Ok(())
}
fn edss_close_streaming() -> Result<(), EdssError> {
    Ok(())
}
fn edss_update_streaming() -> Result<(), EdssError> {
    Ok(())
}
fn edss_open_cal() -> Result<(), EdssError> {
    Ok(())
}

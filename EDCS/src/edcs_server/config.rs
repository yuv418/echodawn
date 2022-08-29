use std::{net::Ipv4Addr, path::PathBuf};

use serde::Deserialize;

#[derive(Deserialize)]
pub struct EdcsConfig {
    pub ip: Ipv4Addr,
    pub port: u16,
    pub cert_path: PathBuf,
    pub key_path: PathBuf,
    pub edss_config: EdssConfig,
}

#[derive(Deserialize)]
pub struct EdssConfig {
    pub plugin_name: String,
    pub port: u16,
}

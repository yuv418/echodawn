use std::{collections::HashMap, net::Ipv4Addr, path::PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct ClientConfig {
    pub host: Ipv4Addr,
    pub domain: String,
    pub port: u16,
    pub cert: PathBuf,
    pub disable_tls_verification: bool,
}

#[derive(Debug, Serialize, PartialEq, Deserialize, Clone)]
pub struct UIConfig {
    pub host_cursor: bool,
}

#[derive(Debug, Serialize, PartialEq, Deserialize, Clone)]
pub struct StreamConfig {
    pub bitrate: u32,
    pub framerate: u32,
    pub cal_plugin_params: HashMap<String, String>,
}

pub mod connection_config;
pub use connection_config::*;

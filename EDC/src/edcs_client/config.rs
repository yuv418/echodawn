use std::{net::Ipv4Addr, path::PathBuf};

use serde::Deserialize;

#[derive(Deserialize)]
pub struct ClientConfig {
    pub host: Ipv4Addr,
    pub domain: String,
    pub port: u16,
    pub cert: PathBuf,
    pub disable_tls_verification: bool,
    pub hostcursor: bool,
}

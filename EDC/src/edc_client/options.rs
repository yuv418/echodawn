use std::{net::Ipv4Addr, path::PathBuf};

use serde::Deserialize;

#[derive(Deserialize)]
pub struct ClientOptions {
    pub host: Ipv4Addr,
    pub port: u16,
    pub cert: PathBuf,
}

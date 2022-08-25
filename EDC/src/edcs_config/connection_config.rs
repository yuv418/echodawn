use log::warn;
use serde::{Deserialize, Serialize};
use std::{
    fs::{File, OpenOptions},
    io::{Read, Seek, Write},
};

use super::{ClientConfig, StreamConfig, UIConfig};
use platform_dirs::AppDirs;

pub struct ConnectionFile {
    cfg_file: File,
    stream_configs: Option<ConnectionConfigs>,
}

#[derive(Debug, Serialize, PartialEq, Deserialize, Clone)]
pub struct ConnectionConfig {
    pub stream_config: StreamConfig,
    pub client_config: ClientConfig,
    pub ui_config: UIConfig,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ConnectionConfigs {
    stream_configs: Vec<ConnectionConfig>,
}

impl ConnectionFile {
    pub fn open() -> anyhow::Result<Self> {
        // TODO make this an error
        let app_dirs = AppDirs::new(Some("echodawn-client"), true).unwrap();

        if !app_dirs.config_dir.exists() {
            std::fs::create_dir_all(&app_dirs.config_dir)?;
        }

        let config_file_path = app_dirs.config_dir.join("edcCfg.toml");
        let cfg_file = OpenOptions::new()
            .create(true)
            .write(true)
            .append(false)
            .read(true)
            .open(config_file_path)?;
        Ok(Self {
            cfg_file,
            stream_configs: None,
        })
    }

    fn setup_file(&mut self) -> anyhow::Result<()> {
        let mut cfg_str = String::new();
        self.cfg_file.read_to_string(&mut cfg_str)?;
        match toml::from_str(&cfg_str) {
            Err(e) => {
                warn!("TOML parse error {:?}", e);
                // TODO: Do we want to wipe out the config for all types of errors?
                self.stream_configs = Some(ConnectionConfigs {
                    stream_configs: vec![],
                });
                self.write_config()?;
            }
            Ok(cfg) => self.stream_configs = Some(cfg),
        }

        Ok(())
    }

    pub fn write_config(&mut self) -> anyhow::Result<()> {
        self.cfg_file.rewind()?;
        self.cfg_file
            .write_all(toml::to_string(&self.stream_configs.as_ref().unwrap())?.as_bytes())?;
        Ok(())
    }

    pub fn connection_config_ref(&mut self) -> anyhow::Result<&mut Vec<ConnectionConfig>> {
        if let None = &mut self.stream_configs {
            self.setup_file()?;
        }
        Ok(&mut self.stream_configs.as_mut().unwrap().stream_configs)
    }
}

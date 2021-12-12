use directories_next::ProjectDirs;
use miette::{IntoDiagnostic, Result, WrapErr};
use serde::{Deserialize, Serialize};
use serde_hex::{Compact, SerHex};
use serialport::UsbPortInfo;
use std::fs::{create_dir_all, read, write};
use std::path::PathBuf;

#[derive(Debug, Deserialize, Serialize, Default, Clone)]
pub struct Config {
    #[serde(default)]
    pub connection: Connection,
    #[serde(default)]
    pub usb_device: Vec<UsbDevice>,
    #[serde(skip)]
    save_path: PathBuf,
}

#[derive(Debug, Deserialize, Serialize, Default, Clone)]
pub struct Connection {
    pub serial: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Default, Clone)]
pub struct UsbDevice {
    #[serde(with = "SerHex::<Compact>")]
    pub vid: u16,
    #[serde(with = "SerHex::<Compact>")]
    pub pid: u16,
}

impl UsbDevice {
    pub fn matches(&self, port: &UsbPortInfo) -> bool {
        self.vid == port.vid && self.pid == port.pid
    }
}

impl Config {
    /// Load the config from config file
    pub fn load() -> Result<Self> {
        let dirs = ProjectDirs::from("rs", "esp", "espflash").unwrap();
        let file = dirs.config_dir().join("espflash.toml");

        let mut config = if let Ok(data) = read(&file) {
            toml::from_slice(&data).into_diagnostic()?
        } else {
            Self::default()
        };
        config.save_path = file;
        Ok(config)
    }

    pub fn save_with<F: Fn(&mut Self)>(&self, modify_fn: F) -> Result<()> {
        let mut copy = self.clone();
        modify_fn(&mut copy);

        let serialized = toml::to_string(&copy)
            .into_diagnostic()
            .wrap_err("Failed to serialize config")?;
        create_dir_all(self.save_path.parent().unwrap())
            .into_diagnostic()
            .wrap_err("Failed to create config directory")?;
        write(&self.save_path, serialized)
            .into_diagnostic()
            .wrap_err_with(|| format!("Failed to write config to {}", self.save_path.display()))
    }
}

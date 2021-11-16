use std::fs::read;

use directories_next::ProjectDirs;
use serde::Deserialize;

#[derive(Debug, Deserialize, Default)]
pub struct Config {
    #[serde(default)]
    pub connection: Connection,
    #[serde(default)]
    pub usb_device: UsbDevice,
}

#[derive(Debug, Deserialize, Default)]
pub struct Connection {
    pub serial: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
pub struct UsbDevice {
    pub vid: Option<u16>,
    pub pid: Option<u16>,
}

impl Config {
    /// Load the config from config file
    pub fn load() -> Self {
        let dirs = ProjectDirs::from("rs", "esp", "espflash").unwrap();
        let file = dirs.config_dir().join("espflash.toml");

        if let Ok(data) = read(&file) {
            toml::from_slice(&data).unwrap()
        } else {
            Self::default()
        }
    }
}

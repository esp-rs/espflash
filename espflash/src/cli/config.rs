use std::fs::read;

use directories_next::ProjectDirs;
use serde::Deserialize;
use serialport::UsbPortInfo;

#[derive(Debug, Deserialize, Default)]
pub struct Config {
    #[serde(default)]
    pub connection: Connection,
    #[serde(default)]
    pub usb_device: Vec<UsbDevice>,
}

#[derive(Debug, Deserialize, Default)]
pub struct Connection {
    pub serial: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
pub struct UsbDevice {
    pub vid: u16,
    pub pid: u16,
}

impl UsbDevice {
    pub fn matches(&self, port: &UsbPortInfo) -> bool {
        self.vid == port.vid && self.pid == port.pid
    }
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

//! Command-line interface configuration
//!
//! Both [cargo-espflash] and [espflash] allow for the use of configuration
//! files; the [Config] type handles the loading and saving of this
//! configuration file.
//!
//! [cargo-espflash]: https://crates.io/crates/cargo-espflash
//! [espflash]: https://crates.io/crates/espflash

use std::{
    fs::{create_dir_all, read, write},
    path::PathBuf,
};

use directories_next::ProjectDirs;
use miette::{IntoDiagnostic, Result, WrapErr};
use serde::{Deserialize, Serialize};
use serde_hex::{Compact, SerHex};
use serialport::UsbPortInfo;

/// A configured, known serial connection
#[derive(Debug, Deserialize, Serialize, Default, Clone)]
pub struct Connection {
    /// Name of the serial port used for communication
    pub serial: Option<String>,
    /// Data Transmit Ready pin
    #[cfg(feature = "raspberry")]
    pub dtr: Option<u8>,
    /// Ready To Send pin
    #[cfg(feature = "raspberry")]
    pub rts: Option<u8>,
}

/// A configured, known USB device
#[derive(Debug, Deserialize, Serialize, Default, Clone)]
pub struct UsbDevice {
    /// USB Vendor ID
    #[serde(with = "SerHex::<Compact>")]
    pub vid: u16,
    /// USB Product ID
    #[serde(with = "SerHex::<Compact>")]
    pub pid: u16,
}

impl UsbDevice {
    pub fn matches(&self, port: &UsbPortInfo) -> bool {
        self.vid == port.vid && self.pid == port.pid
    }
}

/// Deserialized contents of a configuration file
#[derive(Debug, Deserialize, Serialize, Default, Clone)]
pub struct Config {
    /// Preferred serial port connection information
    #[serde(default)]
    pub connection: Connection,
    /// Preferred USB devices
    #[serde(default)]
    pub usb_device: Vec<UsbDevice>,
    #[serde(skip)]
    save_path: PathBuf,
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

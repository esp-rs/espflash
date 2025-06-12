//! Command-line interface configuration
//!
//! Both [cargo-espflash] and [espflash] allow for the use of configuration
//! files; the [Config] type handles the loading and saving of this
//! configuration file.
//!
//! [cargo-espflash]: https://crates.io/crates/cargo-espflash
//! [espflash]: https://crates.io/crates/espflash

use std::{
    ffi::OsStr,
    fs::{create_dir_all, read_to_string, write},
    path::PathBuf,
};

use directories::ProjectDirs;
use log::debug;
use miette::{IntoDiagnostic, Result, WrapErr};
use serde::{Deserialize, Serialize};
use serialport::UsbPortInfo;

use crate::{Error, cli, flasher::FlashSettings, image_format::ImageFormatKind};

/// A configured, known serial connection
#[derive(Debug, Deserialize, Serialize, Default, Clone)]
pub struct Connection {
    /// Name of the serial port used for communication
    pub serial: Option<String>,
}

/// A configured, known USB device
#[derive(Debug, Deserialize, Serialize, Default, Clone)]
pub struct UsbDevice {
    /// USB Vendor ID
    #[serde(
        serialize_with = "serialize_u16_to_hex",
        deserialize_with = "deserialize_hex_to_u16"
    )]
    pub vid: u16,
    /// USB Product ID
    #[serde(
        serialize_with = "serialize_u16_to_hex",
        deserialize_with = "deserialize_hex_to_u16"
    )]
    pub pid: u16,
}

fn deserialize_hex_to_u16<'de, D>(deserializer: D) -> Result<u16, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let hex = String::deserialize(deserializer)?.to_lowercase();
    let hex = hex.trim_start_matches("0x");

    let int = u16::from_str_radix(hex, 16).map_err(serde::de::Error::custom)?;

    Ok(int)
}

fn serialize_u16_to_hex<S>(decimal: &u16, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    let hex_string = format!("{:04x}", decimal);
    serializer.serialize_str(&hex_string)
}

impl UsbDevice {
    /// Check if the given USB port matches this device
    pub fn matches(&self, port: &UsbPortInfo) -> bool {
        self.vid == port.vid && self.pid == port.pid
    }
}

/// Configuration for the project and the port
#[derive(Debug, Deserialize, Serialize, Default, Clone)]
pub struct Config {
    /// Project configuration
    pub project_config: ProjectConfig,
    /// Port configuration
    pub port_config: PortConfig,
}

/// Project configuration
#[derive(Debug, Deserialize, Serialize, Default, Clone)]
pub struct ProjectConfig {
    /// Baudrate
    #[serde(default)]
    pub baudrate: Option<u32>,
    /// Image format
    #[serde(default)]
    pub format: ImageFormatKind,
    /// ESP-IDF format arguments
    #[serde(default)]
    pub esp_idf_format_args: cli::EspIdfFormatArgs,
    /// Flash settings
    #[serde(default)]
    pub flash: FlashSettings,
}

/// Serial port configuration
#[derive(Debug, Deserialize, Serialize, Default, Clone)]
pub struct PortConfig {
    /// Preferred serial port connection information
    #[serde(default)]
    pub connection: Connection,
    /// Preferred USB devices
    #[serde(default)]
    pub usb_device: Vec<UsbDevice>,
    /// Path of the file to save the configuration to
    #[serde(skip)]
    save_path: PathBuf,
}

impl Config {
    /// Load configuration from the configuration files
    pub fn load() -> Result<Self> {
        let project_config_file = Self::project_config_path()?;
        let port_config_file = Self::port_config_path()?;

        let project_config = if let Ok(data) = read_to_string(&project_config_file) {
            toml::from_str(&data).into_diagnostic()?
        } else {
            ProjectConfig::default()
        };

        if let Some(table) = &project_config.esp_idf_format_args.partition_table {
            match table.extension() {
                Some(ext) if ext == "bin" || ext == "csv" => {}
                _ => return Err(Error::InvalidPartitionTablePath.into()),
            }
        }

        if let Some(bootloader) = &project_config.esp_idf_format_args.bootloader {
            if bootloader.extension() != Some(OsStr::new("bin")) {
                return Err(Error::InvalidBootloaderPath.into());
            }
        }

        debug!("Config: {:#?}", &project_config);

        let mut port_config = if let Ok(data) = read_to_string(&port_config_file) {
            toml::from_str(&data).into_diagnostic()?
        } else {
            PortConfig::default()
        };
        port_config.save_path = port_config_file;
        debug!("Port Config: {:#?}", &port_config);

        Ok(Config {
            project_config,
            port_config,
        })
    }

    /// Save port configuration to the configuration file
    pub fn save_with<F: Fn(&mut Self)>(&self, modify_fn: F) -> Result<()> {
        let mut copy = self.clone();
        modify_fn(&mut copy);

        let serialized = toml::to_string(&copy.port_config)
            .into_diagnostic()
            .wrap_err("Failed to serialize config")?;

        create_dir_all(self.port_config.save_path.parent().unwrap())
            .into_diagnostic()
            .wrap_err("Failed to create config directory")?;
        write(&self.port_config.save_path, serialized)
            .into_diagnostic()
            .wrap_err_with(|| {
                format!(
                    "Failed to write config to {}",
                    self.port_config.save_path.display()
                )
            })
    }

    fn project_config_path() -> Result<PathBuf, Error> {
        Self::find_config_path("espflash.toml")
    }

    fn port_config_path() -> Result<PathBuf, Error> {
        Self::find_config_path("espflash_ports.toml")
    }

    fn find_config_path(filename: &str) -> Result<PathBuf, Error> {
        let local_config = std::env::current_dir()?.join(filename);
        if local_config.exists() {
            return Ok(local_config);
        }
        if let Some(parent_folder) = std::env::current_dir()?.parent() {
            let workspace_config = parent_folder.join(filename);
            if workspace_config.exists() {
                return Ok(workspace_config);
            }
        }

        let project_dirs = ProjectDirs::from("rs", "esp", "espflash").unwrap();
        let global_config = project_dirs.config_dir().join(filename);
        Ok(global_config)
    }
}

#[cfg(test)]
mod tests {
    use serde::Deserialize;

    use super::*;

    #[derive(Debug, Deserialize, Serialize)]
    struct TestData {
        #[serde(
            serialize_with = "serialize_u16_to_hex",
            deserialize_with = "deserialize_hex_to_u16"
        )]
        value: u16,
    }

    #[test]
    fn test_deserialize_hex_to_u16() {
        // Test no padding
        let result: Result<TestData, _> = toml::from_str(r#"value = "aaaa""#);
        assert_eq!(result.unwrap().value, 0xaaaa);

        let result: Result<TestData, _> = toml::from_str(r#"value = "1234""#);
        assert_eq!(result.unwrap().value, 0x1234);

        // Test padding
        let result: Result<TestData, _> = toml::from_str(r#"value = "a""#);
        assert_eq!(result.unwrap().value, 0x0a);

        let result: Result<TestData, _> = toml::from_str(r#"value = "10""#);
        assert_eq!(result.unwrap().value, 0x10);

        let result: Result<TestData, _> = toml::from_str(r#"value = "100""#);
        assert_eq!(result.unwrap().value, 0x0100);

        // Test uppercase
        let result: Result<TestData, _> = toml::from_str(r#"value = "A1B2""#);
        assert_eq!(result.unwrap().value, 0xA1B2);

        // Test invalid
        let result: Result<TestData, _> = toml::from_str(r#"value = "gg""#);
        assert!(result.is_err());

        let result: Result<TestData, _> = toml::from_str(r#"value = "10gg""#);
        assert!(result.is_err());
    }

    #[test]
    fn test_serialize_u16_to_hex() {
        // Valid hexadecimal input with 1 digit
        let result: Result<TestData, _> = toml::from_str(r#"value = "1""#);
        assert_eq!(result.unwrap().value, 0x1);

        // Valid hexadecimal input with 2 digits
        let result: Result<TestData, _> = toml::from_str(r#"value = "ff""#);
        assert_eq!(result.unwrap().value, 0xff);

        // Valid hexadecimal input with 3 digits
        let result: Result<TestData, _> = toml::from_str(r#"value = "b1a""#);
        assert_eq!(result.unwrap().value, 0xb1a);

        // Valid hexadecimal input with 4 digits
        let result: Result<TestData, _> = toml::from_str(r#"value = "abc1""#);
        assert_eq!(result.unwrap().value, 0xabc1);

        // Invalid input (non-hexadecimal character)
        let result: Result<TestData, _> = toml::from_str(r#"value = "xyz""#);
        assert!(result.is_err());
    }
}

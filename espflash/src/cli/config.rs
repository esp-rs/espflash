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
    path::{Path, PathBuf},
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
    let hex_string = format!("{decimal:04x}");
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
    #[serde(default, alias = "idf")]
    pub idf_format_args: cli::IdfFormatArgs,
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
    /// Load configuration from the configuration files.
    pub fn load() -> Result<Self> {
        let project_config_file = Self::project_config_path()?;
        let port_config_file = Self::port_config_path()?;

        let raw_data = read_to_string(&project_config_file).unwrap_or_default();
        let toml_value = toml::from_str::<toml::Value>(&raw_data)
            .unwrap_or_else(|_| toml::Value::Table(Default::default()));

        if let toml::Value::Table(top_level) = &toml_value {
            Self::validate_keys(top_level)?;
        }

        let project_config: ProjectConfig =
            toml::from_str(&raw_data).unwrap_or_else(|_| ProjectConfig::default());

        Self::validate_partition_table_path(&project_config)?;
        Self::validate_bootloader_path(&project_config)?;

        debug!("Config: {:#?}", &project_config);

        let mut port_config =
            Self::load_port_config(&port_config_file, &project_config_file, &raw_data)?;
        port_config.save_path = port_config_file;
        debug!("Port Config: {:#?}", &port_config);

        Ok(Config {
            project_config,
            port_config,
        })
    }

    fn validate_keys(top_level: &toml::map::Map<String, toml::Value>) -> Result<()> {
        let forbidden_keys: &[&[&str]] = &[
            &[
                "bootloader",
                "partition_table",
                "partition_table_offset",
                "target_app_partition",
            ],
            &["size", "mode", "frequency"],
        ];
        let allowed_sections: &[&[&str]] = &[&["idf_format_args", "idf"], &["flash"]];

        let mut misplaced_keys = Vec::new();

        for (section_keys, allowed) in forbidden_keys.iter().zip(allowed_sections.iter()) {
            for &key in *section_keys {
                for (section_name, value) in top_level {
                    if let toml::Value::Table(table) = value {
                        if table.contains_key(key) && !allowed.contains(&section_name.as_str()) {
                            misplaced_keys.push((key, allowed[0]));
                        }
                    }
                }
                if top_level.contains_key(key) {
                    misplaced_keys.push((key, allowed[0]));
                }
            }
        }

        if misplaced_keys.is_empty() {
            Ok(())
        } else {
            let msg = misplaced_keys
                .into_iter()
                .map(|(key, section)| format!("'{key}' should be under [{section}]!"))
                .collect::<Vec<_>>()
                .join(", ");
            Err(Error::MisplacedKey(msg).into())
        }
    }

    fn validate_partition_table_path(config: &ProjectConfig) -> Result<()> {
        if let Some(path) = &config.idf_format_args.partition_table {
            match path.extension() {
                Some(ext) if ext == "bin" || ext == "csv" => Ok(()),
                _ => Err(Error::InvalidPartitionTablePath.into()),
            }
        } else {
            Ok(())
        }
    }

    fn validate_bootloader_path(config: &ProjectConfig) -> Result<()> {
        if let Some(path) = &config.idf_format_args.bootloader {
            if path.extension() != Some(OsStr::new("bin")) {
                return Err(Error::InvalidBootloaderPath.into());
            }
        }
        Ok(())
    }

    fn load_port_config(
        port_config_file: &Path,
        project_config_file: &Path,
        raw_data: &str,
    ) -> Result<PortConfig> {
        if let Ok(data) = read_to_string(port_config_file) {
            toml::from_str(&data).into_diagnostic()
        } else if let Ok(data) = read_to_string(project_config_file) {
            if data.contains("[connection]") || data.contains("[[usb_device]]") {
                log::info!(
                    "espflash@3 configuration detected. Migrating port config to port_config_file: {:#?}",
                    &port_config_file
                );

                let port_config: PortConfig = toml::from_str(&data).into_diagnostic()?;
                let project_config: ProjectConfig = toml::from_str(raw_data).unwrap_or_default();

                Self::write_config(&port_config, port_config_file)?;
                Self::write_config(&project_config, project_config_file)?;
                Ok(port_config)
            } else {
                Ok(PortConfig::default())
            }
        } else {
            Ok(PortConfig::default())
        }
    }

    fn write_config<T: Serialize>(config: &T, path: &Path) -> Result<()> {
        let serialized = toml::to_string(config)
            .into_diagnostic()
            .wrap_err("Failed to serialize config")?;

        if let Some(parent) = path.parent() {
            create_dir_all(parent)
                .into_diagnostic()
                .wrap_err("Failed to create config directory")?;
        }

        write(path, serialized)
            .into_diagnostic()
            .wrap_err_with(|| format!("Failed to write config to {}", path.display()))
    }

    /// Save port configuration to the configuration file
    pub fn save_with<F: Fn(&mut Self)>(&self, modify_fn: F) -> Result<()> {
        let mut copy = self.clone();
        modify_fn(&mut copy);

        Self::write_config(&copy.port_config, &self.port_config.save_path)
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

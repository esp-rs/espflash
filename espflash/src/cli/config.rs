//! Command-line interface configuration
//!
//! Both [cargo-espflash] and [espflash] allow for the use of configuration
//! files; the [Config] type handles the loading and saving of this
//! configuration file.
//!
//! [cargo-espflash]: https://crates.io/crates/cargo-espflash
//! [espflash]: https://crates.io/crates/espflash

use std::{
    fs::{create_dir_all, read_to_string, write},
    path::PathBuf,
};

use directories_next::ProjectDirs;
use miette::{IntoDiagnostic, Result, WrapErr};
use serde::{Deserialize, Serialize};
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
    #[serde(serialize_with = "parse_u16_hex", deserialize_with = "parse_hex_u16")]
    pub vid: u16,
    /// USB Product ID
    #[serde(serialize_with = "parse_u16_hex", deserialize_with = "parse_hex_u16")]
    pub pid: u16,
}

fn parse_hex_u16<'de, D>(deserializer: D) -> Result<u16, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    let bytes = hex::decode(if s.len() % 2 == 1 {
        format!("0{}", s)
    } else {
        s
    })
    .map_err(serde::de::Error::custom)?;
    let padding = vec![0; 2_usize.saturating_sub(bytes.len())];
    // Apend the padding before the bytes
    let vec = [&padding[..], &bytes[..]].concat();
    let decimal = u16::from_be_bytes(vec.try_into().unwrap());
    Ok(decimal)
}

fn parse_u16_hex<S>(decimal: &u16, serializer: S) -> Result<S::Ok, S::Error>
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

/// Deserialized contents of a configuration file
#[derive(Debug, Deserialize, Serialize, Default, Clone)]
pub struct Config {
    /// Preferred serial port connection information
    #[serde(default)]
    pub connection: Connection,
    /// Preferred USB devices
    #[serde(default)]
    pub usb_device: Vec<UsbDevice>,
    /// Path of the file to save the config to
    #[serde(skip)]
    save_path: PathBuf,
}

impl Config {
    /// Load the config from config file
    pub fn load() -> Result<Self> {
        let dirs = ProjectDirs::from("rs", "esp", "espflash").unwrap();
        let file = dirs.config_dir().join("espflash.toml");

        let mut config = if let Ok(data) = read_to_string(&file) {
            toml::from_str(&data).into_diagnostic()?
        } else {
            Self::default()
        };
        config.save_path = file;
        Ok(config)
    }

    /// Save the config to the config file
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    #[derive(Debug, Deserialize, Serialize)]
    struct TestData {
        #[serde(serialize_with = "parse_u16_hex", deserialize_with = "parse_hex_u16")]
        value: u16,
    }

    #[test]
    fn test_parse_hex_u16() {
        // Test no padding
        let input = "aaaa";
        let result: Result<TestData, _> = toml::from_str(&format!("value = \"{}\"", input));
        assert_eq!(result.unwrap().value, 0xaaaa);
        let input = "1234";
        let result: Result<TestData, _> = toml::from_str(&format!("value = \"{}\"", input));
        assert_eq!(result.unwrap().value, 0x1234);

        // Test padding
        let input = "a";
        let result: Result<TestData, _> = toml::from_str(&format!("value = \"{}\"", input));
        assert_eq!(result.unwrap().value, 0x0a);

        let input = "10";
        let result: Result<TestData, _> = toml::from_str(&format!("value = \"{}\"", input));
        assert_eq!(result.unwrap().value, 0x10);

        let input = "100";
        let result: Result<TestData, _> = toml::from_str(&format!("value = \"{}\"", input));
        assert_eq!(result.unwrap().value, 0x0100);

        // Test uppercase
        let input = "A1B2";
        let result: Result<TestData, _> = toml::from_str(&format!("value = \"{}\"", input));
        assert_eq!(result.unwrap().value, 0xA1B2);

        // Test invalid
        let input = "gg";
        let result: Result<TestData, _> = toml::from_str(&format!("value = \"{}\"", input));
        assert!(result.is_err());

        let input = "10gg";
        let result: Result<TestData, _> = toml::from_str(&format!("value = \"{}\"", input));
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_u16_hex() {
        // Valid hexadecimal input with 1 digit
        let input = "1";
        let result: Result<TestData, _> = toml::from_str(&format!("value = \"{}\"", input));
        assert_eq!(result.unwrap().value, 0x1);

        // Valid hexadecimal input with 2 digits
        let input = "ff";
        let result: Result<TestData, _> = toml::from_str(&format!("value = \"{}\"", input));
        assert_eq!(result.unwrap().value, 0xff);

        // Valid hexadecimal input with 3 digits
        let input = "b1a";
        let result: Result<TestData, _> = toml::from_str(&format!("value = \"{}\"", input));
        assert_eq!(result.unwrap().value, 0xb1a);

        // Valid hexadecimal input with 4 digits
        let input = "abc1";
        let result: Result<TestData, _> = toml::from_str(&format!("value = \"{}\"", input));
        assert_eq!(result.unwrap().value, 0xabc1);

        // Invalid input (non-hexadecimal character)
        let input = "xyz";
        let result: Result<TestData, _> = toml::from_str(&format!("value = \"{}\"", input));
        assert!(result.is_err());
    }
}

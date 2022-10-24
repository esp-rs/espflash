//! Supported binary image formats
//!
//! Since the ESP8266 is not supported by ESP-IDF, it has its own image format
//! which must be used. All other devices support the ESP-IDF bootloader format.
//! Certain devices additionall support direct boot, which needs its own unique
//! image format.

use std::str::FromStr;

use bytemuck::{Pod, Zeroable};
use serde::Deserialize;
use strum::{Display, EnumVariantNames, IntoStaticStr};

pub use self::{
    direct_boot::DirectBootFormat, esp8266::Esp8266Format, idf_bootloader::IdfBootloaderFormat,
};
use crate::{elf::RomSegment, error::Error, flasher::FlashFrequency, targets::Chip};

mod direct_boot;
mod esp8266;
mod idf_bootloader;

const ESP_CHECKSUM_MAGIC: u8 = 0xef;
const ESP_MAGIC: u8 = 0xE9;
const WP_PIN_DISABLED: u8 = 0xEE;

#[derive(Debug, Clone, Copy, Pod, Zeroable)]
#[repr(C, packed)]
struct EspCommonHeader {
    magic: u8,
    segment_count: u8,
    flash_mode: u8,
    flash_config: u8,
    entry: u32,
}

#[derive(Debug, Clone, Copy, Pod, Zeroable)]
#[repr(C, packed)]
struct SegmentHeader {
    addr: u32,
    length: u32,
}

/// Operations for working with firmware image formats
pub trait ImageFormat<'a>: Send {
    /// Get the rom segments needed when flashing to device
    fn flash_segments<'b>(&'b self) -> Box<dyn Iterator<Item = RomSegment<'b>> + 'b>
    where
        'a: 'b;

    /// Get the rom segments to save when exporting for ota
    ///
    /// Compared to `flash_segments` this excludes things like bootloader and
    /// partition table
    fn ota_segments<'b>(&'b self) -> Box<dyn Iterator<Item = RomSegment<'b>> + 'b>
    where
        'a: 'b;
}

/// All supported firmware image formats
#[derive(
    Debug, Copy, Clone, Eq, PartialEq, Display, IntoStaticStr, EnumVariantNames, Deserialize,
)]
#[strum(serialize_all = "kebab-case")]
#[serde(rename_all = "kebab-case")]
pub enum ImageFormatKind {
    /// Use the second-stage bootloader from ESP-IDF
    EspBootloader,
    /// Use direct boot and do not use a second-stage bootloader at all
    DirectBoot,
}

impl FromStr for ImageFormatKind {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "bootloader" => Ok(Self::EspBootloader),
            "direct-boot" => Ok(Self::DirectBoot),
            _ => Err(Error::UnknownImageFormat(s.into())),
        }
    }
}

fn encode_flash_frequency(chip: Chip, frequency: FlashFrequency) -> Result<u8, Error> {
    let encodings = chip.into_target().flash_frequency_encodings();
    if let Some(&f) = encodings.get(&frequency) {
        Ok(f)
    } else {
        Err(Error::UnsupportedFlashFrequency { chip, frequency })
    }
}

fn update_checksum(data: &[u8], mut checksum: u8) -> u8 {
    for byte in data {
        checksum ^= *byte;
    }

    checksum
}

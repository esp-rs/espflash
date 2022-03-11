mod esp32bootloader;
mod esp32directboot;
mod esp8266;

use crate::elf::RomSegment;
use bytemuck::{Pod, Zeroable};
pub use esp32bootloader::*;
pub use esp32directboot::*;
pub use esp8266::*;

use crate::error::Error;
use serde::Deserialize;
use std::str::FromStr;
use strum_macros::{Display, EnumVariantNames, IntoStaticStr};

const ESP_MAGIC: u8 = 0xE9;
const WP_PIN_DISABLED: u8 = 0xEE;

#[derive(Copy, Clone, Zeroable, Pod, Debug)]
#[repr(C, packed)]
struct EspCommonHeader {
    magic: u8,
    segment_count: u8,
    flash_mode: u8,
    flash_config: u8,
    entry: u32,
}

#[derive(Copy, Clone, Zeroable, Pod, Debug)]
#[repr(C, packed)]
struct SegmentHeader {
    addr: u32,
    length: u32,
}

pub trait ImageFormat<'a> {
    /// Get the rom segments needed when flashing to device
    fn flash_segments<'b>(&'b self) -> Box<dyn Iterator<Item = RomSegment<'b>> + 'b>
    where
        'a: 'b;

    /// Get the rom segments to save when exporting for ota
    ///
    /// Compared to `flash_segments` this excludes things like bootloader and partition table
    fn ota_segments<'b>(&'b self) -> Box<dyn Iterator<Item = RomSegment<'b>> + 'b>
    where
        'a: 'b;
}

#[derive(
    Debug, Copy, Clone, Eq, PartialEq, Display, IntoStaticStr, EnumVariantNames, Deserialize,
)]
#[strum(serialize_all = "kebab-case")]
#[serde(rename_all = "kebab-case")]
pub enum ImageFormatId {
    Bootloader,
    DirectBoot,
}

impl FromStr for ImageFormatId {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "bootloader" => Ok(Self::Bootloader),
            "direct-boot" => Ok(Self::DirectBoot),
            _ => Err(Error::UnknownImageFormat(s.into())),
        }
    }
}

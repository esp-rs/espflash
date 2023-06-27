//! Supported binary image formats
//!
//! Since the ESP8266 is not supported by ESP-IDF, it has its own image format
//! which must be used. All other devices support the ESP-IDF bootloader format.
//! Certain devices additionally support direct boot, which needs its own unique
//! image format.

use std::str::FromStr;

use bytemuck::{Pod, Zeroable};
use serde::Deserialize;
use strum::{Display, EnumVariantNames, IntoStaticStr};

pub use self::{
    direct_boot::DirectBootFormat, esp8266::Esp8266Format, idf_bootloader::IdfBootloaderFormat,
};
use crate::{
    elf::RomSegment,
    error::Error,
    flasher::{FlashFrequency, FlashMode, FlashSize},
    targets::Chip,
};

mod direct_boot;
mod esp8266;
mod idf_bootloader;

const ESP_CHECKSUM_MAGIC: u8 = 0xef;
const ESP_MAGIC: u8 = 0xE9;
const WP_PIN_DISABLED: u8 = 0xEE;

/// Firmware header used by the ESP-IDF bootloader.
///
/// ## Header documentation:
/// * [Header](https://docs.espressif.com/projects/esptool/en/latest/esp32c3/advanced-topics/firmware-image-format.html#file-header)
/// * [Extended header](https://docs.espressif.com/projects/esptool/en/latest/esp32c3/advanced-topics/firmware-image-format.html#extended-file-header)
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
#[repr(C, packed)]
#[doc(alias = "esp_image_header_t")]
struct ImageHeader {
    magic: u8,
    segment_count: u8,
    /// Flash read mode (esp_image_spi_mode_t)
    flash_mode: u8,
    /// ..4 bits are flash chip size (esp_image_flash_size_t)
    /// 4.. bits are flash frequency (esp_image_spi_freq_t)
    #[doc(alias = "spi_size")]
    #[doc(alias = "spi_speed")]
    flash_config: u8,
    entry: u32,

    // extended header part
    wp_pin: u8,
    clk_q_drv: u8,
    d_cs_drv: u8,
    gd_wp_drv: u8,
    chip_id: u16,
    min_rev: u8,
    /// Minimal chip revision supported by image, in format: major * 100 + minor
    min_chip_rev_full: u16,
    /// Maximal chip revision supported by image, in format: major * 100 + minor
    max_chip_rev_full: u16,
    reserved: [u8; 4],
    append_digest: u8,
}

impl Default for ImageHeader {
    fn default() -> Self {
        Self {
            magic: ESP_MAGIC,
            segment_count: 3,
            flash_mode: FlashMode::default() as _,
            flash_config: ((FlashSize::default() as u8) << 4) | FlashFrequency::default() as u8,
            entry: 0,
            wp_pin: WP_PIN_DISABLED,
            clk_q_drv: 0,
            d_cs_drv: 0,
            gd_wp_drv: 0,
            chip_id: Default::default(),
            min_rev: 0,
            min_chip_rev_full: 0,
            max_chip_rev_full: u16::MAX,
            reserved: Default::default(),
            append_digest: 1,
        }
    }
}

impl ImageHeader {
    /// Header size without extended part.
    ///
    /// [ESP8266 header format](https://docs.espressif.com/projects/esptool/en/latest/esp8266/advanced-topics/firmware-image-format.html#file-header)
    pub const COMMON_HEADER_LEN: u32 = 8;

    /// Updates flash size and speed filed.
    pub fn write_flash_config(
        &mut self,
        size: FlashSize,
        freq: FlashFrequency,
        chip: Chip,
    ) -> Result<(), Error> {
        let flash_size = size.encode_flash_size(chip)?;
        let flash_speed = freq.encode_flash_frequency(chip)?;

        // bit field
        self.flash_config = (flash_size << 4) | flash_speed;
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, Pod, Zeroable)]
#[repr(C, packed)]
struct SegmentHeader {
    addr: u32,
    length: u32,
}

/// Operations for working with firmware image formats
pub trait ImageFormat<'a>: Send {
    /// Get the ROM segments needed when flashing to device
    fn flash_segments<'b>(&'b self) -> Box<dyn Iterator<Item = RomSegment<'b>> + 'b>
    where
        'a: 'b;

    /// Get the ROM segments to save when exporting for OTA
    ///
    /// Compared to `flash_segments` this excludes things like bootloader and
    /// partition table
    fn ota_segments<'b>(&'b self) -> Box<dyn Iterator<Item = RomSegment<'b>> + 'b>
    where
        'a: 'b;

    /// The size of the application binary
    fn app_size(&self) -> u32;

    /// If applicable, the size of the application partition (if it can be
    /// determined)
    fn part_size(&self) -> Option<u32>;
}

/// All supported firmware image formats
#[cfg_attr(feature = "cli", derive(clap::ValueEnum))]
#[derive(
    Debug, Copy, Clone, Eq, PartialEq, Display, IntoStaticStr, EnumVariantNames, Deserialize,
)]
#[non_exhaustive]
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
            "esp-bootloader" => Ok(Self::EspBootloader),
            "direct-boot" => Ok(Self::DirectBoot),
            _ => Err(Error::UnknownImageFormat(s.into())),
        }
    }
}

/// Update the checksum with the given data
fn update_checksum(data: &[u8], mut checksum: u8) -> u8 {
    for byte in data {
        checksum ^= *byte;
    }

    checksum
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_flash_config_write() {
        let mut header = ImageHeader::default();
        header
            .write_flash_config(FlashSize::_4Mb, FlashFrequency::_40Mhz, Chip::Esp32c3)
            .unwrap();
        assert_eq!(header.flash_config, 0x20);

        header
            .write_flash_config(FlashSize::_32Mb, FlashFrequency::_80Mhz, Chip::Esp32s3)
            .unwrap();
        assert_eq!(header.flash_config, 0x5F);
    }
}

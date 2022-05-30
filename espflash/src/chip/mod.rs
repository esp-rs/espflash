use std::ops::Range;

use strum_macros::Display;

use crate::{
    connection::Connection,
    elf::FirmwareImage,
    error::ChipDetectError,
    flash_target::{Esp32Target, Esp8266Target, FlashTarget, RamTarget},
    flasher::SpiAttachParams,
    image_format::{ImageFormat, ImageFormatId},
    Error, PartitionTable,
};

mod esp32;
mod esp8266;

pub use esp32::{Esp32, Esp32Params, Esp32c3, Esp32s2, Esp32s3};
pub use esp8266::Esp8266;
use std::str::FromStr;

pub trait ChipType: ReadEFuse {
    const CHIP_DETECT_MAGIC_VALUE: u32;
    const CHIP_DETECT_MAGIC_VALUE2: u32 = 0x0; // give default value, as most chips don't only have one

    const UART_CLKDIV_REG: u32;
    const UART_CLKDIV_MASK: u32 = 0xFFFFF;
    const XTAL_CLK_DIVIDER: u32 = 1;

    const SPI_REGISTERS: SpiRegisters;
    const FLASH_RANGES: &'static [Range<u32>];

    const DEFAULT_IMAGE_FORMAT: ImageFormatId;
    const SUPPORTED_IMAGE_FORMATS: &'static [ImageFormatId];

    const SUPPORTED_TARGETS: &'static [&'static str];

    /// List the available features of the connected chip.
    fn chip_features(&self, connection: &mut Connection) -> Result<Vec<&str>, Error>;

    /// Determine the frequency of the crytal on the connected chip.
    fn crystal_freq(&self, connection: &mut Connection) -> Result<u32, Error> {
        let uart_div = connection.read_reg(Self::UART_CLKDIV_REG)? & Self::UART_CLKDIV_MASK;
        let est_xtal = (connection.get_baud()? * uart_div) / 1_000_000 / Self::XTAL_CLK_DIVIDER;
        let norm_xtal = if est_xtal > 33 { 40 } else { 26 };

        Ok(norm_xtal)
    }

    /// Get the firmware segments for writing an image to flash.
    fn get_flash_segments<'a>(
        image: &'a FirmwareImage,
        bootloader: Option<Vec<u8>>,
        partition_table: Option<PartitionTable>,
        image_format: ImageFormatId,
        chip_revision: Option<u32>,
    ) -> Result<Box<dyn ImageFormat<'a> + 'a>, Error>;

    /// Read the MAC address of the connected chip.
    fn mac_address(&self, connection: &mut Connection) -> Result<String, Error> {
        let word5 = self.read_efuse(connection, 5)?;
        let word6 = self.read_efuse(connection, 6)?;

        let bytes = ((word6 as u64) << 32) | word5 as u64;
        let bytes = bytes.to_be_bytes();
        let bytes = &bytes[2..];

        Ok(bytes_to_mac_addr(bytes))
    }

    fn supports_target(target: &str) -> bool {
        Self::SUPPORTED_TARGETS.contains(&target)
    }
}

pub trait ReadEFuse {
    const EFUSE_REG_BASE: u32;

    /// Given an active connection, read the nth word of the eFuse region.
    fn read_efuse(&self, connection: &mut Connection, n: u32) -> Result<u32, Error> {
        let reg = Self::EFUSE_REG_BASE + (n * 0x4);
        connection.read_reg(reg)
    }
}

pub struct SpiRegisters {
    base: u32,
    usr_offset: u32,
    usr1_offset: u32,
    usr2_offset: u32,
    w0_offset: u32,
    mosi_length_offset: Option<u32>,
    miso_length_offset: Option<u32>,
}

impl SpiRegisters {
    pub fn cmd(&self) -> u32 {
        self.base
    }

    pub fn usr(&self) -> u32 {
        self.base + self.usr_offset
    }

    pub fn usr1(&self) -> u32 {
        self.base + self.usr1_offset
    }

    pub fn usr2(&self) -> u32 {
        self.base + self.usr2_offset
    }

    pub fn w0(&self) -> u32 {
        self.base + self.w0_offset
    }

    pub fn mosi_length(&self) -> Option<u32> {
        self.mosi_length_offset.map(|offset| self.base + offset)
    }

    pub fn miso_length(&self) -> Option<u32> {
        self.miso_length_offset.map(|offset| self.base + offset)
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Display)]
pub enum Chip {
    #[strum(serialize = "ESP32")]
    Esp32,
    #[strum(serialize = "ESP32-C3")]
    Esp32c3,
    #[strum(serialize = "ESP32-S2")]
    Esp32s2,
    #[strum(serialize = "ESP32-S3")]
    Esp32s3,
    #[strum(serialize = "ESP8266")]
    Esp8266,
}

impl FromStr for Chip {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s: String = s
            .chars()
            .filter(|&c| c != '-')
            .map(|c| c.to_ascii_lowercase())
            .collect();
        match s.as_str() {
            "esp32" => Ok(Chip::Esp32),
            "esp32c3" => Ok(Chip::Esp32c3),
            "esp32s2" => Ok(Chip::Esp32s2),
            "esp32s3" => Ok(Chip::Esp32s3),
            "esp8266" => Ok(Chip::Esp8266),
            _ => Err(Error::UnrecognizedChipName),
        }
    }
}

impl Chip {
    pub fn from_magic(magic: u32) -> Result<Self, ChipDetectError> {
        match magic {
            Esp32::CHIP_DETECT_MAGIC_VALUE => Ok(Chip::Esp32),
            Esp32c3::CHIP_DETECT_MAGIC_VALUE | Esp32c3::CHIP_DETECT_MAGIC_VALUE2 => {
                Ok(Chip::Esp32c3)
            }
            Esp32s2::CHIP_DETECT_MAGIC_VALUE => Ok(Chip::Esp32s2),
            Esp32s3::CHIP_DETECT_MAGIC_VALUE => Ok(Chip::Esp32s3),
            Esp8266::CHIP_DETECT_MAGIC_VALUE => Ok(Chip::Esp8266),
            _ => Err(ChipDetectError::from(magic)),
        }
    }

    pub fn from_target(target: &str) -> Option<Self> {
        if Esp32::supports_target(target) {
            return Some(Chip::Esp32);
        } else if Esp32c3::supports_target(target) {
            return Some(Chip::Esp32c3);
        } else if Esp32s2::supports_target(target) {
            return Some(Chip::Esp32s2);
        } else if Esp32s3::supports_target(target) {
            return Some(Chip::Esp32s3);
        } else if Esp8266::supports_target(target) {
            return Some(Chip::Esp8266);
        }

        None
    }

    pub fn get_flash_image<'a>(
        &self,
        image: &'a FirmwareImage,
        bootloader: Option<Vec<u8>>,
        partition_table: Option<PartitionTable>,
        image_format: Option<ImageFormatId>,
        chip_revision: Option<u32>,
    ) -> Result<Box<dyn ImageFormat<'a> + 'a>, Error> {
        let image_format = image_format.unwrap_or_else(|| self.default_image_format());

        match self {
            Chip::Esp32 => Esp32::get_flash_segments(
                image,
                bootloader,
                partition_table,
                image_format,
                chip_revision,
            ),
            Chip::Esp32c3 => Esp32c3::get_flash_segments(
                image,
                bootloader,
                partition_table,
                image_format,
                chip_revision,
            ),
            Chip::Esp32s2 => Esp32s2::get_flash_segments(
                image,
                bootloader,
                partition_table,
                image_format,
                chip_revision,
            ),
            Chip::Esp32s3 => Esp32s3::get_flash_segments(
                image,
                bootloader,
                partition_table,
                image_format,
                chip_revision,
            ),
            Chip::Esp8266 => {
                Esp8266::get_flash_segments(image, None, None, image_format, chip_revision)
            }
        }
    }

    pub fn addr_is_flash(&self, addr: u32) -> bool {
        let flash_ranges = match self {
            Chip::Esp32 => Esp32::FLASH_RANGES,
            Chip::Esp32c3 => Esp32c3::FLASH_RANGES,
            Chip::Esp32s2 => Esp32s2::FLASH_RANGES,
            Chip::Esp32s3 => Esp32s3::FLASH_RANGES,
            Chip::Esp8266 => Esp8266::FLASH_RANGES,
        };

        flash_ranges.iter().any(|range| range.contains(&addr))
    }

    pub fn spi_registers(&self) -> SpiRegisters {
        match self {
            Chip::Esp32 => Esp32::SPI_REGISTERS,
            Chip::Esp32c3 => Esp32c3::SPI_REGISTERS,
            Chip::Esp32s2 => Esp32s2::SPI_REGISTERS,
            Chip::Esp32s3 => Esp32s3::SPI_REGISTERS,
            Chip::Esp8266 => Esp8266::SPI_REGISTERS,
        }
    }

    pub fn ram_target(&self, entry: Option<u32>) -> Box<dyn FlashTarget> {
        Box::new(RamTarget::new(entry))
    }

    pub fn flash_target(&self, spi_params: SpiAttachParams) -> Box<dyn FlashTarget> {
        match self {
            Chip::Esp8266 => Box::new(Esp8266Target::new()),
            _ => Box::new(Esp32Target::new(*self, spi_params)),
        }
    }

    fn default_image_format(&self) -> ImageFormatId {
        match self {
            Chip::Esp32 => Esp32::DEFAULT_IMAGE_FORMAT,
            Chip::Esp32c3 => Esp32c3::DEFAULT_IMAGE_FORMAT,
            Chip::Esp32s2 => Esp32s2::DEFAULT_IMAGE_FORMAT,
            Chip::Esp32s3 => Esp32s3::DEFAULT_IMAGE_FORMAT,
            Chip::Esp8266 => Esp8266::DEFAULT_IMAGE_FORMAT,
        }
    }

    pub fn supported_image_formats(&self) -> &[ImageFormatId] {
        match self {
            Chip::Esp32 => Esp32::SUPPORTED_IMAGE_FORMATS,
            Chip::Esp32c3 => Esp32c3::SUPPORTED_IMAGE_FORMATS,
            Chip::Esp32s2 => Esp32s2::SUPPORTED_IMAGE_FORMATS,
            Chip::Esp32s3 => Esp32s3::SUPPORTED_IMAGE_FORMATS,
            Chip::Esp8266 => Esp8266::SUPPORTED_IMAGE_FORMATS,
        }
    }

    pub fn supports_target(&self, target: &str) -> bool {
        match self {
            Chip::Esp32 => Esp32::supports_target(target),
            Chip::Esp32c3 => Esp32c3::supports_target(target),
            Chip::Esp32s2 => Esp32s2::supports_target(target),
            Chip::Esp32s3 => Esp32s3::supports_target(target),
            Chip::Esp8266 => Esp8266::supports_target(target),
        }
    }

    pub fn supported_targets(&self) -> &[&str] {
        match self {
            Chip::Esp32 => Esp32::SUPPORTED_TARGETS,
            Chip::Esp32c3 => Esp32c3::SUPPORTED_TARGETS,
            Chip::Esp32s2 => Esp32s2::SUPPORTED_TARGETS,
            Chip::Esp32s3 => Esp32s3::SUPPORTED_TARGETS,
            Chip::Esp8266 => Esp8266::SUPPORTED_TARGETS,
        }
    }

    pub fn crystal_freq(&self, connection: &mut Connection) -> Result<u32, Error> {
        match self {
            Chip::Esp32 => Esp32.crystal_freq(connection),
            Chip::Esp32c3 => Esp32c3.crystal_freq(connection),
            Chip::Esp32s2 => Esp32s2.crystal_freq(connection),
            Chip::Esp32s3 => Esp32s3.crystal_freq(connection),
            Chip::Esp8266 => Esp8266.crystal_freq(connection),
        }
    }

    pub fn chip_revision(&self, connection: &mut Connection) -> Result<Option<u32>, Error> {
        let rev = match self {
            Chip::Esp32 => Some(Esp32.chip_revision(connection)?),
            Chip::Esp32c3 => Some(Esp32c3.chip_revision(connection)?),
            _ => None,
        };

        Ok(rev)
    }

    pub fn chip_features(&self, connection: &mut Connection) -> Result<Vec<&str>, Error> {
        match self {
            Chip::Esp32 => Esp32.chip_features(connection),
            Chip::Esp32c3 => Esp32c3.chip_features(connection),
            Chip::Esp32s2 => Esp32s2.chip_features(connection),
            Chip::Esp32s3 => Esp32s3.chip_features(connection),
            Chip::Esp8266 => Esp8266.chip_features(connection),
        }
    }

    pub fn mac_address(&self, connection: &mut Connection) -> Result<String, Error> {
        match self {
            Chip::Esp32 => Esp32.mac_address(connection),
            Chip::Esp32c3 => Esp32c3.mac_address(connection),
            Chip::Esp32s2 => Esp32s2.mac_address(connection),
            Chip::Esp32s3 => Esp32s3.mac_address(connection),
            Chip::Esp8266 => Esp8266.mac_address(connection),
        }
    }
}

pub(crate) fn bytes_to_mac_addr(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|b| format!("{:02x}", b))
        .collect::<Vec<_>>()
        .join(":")
}

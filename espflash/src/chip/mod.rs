use strum_macros::Display;

use crate::{
    elf::FirmwareImage,
    error::ChipDetectError,
    flash_target::{Esp32Target, Esp8266Target, FlashTarget, RamTarget},
    flasher::SpiAttachParams,
    Error, PartitionTable,
};

use crate::image_format::{ImageFormat, ImageFormatId};
pub use esp32::Esp32;
pub use esp32c3::Esp32c3;
pub use esp32s2::Esp32s2;
pub use esp8266::Esp8266;
use std::ops::Range;

mod esp32;
mod esp32c3;
mod esp32s2;
mod esp8266;

pub trait ChipType {
    const CHIP_DETECT_MAGIC_VALUE: u32;
    const CHIP_DETECT_MAGIC_VALUE2: u32 = 0x0; // give default value, as most chips don't only have one

    const SPI_REGISTERS: SpiRegisters;
    const FLASH_RANGES: &'static [Range<u32>];

    const DEFAULT_IMAGE_FORMAT: ImageFormatId;
    const SUPPORTED_IMAGE_FORMATS: &'static [ImageFormatId];

    /// Get the firmware segments for writing an image to flash
    fn get_flash_segments<'a>(
        image: &'a FirmwareImage,
        bootloader: Option<Vec<u8>>,
        partition_table: Option<PartitionTable>,
        image_format: ImageFormatId,
    ) -> Result<Box<dyn ImageFormat<'a> + 'a>, Error>;
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
    #[strum(serialize = "ESP8266")]
    Esp8266,
}

impl Chip {
    pub fn from_magic(magic: u32) -> Result<Self, ChipDetectError> {
        match magic {
            Esp32::CHIP_DETECT_MAGIC_VALUE => Ok(Chip::Esp32),
            Esp32c3::CHIP_DETECT_MAGIC_VALUE | Esp32c3::CHIP_DETECT_MAGIC_VALUE2 => {
                Ok(Chip::Esp32c3)
            }
            Esp32s2::CHIP_DETECT_MAGIC_VALUE => Ok(Chip::Esp32s2),
            Esp8266::CHIP_DETECT_MAGIC_VALUE => Ok(Chip::Esp8266),
            _ => Err(ChipDetectError::from(magic)),
        }
    }

    pub fn get_flash_image<'a>(
        &self,
        image: &'a FirmwareImage,
        bootloader: Option<Vec<u8>>,
        partition_table: Option<PartitionTable>,
        image_format: Option<ImageFormatId>,
    ) -> Result<Box<dyn ImageFormat<'a> + 'a>, Error> {
        let image_format = image_format.unwrap_or_else(|| self.default_image_format());

        match self {
            Chip::Esp32 => {
                Esp32::get_flash_segments(image, bootloader, partition_table, image_format)
            }
            Chip::Esp32c3 => {
                Esp32c3::get_flash_segments(image, bootloader, partition_table, image_format)
            }
            Chip::Esp32s2 => {
                Esp32s2::get_flash_segments(image, bootloader, partition_table, image_format)
            }
            Chip::Esp8266 => Esp8266::get_flash_segments(image, None, None, image_format),
        }
    }

    pub fn addr_is_flash(&self, addr: u32) -> bool {
        let flash_ranges = match self {
            Chip::Esp32 => Esp32::FLASH_RANGES,
            Chip::Esp32c3 => Esp32c3::FLASH_RANGES,
            Chip::Esp32s2 => Esp32s2::FLASH_RANGES,
            Chip::Esp8266 => Esp8266::FLASH_RANGES,
        };

        flash_ranges.iter().any(|range| range.contains(&addr))
    }

    pub fn spi_registers(&self) -> SpiRegisters {
        match self {
            Chip::Esp32 => Esp32::SPI_REGISTERS,
            Chip::Esp32c3 => Esp32c3::SPI_REGISTERS,
            Chip::Esp32s2 => Esp32s2::SPI_REGISTERS,
            Chip::Esp8266 => Esp8266::SPI_REGISTERS,
        }
    }

    pub fn ram_target(&self) -> Box<dyn FlashTarget> {
        Box::new(RamTarget::new())
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
            Chip::Esp8266 => Esp8266::DEFAULT_IMAGE_FORMAT,
        }
    }

    pub fn supported_image_formats(&self) -> &[ImageFormatId] {
        match self {
            Chip::Esp32 => Esp32::SUPPORTED_IMAGE_FORMATS,
            Chip::Esp32c3 => Esp32c3::SUPPORTED_IMAGE_FORMATS,
            Chip::Esp32s2 => Esp32s2::SUPPORTED_IMAGE_FORMATS,
            Chip::Esp8266 => Esp8266::SUPPORTED_IMAGE_FORMATS,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Esp32Params {
    pub boot_addr: u32,
    pub partition_addr: u32,
    pub nvs_addr: u32,
    pub nvs_size: u32,
    pub phy_init_data_addr: u32,
    pub phy_init_data_size: u32,
    pub app_addr: u32,
    pub app_size: u32,
    pub chip_id: u16,
    pub default_bootloader: &'static [u8],
}

impl Esp32Params {
    pub fn default_partition_table(&self) -> PartitionTable {
        PartitionTable::basic(
            self.nvs_addr,
            self.nvs_size,
            self.phy_init_data_addr,
            self.phy_init_data_size,
            self.app_addr,
            self.app_size,
        )
    }
}

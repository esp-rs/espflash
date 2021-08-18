use crate::elf::{FirmwareImage, RomSegment};
use crate::Error;
use bytemuck::{Pod, Zeroable};
use std::str::FromStr;

pub use esp32::Esp32;
pub use esp8266::Esp8266;

mod esp32;
mod esp32s2;
mod esp8266;

const ESP_MAGIC: u8 = 0xe9;
const WP_PIN_DISABLED: u8 = 0xEE;

pub trait ChipType {
    const DATE_REG1_VALUE: u32;
    const DATE_REG2_VALUE: u32;
    const SPI_REGISTERS: SpiRegisters;

    /// Get the firmware segments for writing an image to flash
    fn get_flash_segments<'a>(
        image: &'a FirmwareImage,
    ) -> Box<dyn Iterator<Item = Result<RomSegment<'a>, Error>> + 'a>;

    fn addr_is_flash(addr: u32) -> bool;
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

#[derive(Copy, Clone, Zeroable, Pod)]
#[repr(C)]
struct ExtendedHeader {
    wp_pin: u8,
    clk_q_drv: u8,
    d_cs_drv: u8,
    gd_wp_drv: u8,
    chip_id: u16,
    min_rev: u8,
    padding: [u8; 8],
    append_digest: u8,
}

#[derive(Debug, Copy, Clone)]
pub enum Chip {
    Esp8266,
    Esp32,
}

impl Chip {
    pub fn from_regs(value1: u32, value2: u32) -> Option<Self> {
        match (value1, value2) {
            (Esp8266::DATE_REG1_VALUE, _) => Some(Chip::Esp8266),
            (Esp32::DATE_REG1_VALUE, _) => Some(Chip::Esp32),
            _ => None,
        }
    }

    pub fn get_flash_segments<'a>(
        &self,
        image: &'a FirmwareImage,
    ) -> Box<dyn Iterator<Item = Result<RomSegment<'a>, Error>> + 'a> {
        match self {
            Chip::Esp8266 => Esp8266::get_flash_segments(image),
            Chip::Esp32 => Esp32::get_flash_segments(image),
        }
    }

    pub fn addr_is_flash(&self, addr: u32) -> bool {
        match self {
            Chip::Esp8266 => Esp8266::addr_is_flash(addr),
            Chip::Esp32 => Esp32::addr_is_flash(addr),
        }
    }

    pub fn spi_registers(&self) -> SpiRegisters {
        match self {
            Chip::Esp8266 => Esp8266::SPI_REGISTERS,
            Chip::Esp32 => Esp32::SPI_REGISTERS,
        }
    }

    /// Get the target triplet for the chip
    pub fn target(&self) -> &'static str {
        match self {
            Chip::Esp8266 => "xtensa-esp8266-none-elf",
            Chip::Esp32 => "xtensa-esp32-none-elf",
        }
    }
}

impl FromStr for Chip {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "esp32" => Ok(Chip::Esp32),
            "esp8266" => Ok(Chip::Esp8266),
            _ => Err(Error::UnrecognizedChip),
        }
    }
}

#[derive(Copy, Clone, Zeroable, Pod, Debug)]
#[repr(C)]
struct EspCommonHeader {
    magic: u8,
    segment_count: u8,
    flash_mode: u8,
    flash_config: u8,
    entry: u32,
}

#[derive(Copy, Clone, Zeroable, Pod, Debug)]
#[repr(C)]
struct SegmentHeader {
    addr: u32,
    length: u32,
}

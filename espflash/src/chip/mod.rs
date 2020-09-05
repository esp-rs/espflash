use crate::elf::{FirmwareImage, RomSegment};
use crate::Error;
use bytemuck::{Pod, Zeroable};

pub use esp32::ESP32;
pub use esp8266::ESP8266;

mod esp32;
mod esp8266;

const ESP_MAGIC: u8 = 0xe9;

pub trait ChipType {
    const DATE_REG1_VALUE: u32;
    const DATE_REG2_VALUE: u32;

    /// Get the firmware segments for writing an image to flash
    fn get_flash_segments<'a>(
        image: &'a FirmwareImage,
    ) -> Box<dyn Iterator<Item = Result<RomSegment<'a>, Error>> + 'a>;

    fn addr_is_flash(addr: u32) -> bool;
}

#[derive(Debug, Copy, Clone)]
pub enum Chip {
    Esp8266,
    Esp32,
}

impl Chip {
    pub fn from_regs(value1: u32, value2: u32) -> Option<Self> {
        match (value1, value2) {
            (ESP8266::DATE_REG1_VALUE, _) => Some(Chip::Esp8266),
            (ESP32::DATE_REG1_VALUE, _) => Some(Chip::Esp32),
            _ => None,
        }
    }

    pub fn get_flash_segments<'a>(
        &self,
        image: &'a FirmwareImage,
    ) -> Box<dyn Iterator<Item = Result<RomSegment<'a>, Error>> + 'a> {
        match self {
            Chip::Esp8266 => ESP8266::get_flash_segments(image),
            Chip::Esp32 => ESP32::get_flash_segments(image),
        }
    }

    pub fn addr_is_flash(&self, addr: u32) -> bool {
        match self {
            Chip::Esp8266 => ESP8266::addr_is_flash(addr),
            Chip::Esp32 => ESP32::addr_is_flash(addr),
        }
    }
}

#[derive(Copy, Clone, Zeroable, Pod, Debug)]
#[repr(C)]
struct ESPCommonHeader {
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

use crate::elf::{FirmwareImage, RomSegment};
use crate::Error;

pub use esp32::ESP32;
pub use esp8266::ESP8266;

mod esp32;
mod esp8266;

pub trait ChipType {
    const DATE_REG1_VALUE: u32;
    const DATE_REG2_VALUE: u32;

    /// Get the firmware segments for writing an image to flash
    fn get_flash_segments<'a>(
        image: &'a FirmwareImage,
    ) -> Box<dyn Iterator<Item = Result<RomSegment<'a>, Error>> + 'a>;
}

#[derive(Debug)]
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
}

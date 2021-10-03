mod esp32bootloader;
mod esp8266;

use crate::elf::RomSegment;
use bytemuck::{Pod, Zeroable};
pub use esp32bootloader::*;
pub use esp8266::*;

use strum_macros::{AsStaticStr, Display, EnumString};

const ESP_MAGIC: u8 = 0xE9;
const WP_PIN_DISABLED: u8 = 0xEE;

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

#[derive(Debug, Copy, Clone, Eq, PartialEq, Display, EnumString, AsStaticStr)]
pub enum ImageFormatId {
    #[strum(serialize = "bootloader")]
    Bootloader,
    #[strum(serialize = "direct-boot")]
    DirectBoot,
}

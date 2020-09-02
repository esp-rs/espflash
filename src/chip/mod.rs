use crate::elf::{FirmwareImage, RomSegment};
use crate::Error;

pub use esp8266::ESP8266;

mod esp8266;

pub trait Chip {
    /// Get the firmware segments for writing an image to flash
    fn get_flash_segments<'a>(
        image: &'a FirmwareImage,
    ) -> Box<dyn Iterator<Item = Result<RomSegment<'a>, Error>> + 'a>;
}

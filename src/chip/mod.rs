use crate::elf::{FirmwareImage, RomSegment};
use crate::Error;

pub use esp8266::ESP8266;

mod esp8266;

pub trait Chip<'a> {
    type Iter: Iterator<Item = Result<RomSegment<'a>, Error>>;

    fn get_rom_segments(image: &'a FirmwareImage) -> Self::Iter;
}

use crate::chip::ChipType;
use crate::elf::{FirmwareImage, RomSegment};
use crate::Error;

pub struct ESP32;

impl ChipType for ESP32 {
    const DATE_REG1_VALUE: u32 = 0x15122500;
    const DATE_REG2_VALUE: u32 = 0;

    fn get_flash_segments<'a>(
        image: &'a FirmwareImage,
    ) -> Box<dyn Iterator<Item = Result<RomSegment<'a>, Error>> + 'a> {
        todo!()
    }
}

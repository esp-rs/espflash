use crate::elf::CodeSegment;
use crate::{
    elf::{FirmwareImage, RomSegment},
    error::Error,
    image_format::ImageFormat,
};
use std::iter::once;

/// Image format for esp32 family chips using a 2nd stage bootloader
pub struct Esp32DirectBootFormat<'a> {
    segment: RomSegment<'a>,
}

impl<'a> Esp32DirectBootFormat<'a> {
    pub fn new(image: &'a FirmwareImage) -> Result<Self, Error> {
        let mut segment = image
            .segments()
            .map(|mut segment| {
                // map address to the first 4MB
                segment.addr %= 0x400000;
                segment
            })
            .fold(CodeSegment::default(), |mut a, b| {
                a += &b;
                a
            });
        segment.pad_align(4);

        Ok(Self {
            segment: segment.into(),
        })
    }
}

impl<'a> ImageFormat<'a> for Esp32DirectBootFormat<'a> {
    fn flash_segments<'b>(&'b self) -> Box<dyn Iterator<Item = RomSegment<'b>> + 'b>
    where
        'a: 'b,
    {
        Box::new(once(self.segment.borrow()))
    }

    fn ota_segments<'b>(&'b self) -> Box<dyn Iterator<Item = RomSegment<'b>> + 'b>
    where
        'a: 'b,
    {
        Box::new(once(self.segment.borrow()))
    }
}

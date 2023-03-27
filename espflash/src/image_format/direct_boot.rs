use std::iter::once;

use crate::{
    elf::{CodeSegment, FirmwareImage, RomSegment},
    error::Error,
    image_format::ImageFormat,
};

/// Magic number for Direct boot which should be the first 8 bytes in flash
const DIRECT_BOOT_MAGIC: &[u8] = &[0x1d, 0x04, 0xdb, 0xae, 0x1d, 0x04, 0xdb, 0xae];

/// Image format for ESP32 family chips not using a second-stage bootloader
pub struct DirectBootFormat<'a> {
    segment: RomSegment<'a>,
}

impl<'a> DirectBootFormat<'a> {
    pub fn new(image: &'a dyn FirmwareImage<'a>, magic_offset: usize) -> Result<Self, Error> {
        let mut segment = image
            .segments_with_load_addresses()
            .map(|mut segment| {
                // Map the address to the first 4MB of address space
                segment.addr %= 0x40_0000;
                segment
            })
            .fold(CodeSegment::default(), |mut a, b| {
                a += &b;
                a
            });

        segment.pad_align(4);

        if segment.addr != 0
            || (segment.data().len() >= magic_offset + 8
                && &segment.data()[magic_offset..][..8] != DIRECT_BOOT_MAGIC)
        {
            return Err(Error::InvalidDirectBootBinary);
        }

        Ok(Self {
            segment: segment.into(),
        })
    }
}

impl<'a> ImageFormat<'a> for DirectBootFormat<'a> {
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

    fn app_size(&self) -> u32 {
        self.segment.data.len() as u32
    }

    fn part_size(&self) -> Option<u32> {
        None
    }
}

#[cfg(test)]
pub mod tests {
    use std::fs;

    use super::*;
    use crate::elf::ElfFirmwareImage;

    #[test]
    fn test_direct_boot_format() {
        let input_bytes = fs::read("tests/resources/esp32c3_hal_blinky_db").unwrap();
        let expected_bin = fs::read("tests/resources/esp32c3_hal_blinky_db.bin").unwrap();

        let image = ElfFirmwareImage::try_from(input_bytes.as_slice()).unwrap();
        let flash_image = DirectBootFormat::new(&image, 0).unwrap();

        let segments = flash_image.flash_segments().collect::<Vec<_>>();
        assert_eq!(segments.len(), 1);

        let buf = segments[0].data.as_ref();
        assert_eq!(expected_bin.len(), buf.len());
        assert_eq!(expected_bin.as_slice(), buf);
    }
}

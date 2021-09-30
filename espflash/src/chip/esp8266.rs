use std::ops::Range;

use super::ChipType;
use crate::{
    chip::SpiRegisters,
    elf::FirmwareImage,
    error::UnsupportedImageFormatError,
    image_format::{Esp8266Format, ImageFormat, ImageFormatId},
    Chip, Error, PartitionTable,
};

const IROM_MAP_START: u32 = 0x40200000;
const IROM_MAP_END: u32 = 0x40300000;

pub struct Esp8266;

impl ChipType for Esp8266 {
    const CHIP_DETECT_MAGIC_VALUE: u32 = 0xfff0c101;

    const SPI_REGISTERS: SpiRegisters = SpiRegisters {
        base: 0x60000200,
        usr_offset: 0x1c,
        usr1_offset: 0x20,
        usr2_offset: 0x24,
        w0_offset: 0x40,
        mosi_length_offset: None,
        miso_length_offset: None,
    };

    const FLASH_RANGES: &'static [Range<u32>] = &[IROM_MAP_START..IROM_MAP_END];

    const DEFAULT_IMAGE_FORMAT: ImageFormatId = ImageFormatId::Bootloader;
    const SUPPORTED_IMAGE_FORMATS: &'static [ImageFormatId] = &[ImageFormatId::Bootloader];

    const SUPPORTED_TARGETS: &'static [&'static str] = &["xtensa-esp8266-none-elf"];

    fn get_flash_segments<'a>(
        image: &'a FirmwareImage,
        _bootloader: Option<Vec<u8>>,
        _partition_table: Option<PartitionTable>,
        image_format: ImageFormatId,
    ) -> Result<Box<dyn ImageFormat<'a> + 'a>, Error> {
        match image_format {
            ImageFormatId::Bootloader => Ok(Box::new(Esp8266Format::new(image)?)),
            _ => Err(UnsupportedImageFormatError::new(image_format, Chip::Esp8266).into()),
        }
    }

    fn supports_target(target: &str) -> bool {
        target.starts_with("xtensa-esp8266-")
    }
}

#[test]
fn test_esp8266_rom() {
    use pretty_assertions::assert_eq;
    use std::fs::read;

    let input_bytes = read("./tests/data/esp8266").unwrap();
    let expected_bin = read("./tests/data/esp8266.bin").unwrap();

    let image = FirmwareImage::from_data(&input_bytes).unwrap();
    let flash_image = Esp8266Format::new(&image).unwrap();

    let segments = flash_image.segments().collect::<Vec<_>>();

    assert_eq!(1, segments.len());
    let buff = segments[0].data.as_ref();
    assert_eq!(expected_bin.len(), buff.len());
    assert_eq!(expected_bin.as_slice(), buff);
}

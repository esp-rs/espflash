use crate::chip::Esp32Params;

use crate::image_format::{Esp32BootloaderFormat, ImageFormat};
use crate::{
    chip::{Chip, ChipType, SpiRegisters},
    elf::{FirmwareImage, RomSegment},
    Error, PartitionTable,
};

use std::ops::Range;
use std::{borrow::Cow, iter::once};

pub struct Esp32;

const IROM_MAP_START: u32 = 0x400d0000;
const IROM_MAP_END: u32 = 0x40400000;

const DROM_MAP_START: u32 = 0x3F400000;
const DROM_MAP_END: u32 = 0x3F800000;

pub const PARAMS: Esp32Params = Esp32Params {
    boot_addr: 0x1000,
    partition_addr: 0x8000,
    nvs_addr: 0x9000,
    nvs_size: 0x6000,
    phy_init_data_addr: 0xf000,
    phy_init_data_size: 0x1000,
    app_addr: 0x10000,
    app_size: 0x3f0000,
    chip_id: 0,
};

impl ChipType for Esp32 {
    const CHIP_DETECT_MAGIC_VALUE: u32 = 0x00f01d83;

    const SPI_REGISTERS: SpiRegisters = SpiRegisters {
        base: 0x3ff42000,
        usr_offset: 0x1c,
        usr1_offset: 0x20,
        usr2_offset: 0x24,
        w0_offset: 0x80,
        mosi_length_offset: Some(0x28),
        miso_length_offset: Some(0x2c),
    };

    const FLASH_RANGES: &'static [Range<u32>] =
        &[IROM_MAP_START..IROM_MAP_END, DROM_MAP_START..DROM_MAP_END];

    fn get_flash_segments<'a>(
        image: &'a FirmwareImage,
        bootloader: Option<Vec<u8>>,
        partition_table: Option<PartitionTable>,
    ) -> Box<dyn Iterator<Item = Result<RomSegment<'a>, Error>> + 'a> {
        let bootloader = if let Some(bytes) = bootloader {
            Cow::Owned(bytes)
        } else {
            Cow::Borrowed(&include_bytes!("../../bootloader/esp32-bootloader.bin")[..])
        };

        match Esp32BootloaderFormat::new(image, Chip::Esp32, PARAMS, partition_table, bootloader) {
            Ok(format) => Box::new(format.segments().map(Ok)),
            Err(e) => Box::new(once(Err(e))),
        }
    }
}

#[test]
fn test_esp32_rom() {
    use std::fs::read;

    let input_bytes = read("./tests/data/esp32").unwrap();
    let expected_bin = read("./tests/data/esp32.bin").unwrap();

    let image = FirmwareImage::from_data(&input_bytes).unwrap();

    let segments = Esp32::get_flash_segments(&image, None, None)
        .collect::<Result<Vec<_>, Error>>()
        .unwrap();

    assert_eq!(3, segments.len());
    let buff = segments[2].data.as_ref();
    assert_eq!(expected_bin.len(), buff.len());
    assert_eq!(&expected_bin.as_slice(), &buff);
}

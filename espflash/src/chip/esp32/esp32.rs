use std::ops::Range;

use super::Esp32Params;
use crate::{
    chip::{Chip, ChipType, SpiRegisters},
    elf::FirmwareImage,
    image_format::{Esp32BootloaderFormat, ImageFormat, ImageFormatId},
    Error, PartitionTable,
};

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
    default_bootloader: include_bytes!("../../../bootloader/esp32-bootloader.bin"),
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

    const DEFAULT_IMAGE_FORMAT: ImageFormatId = ImageFormatId::Bootloader;
    const SUPPORTED_IMAGE_FORMATS: &'static [ImageFormatId] = &[ImageFormatId::Bootloader];

    const SUPPORTED_TARGETS: &'static [&'static str] =
        &["xtensa-esp32-none-elf", "xtensa-esp32-espidf"];

    fn get_flash_segments<'a>(
        image: &'a FirmwareImage,
        bootloader: Option<Vec<u8>>,
        partition_table: Option<PartitionTable>,
        image_format: ImageFormatId,
    ) -> Result<Box<dyn ImageFormat<'a> + 'a>, Error> {
        match image_format {
            ImageFormatId::Bootloader => Ok(Box::new(Esp32BootloaderFormat::new(
                image,
                Chip::Esp32,
                PARAMS,
                partition_table,
                bootloader,
            )?)),
            ImageFormatId::DirectBoot => {
                todo!()
            }
        }
    }

    fn supports_target(target: &str) -> bool {
        target.starts_with("xtensa-esp32-")
    }
}

#[test]
fn test_esp32_rom() {
    use std::fs::read;

    let input_bytes = read("./tests/data/esp32").unwrap();
    let expected_bin = read("./tests/data/esp32.bin").unwrap();

    let image = FirmwareImage::from_data(&input_bytes).unwrap();
    let flash_image = Esp32BootloaderFormat::new(&image, Chip::Esp32, PARAMS, None, None).unwrap();

    let segments = flash_image.segments().collect::<Vec<_>>();

    assert_eq!(3, segments.len());
    let buff = segments[2].data.as_ref();
    assert_eq!(expected_bin.len(), buff.len());
    assert_eq!(&expected_bin.as_slice(), &buff);
}

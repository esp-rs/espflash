use std::ops::Range;

use super::Esp32Params;
use crate::{
    chip::{ChipType, ReadEFuse, SpiRegisters},
    elf::FirmwareImage,
    image_format::{Esp32BootloaderFormat, ImageFormat, ImageFormatId},
    Chip, Error, PartitionTable,
};

pub struct Esp32c3;

const IROM_MAP_START: u32 = 0x42000000;
const IROM_MAP_END: u32 = 0x42800000;

const DROM_MAP_START: u32 = 0x3c000000;
const DROM_MAP_END: u32 = 0x3c800000;

pub const PARAMS: Esp32Params = Esp32Params {
    boot_addr: 0x0,
    partition_addr: 0x8000,
    nvs_addr: 0x9000,
    nvs_size: 0x6000,
    phy_init_data_addr: 0xf000,
    phy_init_data_size: 0x1000,
    app_addr: 0x10000,
    app_size: 0x3f0000,
    chip_id: 5,
    default_bootloader: include_bytes!("../../../bootloader/esp32c3-bootloader.bin"),
};

impl ChipType for Esp32c3 {
    const CHIP_DETECT_MAGIC_VALUE: u32 = 0x6921506f;
    const CHIP_DETECT_MAGIC_VALUE2: u32 = 0x1b31506f;

    const SPI_REGISTERS: SpiRegisters = SpiRegisters {
        base: 0x60002000,
        usr_offset: 0x18,
        usr1_offset: 0x1C,
        usr2_offset: 0x20,
        w0_offset: 0x58,
        mosi_length_offset: Some(0x24),
        miso_length_offset: Some(0x28),
    };

    const FLASH_RANGES: &'static [Range<u32>] =
        &[IROM_MAP_START..IROM_MAP_END, DROM_MAP_START..DROM_MAP_END];

    const DEFAULT_IMAGE_FORMAT: ImageFormatId = ImageFormatId::Bootloader;
    const SUPPORTED_IMAGE_FORMATS: &'static [ImageFormatId] =
        &[ImageFormatId::Bootloader, ImageFormatId::DirectBoot];

    const SUPPORTED_TARGETS: &'static [&'static str] =
        &["riscv32imc-uknown-none-elf", "riscv32imc-esp-espidf"];

    fn get_flash_segments<'a>(
        image: &'a FirmwareImage,
        bootloader: Option<Vec<u8>>,
        partition_table: Option<PartitionTable>,
        image_format: ImageFormatId,
    ) -> Result<Box<dyn ImageFormat<'a> + 'a>, Error> {
        match image_format {
            ImageFormatId::Bootloader => Ok(Box::new(Esp32BootloaderFormat::new(
                image,
                Chip::Esp32c3,
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
        target.starts_with("riscv32imc-")
    }
}

impl ReadEFuse for Esp32c3 {
    const EFUSE_REG_BASE: u32 = 0x60008830;
}

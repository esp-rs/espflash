use crate::chip::Esp32Params;
use crate::image_format::{Esp32BootloaderFormat, ImageFormat};
use crate::{
    chip::{ChipType, SpiRegisters},
    elf::{FirmwareImage, RomSegment},
    Chip, Error, PartitionTable,
};
use std::{borrow::Cow, iter::once};

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

    fn addr_is_flash(addr: u32) -> bool {
        (IROM_MAP_START..IROM_MAP_END).contains(&addr)
            || (DROM_MAP_START..DROM_MAP_END).contains(&addr)
    }

    fn get_flash_segments<'a>(
        image: &'a FirmwareImage,
        bootloader: Option<Vec<u8>>,
        partition_table: Option<PartitionTable>,
    ) -> Box<dyn Iterator<Item = Result<RomSegment<'a>, Error>> + 'a> {
        let bootloader = if let Some(bytes) = bootloader {
            Cow::Owned(bytes)
        } else {
            Cow::Borrowed(&include_bytes!("../../bootloader/esp32c3-bootloader.bin")[..])
        };

        match Esp32BootloaderFormat::new(image, Chip::Esp32, PARAMS, partition_table, bootloader) {
            Ok(format) => Box::new(format.segments().map(Ok)),
            Err(e) => Box::new(once(Err(e))),
        }
    }
}

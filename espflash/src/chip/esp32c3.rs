use crate::chip::esp32::get_data;
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

const BOOT_ADDR: u32 = 0x0;
const PARTITION_ADDR: u32 = 0x8000;
const NVS_ADDR: u32 = 0x9000;
const PHY_INIT_DATA_ADDR: u32 = 0xf000;
const APP_ADDR: u32 = 0x10000;

const NVS_SIZE: u32 = 0x6000;
const PHY_INIT_DATA_SIZE: u32 = 0x1000;
const APP_SIZE: u32 = 0x3f0000;

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
            bytes
        } else {
            let bytes = include_bytes!("../../bootloader/esp32c3-bootloader.bin");
            bytes.to_vec()
        };

        let partition_table = if let Some(table) = partition_table {
            table
        } else {
            PartitionTable::basic(
                NVS_ADDR,
                NVS_SIZE,
                PHY_INIT_DATA_ADDR,
                PHY_INIT_DATA_SIZE,
                APP_ADDR,
                APP_SIZE,
            )
        };
        let partition_table = partition_table.to_bytes();

        Box::new(
            once(Ok(RomSegment {
                addr: BOOT_ADDR,
                data: Cow::Owned(bootloader),
            }))
            .chain(once(Ok(RomSegment {
                addr: PARTITION_ADDR,
                data: Cow::Owned(partition_table),
            })))
            .chain(once(get_data(image, 5, Chip::Esp32c3))),
        )
    }
}

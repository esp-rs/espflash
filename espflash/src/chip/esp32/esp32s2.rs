use std::ops::Range;

use super::Esp32Params;
use crate::{
    chip::{ChipType, ReadEFuse, SpiRegisters},
    connection::Connection,
    elf::FirmwareImage,
    error::UnsupportedImageFormatError,
    image_format::{Esp32BootloaderFormat, ImageFormat, ImageFormatId},
    Chip, Error, PartitionTable,
};

pub struct Esp32s2;

const IROM_MAP_START: u32 = 0x40080000;
const IROM_MAP_END: u32 = 0x40b80000;

const DROM_MAP_START: u32 = 0x3F000000;
const DROM_MAP_END: u32 = 0x3F3F0000;

pub const PARAMS: Esp32Params = Esp32Params {
    boot_addr: 0x1000,
    partition_addr: 0x8000,
    nvs_addr: 0x9000,
    nvs_size: 0x6000,
    phy_init_data_addr: 0xf000,
    phy_init_data_size: 0x1000,
    app_addr: 0x10000,
    app_size: 0x100000,
    chip_id: 2,
    default_bootloader: include_bytes!("../../../bootloader/esp32s2-bootloader.bin"),
};

impl ChipType for Esp32s2 {
    const CHIP_DETECT_MAGIC_VALUE: u32 = 0x000007c6;

    const UART_CLKDIV_REG: u32 = 0x3f400014;

    const SPI_REGISTERS: SpiRegisters = SpiRegisters {
        base: 0x3f402000,
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
    const SUPPORTED_IMAGE_FORMATS: &'static [ImageFormatId] = &[ImageFormatId::Bootloader];

    const SUPPORTED_TARGETS: &'static [&'static str] =
        &["xtensa-esp32s2-none-elf", "xtensa-esp32s2-espidf"];

    fn chip_features(&self, connection: &mut Connection) -> Result<Vec<&str>, Error> {
        let mut features = vec!["WiFi"];

        let flash_version = match self.get_flash_version(connection)? {
            0 => "No Embedded Flash",
            1 => "Embedded Flash 2MB",
            2 => "Embedded Flash 4MB",
            _ => "Unknown Embedded Flash",
        };
        features.push(flash_version);

        let psram_version = match self.get_psram_version(connection)? {
            0 => "No Embedded PSRAM",
            1 => "Embedded PSRAM 2MB",
            2 => "Embedded PSRAM 4MB",
            _ => "Unknown Embedded PSRAM",
        };
        features.push(psram_version);

        let block2_version = match self.get_block2_version(connection)? {
            0 => "No calibration in BLK2 of efuse",
            1 => "ADC and temperature sensor calibration in BLK2 of efuse V1",
            2 => "ADC and temperature sensor calibration in BLK2 of efuse V2",
            _ => "Unknown Calibration in BLK2",
        };
        features.push(block2_version);

        Ok(features)
    }

    fn crystal_freq(&self, _connection: &mut Connection) -> Result<u32, Error> {
        // The ESP32-S2's XTAL has a fixed frequency of 40MHz.
        Ok(40)
    }

    fn get_flash_segments<'a>(
        image: &'a FirmwareImage,
        bootloader: Option<Vec<u8>>,
        partition_table: Option<PartitionTable>,
        image_format: ImageFormatId,
        _chip_revision: Option<u32>,
    ) -> Result<Box<dyn ImageFormat<'a> + 'a>, Error> {
        match image_format {
            ImageFormatId::Bootloader => Ok(Box::new(Esp32BootloaderFormat::new(
                image,
                Chip::Esp32s2,
                PARAMS,
                partition_table,
                bootloader,
            )?)),
            _ => Err(UnsupportedImageFormatError::new(image_format, Chip::Esp32s2, None).into()),
        }
    }
}

impl ReadEFuse for Esp32s2 {
    const EFUSE_REG_BASE: u32 = 0x3F41A030;
}

impl Esp32s2 {
    fn get_flash_version(&self, connection: &mut Connection) -> Result<u32, Error> {
        let blk1_word3 = self.read_efuse(connection, 8)?;
        let flash_version = (blk1_word3 >> 21) & 0xf;

        Ok(flash_version)
    }

    fn get_psram_version(&self, connection: &mut Connection) -> Result<u32, Error> {
        let blk1_word3 = self.read_efuse(connection, 8)?;
        let psram_version = (blk1_word3 >> 28) & 0xf;

        Ok(psram_version)
    }

    fn get_block2_version(&self, connection: &mut Connection) -> Result<u32, Error> {
        let blk2_word4 = self.read_efuse(connection, 15)?;
        let block2_version = (blk2_word4 >> 4) & 0x7;

        Ok(block2_version)
    }
}

use std::ops::Range;

use super::Esp32Params;
use crate::{
    chip::{ChipType, ReadEFuse, SpiRegisters},
    connection::Connection,
    elf::{FirmwareImage, FlashFrequency, FlashMode},
    flasher::FlashSize,
    image_format::{Esp32BootloaderFormat, Esp32DirectBootFormat, ImageFormat, ImageFormatId},
    Chip, Error, PartitionTable,
};

pub struct Esp32s3;

pub const PARAMS: Esp32Params = Esp32Params::new(
    0x0,
    0x10000,
    0x100000,
    9,
    include_bytes!("../../../resources/bootloaders/esp32s3-bootloader.bin"),
);

impl ChipType for Esp32s3 {
    const CHIP_DETECT_MAGIC_VALUES: &'static [u32] = &[0x9];

    const UART_CLKDIV_REG: u32 = 0x60000014;

    const SPI_REGISTERS: SpiRegisters = SpiRegisters {
        base: 0x60002000,
        usr_offset: 0x18,
        usr1_offset: 0x1C,
        usr2_offset: 0x20,
        w0_offset: 0x58,
        mosi_length_offset: Some(0x24),
        miso_length_offset: Some(0x28),
    };

    const FLASH_RANGES: &'static [Range<u32>] = &[
        0x42000000..0x44000000, // IROM
        0x3c000000..0x3e000000, // DROM
    ];

    const SUPPORTED_IMAGE_FORMATS: &'static [ImageFormatId] =
        &[ImageFormatId::Bootloader, ImageFormatId::DirectBoot];

    const SUPPORTED_TARGETS: &'static [&'static str] =
        &["xtensa-esp32s3-none-elf", "xtensa-esp32s3-espidf"];

    fn chip_features(&self, _connection: &mut Connection) -> Result<Vec<&str>, Error> {
        Ok(vec!["WiFi", "BLE"])
    }

    fn crystal_freq(&self, _connection: &mut Connection) -> Result<u32, Error> {
        // The ESP32-S3's XTAL has a fixed frequency of 40MHz.
        Ok(40)
    }

    fn get_flash_segments<'a>(
        image: &'a dyn FirmwareImage<'a>,
        bootloader: Option<Vec<u8>>,
        partition_table: Option<PartitionTable>,
        image_format: ImageFormatId,
        _chip_revision: Option<u32>,
        flash_mode: Option<FlashMode>,
        flash_size: Option<FlashSize>,
        flash_freq: Option<FlashFrequency>,
    ) -> Result<Box<dyn ImageFormat<'a> + 'a>, Error> {
        match image_format {
            ImageFormatId::Bootloader => Ok(Box::new(Esp32BootloaderFormat::new(
                image,
                Chip::Esp32s3,
                PARAMS,
                partition_table,
                bootloader,
                flash_mode,
                flash_size,
                flash_freq,
            )?)),
            ImageFormatId::DirectBoot => Ok(Box::new(Esp32DirectBootFormat::new(image, 0x400)?)),
        }
    }
}

impl ReadEFuse for Esp32s3 {
    const EFUSE_REG_BASE: u32 = 0x60007030;
}

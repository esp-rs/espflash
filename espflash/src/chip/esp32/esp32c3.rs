use std::ops::Range;

use super::Esp32Params;
use crate::{
    chip::{ChipType, ReadEFuse, SpiRegisters},
    connection::Connection,
    elf::{FirmwareImage, FlashFrequency, FlashMode},
    error::UnsupportedImageFormatError,
    flasher::FlashSize,
    image_format::{Esp32BootloaderFormat, Esp32DirectBootFormat, ImageFormat, ImageFormatId},
    Chip, Error, PartitionTable,
};

pub struct Esp32c3;

pub const PARAMS: Esp32Params = Esp32Params::new(
    0x0,
    0x10000,
    0x3f0000,
    5,
    include_bytes!("../../../resources/bootloaders/esp32c3-bootloader.bin"),
);

impl ChipType for Esp32c3 {
    const CHIP_DETECT_MAGIC_VALUES: &'static [u32] = &[
        0x6921506f, // ECO1 + ECO2
        0x1b31506f, // ECO3
    ];

    const UART_CLKDIV_REG: u32 = 0x3ff40014;

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
        0x42000000..0x42800000, // IROM
        0x3c000000..0x3c800000, // DROM
    ];

    const SUPPORTED_IMAGE_FORMATS: &'static [ImageFormatId] =
        &[ImageFormatId::Bootloader, ImageFormatId::DirectBoot];

    const SUPPORTED_TARGETS: &'static [&'static str] = &[
        "riscv32imac-unknown-none-elf",
        "riscv32imc-esp-espidf",
        "riscv32imc-unknown-none-elf",
    ];

    fn chip_features(&self, _connection: &mut Connection) -> Result<Vec<&str>, Error> {
        Ok(vec!["WiFi"])
    }

    fn crystal_freq(&self, _connection: &mut Connection) -> Result<u32, Error> {
        // The ESP32-C3's XTAL has a fixed frequency of 40MHz.
        Ok(40)
    }

    fn get_flash_segments<'a>(
        image: &'a dyn FirmwareImage<'a>,
        bootloader: Option<Vec<u8>>,
        partition_table: Option<PartitionTable>,
        image_format: ImageFormatId,
        chip_revision: Option<u32>,
        flash_mode: Option<FlashMode>,
        flash_size: Option<FlashSize>,
        flash_freq: Option<FlashFrequency>,
    ) -> Result<Box<dyn ImageFormat<'a> + 'a>, Error> {
        match (image_format, chip_revision) {
            (ImageFormatId::Bootloader, _) => Ok(Box::new(Esp32BootloaderFormat::new(
                image,
                Chip::Esp32c3,
                PARAMS,
                partition_table,
                bootloader,
                flash_mode,
                flash_size,
                flash_freq,
            )?)),
            (ImageFormatId::DirectBoot, None | Some(3..)) => {
                Ok(Box::new(Esp32DirectBootFormat::new(image, 0)?))
            }
            _ => Err(
                UnsupportedImageFormatError::new(image_format, Chip::Esp32c3, chip_revision).into(),
            ),
        }
    }
}

impl ReadEFuse for Esp32c3 {
    const EFUSE_REG_BASE: u32 = 0x60008830;
}

impl Esp32c3 {
    pub fn chip_revision(&self, connection: &mut Connection) -> Result<u32, Error> {
        let block1_addr = Self::EFUSE_REG_BASE + 0x14;
        let num_word = 3;
        let pos = 18;

        let value = connection.read_reg(block1_addr + (num_word * 0x4))?;
        let value = (value & (0x7 << pos)) >> pos;

        Ok(value)
    }
}

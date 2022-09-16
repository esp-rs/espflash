use std::{collections::HashMap, ops::Range};

use maplit::hashmap;

use super::Esp32Params;
use crate::{
    chip::{ChipType, ReadEFuse, SpiRegisters},
    connection::Connection,
    elf::{FirmwareImage, FlashFrequency, FlashMode},
    flasher::FlashSize,
    image_format::{Esp32BootloaderFormat, Esp32DirectBootFormat, ImageFormat, ImageFormatId},
    Chip, Error, PartitionTable,
};

pub struct Esp32c2;

pub const PARAMS: Esp32Params = Esp32Params::new(
    0x0,
    0x10000,
    0x1f0000,
    12,
    include_bytes!("../../../resources/bootloaders/esp32c2-bootloader.bin"),
);

impl ChipType for Esp32c2 {
    const CHIP_DETECT_MAGIC_VALUES: &'static [u32] = &[
        0x6F51306F, // ECO0
        0x7C41A06F, // ECO1
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
        0x42000000..0x42400000, // IROM
        0x3C000000..0x3C400000, // DROM
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
        // The ESP32-C2's XTAL has a fixed frequency of 40MHz.
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
                Chip::Esp32c2,
                PARAMS,
                partition_table,
                bootloader,
                flash_mode,
                flash_size,
                flash_freq,
            )?)),
            ImageFormatId::DirectBoot => Ok(Box::new(Esp32DirectBootFormat::new(image, 0)?)),
        }
    }

    fn flash_frequency_encodings() -> HashMap<FlashFrequency, u8> {
        use FlashFrequency::*;

        hashmap! {
            Flash15M => 0x2,
            Flash20M => 0x1,
            Flash30M => 0x0,
            Flash60M => 0xF,
        }
    }
}

impl ReadEFuse for Esp32c2 {
    const EFUSE_REG_BASE: u32 = 0x60008800;
}

impl Esp32c2 {
    pub fn chip_revision(&self, connection: &mut Connection) -> Result<u32, Error> {
        let block1_addr = Self::EFUSE_REG_BASE + 0x44;
        let num_word = 3;
        let pos = 18;

        let value = connection.read_reg(block1_addr + (num_word * 0x4))?;
        let value = (value & (0x7 << pos)) >> pos;

        Ok(value)
    }
}

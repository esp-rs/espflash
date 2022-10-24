use std::ops::Range;

use esp_idf_part::PartitionTable;

use super::{Chip, Esp32Params, ReadEFuse, SpiRegisters, Target};
use crate::{
    connection::Connection,
    elf::FirmwareImage,
    error::{Error, UnsupportedImageFormatError},
    flasher::{FlashFrequency, FlashMode, FlashSize},
    image_format::{DirectBootFormat, IdfBootloaderFormat, ImageFormat, ImageFormatKind},
};

const CHIP_DETECT_MAGIC_VALUES: &[u32] = &[
    0x6921_506f, // ECO1 + ECO2
    0x1b31_506f, // ECO3
];

const FLASH_RANGES: &[Range<u32>] = &[
    0x4200_0000..0x4280_0000, // IROM
    0x3c00_0000..0x3c80_0000, // DROM
];

const PARAMS: Esp32Params = Esp32Params::new(
    0x0,
    0x1_0000,
    0x3f_0000,
    5,
    include_bytes!("../../resources/bootloaders/esp32c3-bootloader.bin"),
);

/// ESP32-C3 Target
pub struct Esp32c3;

impl Esp32c3 {
    pub fn has_magic_value(value: u32) -> bool {
        CHIP_DETECT_MAGIC_VALUES.contains(&value)
    }
}

impl ReadEFuse for Esp32c3 {
    fn efuse_reg(&self) -> u32 {
        0x6000_8830
    }
}

impl Target for Esp32c3 {
    fn addr_is_flash(&self, addr: u32) -> bool {
        FLASH_RANGES.iter().any(|range| range.contains(&addr))
    }

    fn chip_features(&self, _connection: &mut Connection) -> Result<Vec<&str>, Error> {
        Ok(vec!["WiFi"])
    }

    fn chip_revision(&self, connection: &mut Connection) -> Result<Option<u32>, Error> {
        let block1_addr = self.efuse_reg() + 0x14;
        let num_word = 3;
        let pos = 18;

        let value = connection.read_reg(block1_addr + (num_word * 0x4))?;
        let value = (value & (0x7 << pos)) >> pos;

        Ok(Some(value))
    }

    fn crystal_freq(&self, _connection: &mut Connection) -> Result<u32, Error> {
        // The ESP32-C3's XTAL has a fixed frequency of 40MHz.
        Ok(40)
    }

    fn get_flash_image<'a>(
        &self,
        image: &'a dyn FirmwareImage<'a>,
        bootloader: Option<Vec<u8>>,
        partition_table: Option<PartitionTable>,
        image_format: Option<ImageFormatKind>,
        chip_revision: Option<u32>,
        flash_mode: Option<FlashMode>,
        flash_size: Option<FlashSize>,
        flash_freq: Option<FlashFrequency>,
    ) -> Result<Box<dyn ImageFormat<'a> + 'a>, Error> {
        let image_format = image_format.unwrap_or(ImageFormatKind::EspBootloader);

        match (image_format, chip_revision) {
            (ImageFormatKind::EspBootloader, _) => Ok(Box::new(IdfBootloaderFormat::new(
                image,
                Chip::Esp32c3,
                PARAMS,
                partition_table,
                bootloader,
                flash_mode,
                flash_size,
                flash_freq,
            )?)),
            (ImageFormatKind::DirectBoot, None | Some(3..)) => {
                Ok(Box::new(DirectBootFormat::new(image, 0)?))
            }
            _ => Err(
                UnsupportedImageFormatError::new(image_format, Chip::Esp32c3, chip_revision).into(),
            ),
        }
    }

    fn spi_registers(&self) -> SpiRegisters {
        SpiRegisters {
            base: 0x6000_2000,
            usr_offset: 0x18,
            usr1_offset: 0x1C,
            usr2_offset: 0x20,
            w0_offset: 0x58,
            mosi_length_offset: Some(0x24),
            miso_length_offset: Some(0x28),
        }
    }

    fn supported_image_formats(&self) -> &[ImageFormatKind] {
        &[ImageFormatKind::EspBootloader, ImageFormatKind::DirectBoot]
    }

    fn supported_build_targets(&self) -> &[&str] {
        &[
            "riscv32imac-unknown-none-elf",
            "riscv32imc-esp-espidf",
            "riscv32imc-unknown-none-elf",
        ]
    }
}

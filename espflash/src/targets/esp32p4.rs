use std::ops::Range;

use esp_idf_part::PartitionTable;

use super::{Chip, Esp32Params, ReadEFuse, SpiRegisters, Target};
use crate::{
    connection::Connection,
    elf::FirmwareImage,
    error::Error,
    flasher::{FlashFrequency, FlashMode, FlashSize},
    image_format::{DirectBootFormat, IdfBootloaderFormat, ImageFormat, ImageFormatKind},
};

const CHIP_DETECT_MAGIC_VALUES: &[u32] = &[0x0];

const FLASH_RANGES: &[Range<u32>] = &[
    0x4000_0000..0x4C00_0000, // IROM
    0x4000_0000..0x4C00_0000, // DROM
];

const PARAMS: Esp32Params = Esp32Params::new(
    0x2000,
    0x1_0000,
    0x3f_0000, // TODO: Update
    18,
    FlashFrequency::_40Mhz,
    include_bytes!("../../resources/bootloaders/esp32p4-bootloader.bin"),
);

/// ESP32-P4 Target
pub struct Esp32p4;

impl Esp32p4 {
    pub fn has_magic_value(value: u32) -> bool {
        CHIP_DETECT_MAGIC_VALUES.contains(&value)
    }
}

impl ReadEFuse for Esp32p4 {
    fn efuse_reg(&self) -> u32 {
        0x5012_D000
    }
}

impl Target for Esp32p4 {
    fn addr_is_flash(&self, addr: u32) -> bool {
        FLASH_RANGES.iter().any(|range| range.contains(&addr))
    }

    fn chip_features(&self, _connection: &mut Connection) -> Result<Vec<&str>, Error> {
        Ok(vec!["High-Performance MCU"])
    }

    fn major_chip_version(&self, _connection: &mut Connection) -> Result<u32, Error> {
        // TODO: https://github.com/espressif/esptool/blob/master/esptool/targets/esp32p4.py#L96
        Ok(0)
    }

    fn minor_chip_version(&self, _connection: &mut Connection) -> Result<u32, Error> {
        // TODO: https://github.com/espressif/esptool/blob/master/esptool/targets/esp32p4.py#L92

        Ok(0)
    }

    fn crystal_freq(&self, _connection: &mut Connection) -> Result<u32, Error> {
        // The ESP32-P4's XTAL has a fixed frequency of 40MHz.
        Ok(40)
    }

    fn get_flash_image<'a>(
        &self,
        image: &'a dyn FirmwareImage<'a>,
        bootloader: Option<Vec<u8>>,
        partition_table: Option<PartitionTable>,
        target_app_partition: Option<String>,
        image_format: Option<ImageFormatKind>,
        _chip_revision: Option<(u32, u32)>,
        flash_mode: Option<FlashMode>,
        flash_size: Option<FlashSize>,
        flash_freq: Option<FlashFrequency>,
        partition_table_offset: Option<u32>,
    ) -> Result<Box<dyn ImageFormat<'a> + 'a>, Error> {
        let image_format = image_format.unwrap_or(ImageFormatKind::EspBootloader);

        match image_format {
            ImageFormatKind::EspBootloader => Ok(Box::new(IdfBootloaderFormat::new(
                image,
                Chip::Esp32p4,
                PARAMS,
                partition_table,
                target_app_partition,
                bootloader,
                flash_mode,
                flash_size,
                flash_freq,
                partition_table_offset,
            )?)),
            ImageFormatKind::DirectBoot => Ok(Box::new(DirectBootFormat::new(image, 0x0)?)),
        }
    }

    fn spi_registers(&self) -> SpiRegisters {
        SpiRegisters {
            base: 0x5008_D000,
            usr_offset: 0x18,
            usr1_offset: 0x1c,
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
        &["riscv32imafc-esp-espidf", "riscv32imafc-unknown-none-elf"]
    }
}

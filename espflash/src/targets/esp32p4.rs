use std::ops::Range;

use super::{Chip, Esp32Params, ReadEFuse, SpiRegisters, Target};
use crate::{
    connection::Connection,
    elf::FirmwareImage,
    error::Error,
    flasher::{FlashData, FlashFrequency},
    image_format::{DirectBootFormat, IdfBootloaderFormat, ImageFormat, ImageFormatKind},
    targets::XtalFrequency,
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

    fn crystal_freq(&self, _connection: &mut Connection) -> Result<XtalFrequency, Error> {
        // The ESP32-P4's XTAL has a fixed frequency of 40MHz.
        Ok(XtalFrequency::_40Mhz)
    }

    fn get_flash_image<'a>(
        &self,
        image: &'a dyn FirmwareImage<'a>,
        flash_data: FlashData,
        _chip_revision: Option<(u32, u32)>,
        xtal_freq: XtalFrequency,
    ) -> Result<Box<dyn ImageFormat<'a> + 'a>, Error> {
        let image_format = flash_data
            .image_format
            .unwrap_or(ImageFormatKind::EspBootloader);

        if xtal_freq != XtalFrequency::_40Mhz {
            return Err(Error::UnsupportedFeature {
                chip: Chip::Esp32p4,
                feature: "the selected crystal frequency".into(),
            });
        }

        match image_format {
            ImageFormatKind::EspBootloader => Ok(Box::new(IdfBootloaderFormat::new(
                image,
                Chip::Esp32p4,
                flash_data.min_chip_rev,
                PARAMS,
                flash_data.partition_table,
                flash_data.partition_table_offset,
                flash_data.target_app_partition,
                flash_data.bootloader,
                flash_data.flash_settings,
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

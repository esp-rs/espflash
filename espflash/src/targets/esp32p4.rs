use std::ops::Range;

#[cfg(feature = "serialport")]
use crate::connection::Connection;
use crate::{
    elf::FirmwareImage,
    error::Error,
    flasher::{FlashData, FlashFrequency},
    image_format::IdfBootloaderFormat,
    targets::{Chip, Esp32Params, ReadEFuse, SpiRegisters, Target, XtalFrequency},
};

const CHIP_DETECT_MAGIC_VALUES: &[u32] = &[0x0, 0x0ADDBAD0];

const FLASH_RANGES: &[Range<u32>] = &[
    0x4000_0000..0x4C00_0000, // IROM
    0x4000_0000..0x4C00_0000, // DROM
];

const PARAMS: Esp32Params = Esp32Params::new(
    0x2000,
    0x1_0000,
    0x3f_0000,
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

    #[cfg(feature = "serialport")]
    fn chip_features(&self, _connection: &mut Connection) -> Result<Vec<&str>, Error> {
        Ok(vec!["High-Performance MCU"])
    }

    #[cfg(feature = "serialport")]
    fn major_chip_version(&self, connection: &mut Connection) -> Result<u32, Error> {
        Ok(self.read_efuse(connection, 19)? >> 4 & 0x03)
    }

    #[cfg(feature = "serialport")]
    fn minor_chip_version(&self, connection: &mut Connection) -> Result<u32, Error> {
        Ok(self.read_efuse(connection, 19)? & 0x0F)
    }

    #[cfg(feature = "serialport")]
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
    ) -> Result<IdfBootloaderFormat<'a>, Error> {
        if xtal_freq != XtalFrequency::_40Mhz {
            return Err(Error::UnsupportedFeature {
                chip: Chip::Esp32p4,
                feature: "the selected crystal frequency".into(),
            });
        }

        IdfBootloaderFormat::new(
            image,
            Chip::Esp32p4,
            flash_data.min_chip_rev,
            PARAMS,
            flash_data.partition_table,
            flash_data.partition_table_offset,
            flash_data.target_app_partition,
            flash_data.bootloader,
            flash_data.flash_settings,
        )
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

    fn supported_build_targets(&self) -> &[&str] {
        &["riscv32imafc-esp-espidf", "riscv32imafc-unknown-none-elf"]
    }
}

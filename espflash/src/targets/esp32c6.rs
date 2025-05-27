use std::ops::Range;

use super::{
    Chip,
    Esp32Params,
    ReadEFuse,
    SpiRegisters,
    Target,
    XtalFrequency,
    efuse::esp32c6 as efuse,
};
#[cfg(feature = "serialport")]
use crate::connection::Connection;
use crate::{
    Error,
    flasher::{FlashData, FlashFrequency},
    image_format::{self, IdfBootloaderFormat, ImageFormat, ImageFormatKind},
};

pub(crate) const CHIP_ID: u16 = 13;

const CHIP_DETECT_MAGIC_VALUES: &[u32] = &[0x2CE0_806F];

const FLASH_RANGES: &[Range<u32>] = &[
    0x4200_0000..0x4280_0000, // IROM
    0x4280_0000..0x4300_0000, // DROM
];

/// ESP32-C6 Target
pub struct Esp32c6;

impl Esp32c6 {
    /// Check if the magic value contains the specified value
    pub fn has_magic_value(value: u32) -> bool {
        CHIP_DETECT_MAGIC_VALUES.contains(&value)
    }
}

impl ReadEFuse for Esp32c6 {
    fn efuse_reg(&self) -> u32 {
        0x600B_0800
    }

    fn block0_offset(&self) -> u32 {
        0x2C
    }

    fn block_size(&self, block: usize) -> u32 {
        efuse::BLOCK_SIZES[block]
    }
}

impl Target for Esp32c6 {
    fn chip(&self) -> Chip {
        Chip::Esp32c6
    }

    fn addr_is_flash(&self, addr: u32) -> bool {
        FLASH_RANGES.iter().any(|range| range.contains(&addr))
    }

    #[cfg(feature = "serialport")]
    fn chip_features(&self, _connection: &mut Connection) -> Result<Vec<&str>, Error> {
        Ok(vec!["WiFi 6", "BT 5"])
    }

    #[cfg(feature = "serialport")]
    fn major_chip_version(&self, connection: &mut Connection) -> Result<u32, Error> {
        self.read_efuse(connection, efuse::WAFER_VERSION_MAJOR)
    }

    #[cfg(feature = "serialport")]
    fn minor_chip_version(&self, connection: &mut Connection) -> Result<u32, Error> {
        self.read_efuse(connection, efuse::WAFER_VERSION_MINOR)
    }

    #[cfg(feature = "serialport")]
    fn crystal_freq(&self, _connection: &mut Connection) -> Result<XtalFrequency, Error> {
        // The ESP32-C6's XTAL has a fixed frequency of 40MHz.
        Ok(XtalFrequency::_40Mhz)
    }

    fn flash_image<'a>(
        &self,
        format: ImageFormatKind,
        elf_data: &'a [u8],
        flash_data: FlashData,
        _chip_revision: Option<(u32, u32)>,
        xtal_freq: XtalFrequency,
    ) -> Result<ImageFormat<'a>, Error> {
        let bootloader: &'static [u8] = match format {
            ImageFormatKind::EspIdf => image_format::esp_idf::bootloader(Chip::Esp32c6, xtal_freq)?,
        };

        let params = Esp32Params::new(
            0x0,
            0x1_0000,
            0x3f_0000,
            CHIP_ID,
            FlashFrequency::_40Mhz,
            bootloader,
            Some(&[8 * 1024, 16 * 1024, 32 * 1024, 64 * 1024]),
        );

        match format {
            ImageFormatKind::EspIdf => {
                let idf = IdfBootloaderFormat::new(elf_data, Chip::Esp32c6, flash_data, params)?;
                Ok(idf.into())
            }
        }
    }

    fn spi_registers(&self) -> SpiRegisters {
        SpiRegisters {
            base: 0x6000_3000,
            usr_offset: 0x18,
            usr1_offset: 0x1c,
            usr2_offset: 0x20,
            w0_offset: 0x58,
            mosi_length_offset: Some(0x24),
            miso_length_offset: Some(0x28),
        }
    }

    fn supported_build_targets(&self) -> &[&str] {
        &["riscv32imac-esp-espidf", "riscv32imac-unknown-none-elf"]
    }
}

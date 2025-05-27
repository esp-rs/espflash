use std::ops::Range;

use super::{
    Chip,
    Esp32Params,
    ReadEFuse,
    SpiRegisters,
    Target,
    XtalFrequency,
    efuse::esp32c3 as efuse,
};
#[cfg(feature = "serialport")]
use crate::connection::Connection;
use crate::{
    Error,
    flasher::{FlashData, FlashFrequency},
    image_format::{self, IdfBootloaderFormat, ImageFormat, ImageFormatKind},
};

pub(crate) const CHIP_ID: u16 = 5;

const CHIP_DETECT_MAGIC_VALUES: &[u32] = &[
    0x6921_506f, // ECO1 + ECO2
    0x1b31_506f, // ECO3
    0x4881_606F, // ECO6
    0x4361_606f, // ECO7
];

const FLASH_RANGES: &[Range<u32>] = &[
    0x4200_0000..0x4280_0000, // IROM
    0x3c00_0000..0x3c80_0000, // DROM
];

/// ESP32-C3 Target
pub struct Esp32c3;

impl Esp32c3 {
    /// Check if the magic value contains the specified value
    pub fn has_magic_value(value: u32) -> bool {
        CHIP_DETECT_MAGIC_VALUES.contains(&value)
    }
}

impl ReadEFuse for Esp32c3 {
    fn efuse_reg(&self) -> u32 {
        0x6000_8800
    }

    fn block0_offset(&self) -> u32 {
        0x2D
    }

    fn block_size(&self, block: usize) -> u32 {
        efuse::BLOCK_SIZES[block]
    }
}

impl Target for Esp32c3 {
    fn chip(&self) -> Chip {
        Chip::Esp32c3
    }

    fn addr_is_flash(&self, addr: u32) -> bool {
        FLASH_RANGES.iter().any(|range| range.contains(&addr))
    }

    #[cfg(feature = "serialport")]
    fn chip_features(&self, _connection: &mut Connection) -> Result<Vec<&str>, Error> {
        Ok(vec!["WiFi", "BLE"])
    }

    #[cfg(feature = "serialport")]
    fn major_chip_version(&self, connection: &mut Connection) -> Result<u32, Error> {
        self.read_efuse(connection, efuse::WAFER_VERSION_MAJOR)
    }

    #[cfg(feature = "serialport")]
    fn minor_chip_version(&self, connection: &mut Connection) -> Result<u32, Error> {
        let hi = self.read_efuse(connection, efuse::WAFER_VERSION_MINOR_HI)?;
        let lo = self.read_efuse(connection, efuse::WAFER_VERSION_MINOR_LO)?;

        Ok((hi << 3) + lo)
    }

    #[cfg(feature = "serialport")]
    fn crystal_freq(&self, _connection: &mut Connection) -> Result<XtalFrequency, Error> {
        // The ESP32-C3's XTAL has a fixed frequency of 40MHz.
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
            ImageFormatKind::EspIdf => image_format::esp_idf::bootloader(Chip::Esp32c3, xtal_freq)?,
        };

        let params = Esp32Params::new(
            0x0,
            0x1_0000,
            0x3f_0000,
            CHIP_ID,
            FlashFrequency::_40Mhz,
            bootloader,
            None,
        );

        match format {
            ImageFormatKind::EspIdf => {
                let idf = IdfBootloaderFormat::new(elf_data, Chip::Esp32c3, flash_data, params)?;
                Ok(idf.into())
            }
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

    fn supported_build_targets(&self) -> &[&str] {
        &[
            "riscv32imac-unknown-none-elf",
            "riscv32imc-esp-espidf",
            "riscv32imc-unknown-none-elf",
        ]
    }
}

#[cfg(feature = "serialport")]
impl super::RtcWdtReset for Esp32c3 {
    fn wdt_wprotect(&self) -> u32 {
        0x6000_80A8
    }

    fn wdt_config0(&self) -> u32 {
        0x6000_8090
    }

    fn wdt_config1(&self) -> u32 {
        0x6000_8094
    }

    fn can_rtc_wdt_reset(&self, _connection: &mut Connection) -> Result<bool, Error> {
        Ok(true)
    }
}

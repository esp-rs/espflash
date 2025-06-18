use std::ops::Range;

#[cfg(feature = "serialport")]
use super::XtalFrequency;
use super::{Chip, SpiRegisters, Target, efuse::esp32c6 as efuse};
#[cfg(feature = "serialport")]
use crate::{Error, connection::Connection};

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
        self.chip()
            .read_efuse(connection, efuse::WAFER_VERSION_MAJOR)
    }

    #[cfg(feature = "serialport")]
    fn minor_chip_version(&self, connection: &mut Connection) -> Result<u32, Error> {
        self.chip()
            .read_efuse(connection, efuse::WAFER_VERSION_MINOR)
    }

    #[cfg(feature = "serialport")]
    fn crystal_freq(&self, _connection: &mut Connection) -> Result<XtalFrequency, Error> {
        // The ESP32-C6's XTAL has a fixed frequency of 40MHz.
        Ok(XtalFrequency::_40Mhz)
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

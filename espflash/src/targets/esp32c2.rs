use std::{collections::HashMap, ops::Range};

#[cfg(feature = "serialport")]
use super::XtalFrequency;
use super::{Chip, SpiRegisters, Target, efuse::esp32c2 as efuse};
use crate::flasher::FlashFrequency;
#[cfg(feature = "serialport")]
use crate::{Error, connection::Connection};

pub(crate) const CHIP_ID: u16 = 12;

const CHIP_DETECT_MAGIC_VALUES: &[u32] = &[
    0x6f51_306f, // ECO0
    0x7c41_a06f, // ECO1
];

const FLASH_RANGES: &[Range<u32>] = &[
    0x4200_0000..0x4240_0000, // IROM
    0x3c00_0000..0x3c40_0000, // DROM
];

#[cfg(feature = "serialport")]
const UART_CLKDIV_REG: u32 = 0x6000_0014; // UART0_BASE_REG + 0x14
#[cfg(feature = "serialport")]
const UART_CLKDIV_MASK: u32 = 0xfffff;

#[cfg(feature = "serialport")]
const XTAL_CLK_DIVIDER: u32 = 1;

/// ESP32-C2 Target
pub struct Esp32c2;

impl Esp32c2 {
    /// Check if the magic value contains the specified value
    pub fn has_magic_value(value: u32) -> bool {
        CHIP_DETECT_MAGIC_VALUES.contains(&value)
    }
}

impl Target for Esp32c2 {
    fn chip(&self) -> Chip {
        Chip::Esp32c2
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
        self.chip()
            .read_efuse(connection, efuse::WAFER_VERSION_MAJOR)
    }

    #[cfg(feature = "serialport")]
    fn minor_chip_version(&self, connection: &mut Connection) -> Result<u32, Error> {
        self.chip()
            .read_efuse(connection, efuse::WAFER_VERSION_MINOR)
    }

    #[cfg(feature = "serialport")]
    fn crystal_freq(&self, connection: &mut Connection) -> Result<XtalFrequency, Error> {
        let uart_div = connection.read_reg(UART_CLKDIV_REG)? & UART_CLKDIV_MASK;
        let est_xtal = (connection.baud()? * uart_div) / 1_000_000 / XTAL_CLK_DIVIDER;
        let norm_xtal = if est_xtal > 33 {
            XtalFrequency::_40Mhz
        } else {
            XtalFrequency::_26Mhz
        };

        Ok(norm_xtal)
    }

    fn flash_frequency_encodings(&self) -> HashMap<FlashFrequency, u8> {
        use FlashFrequency::*;

        let encodings = [(_15Mhz, 0x2), (_20Mhz, 0x1), (_30Mhz, 0x0), (_60Mhz, 0xF)];

        HashMap::from(encodings)
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

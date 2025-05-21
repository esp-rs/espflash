use std::ops::Range;

use super::{
    Chip,
    Esp32Params,
    ReadEFuse,
    SpiRegisters,
    Target,
    XtalFrequency,
    efuse::esp32c5 as efuse,
};
#[cfg(feature = "serialport")]
use crate::connection::Connection;
use crate::{
    Error,
    flasher::{FlashData, FlashFrequency},
    image_format::IdfBootloaderFormat,
};

pub(crate) const CHIP_ID: u16 = 23;

const CHIP_DETECT_MAGIC_VALUES: &[u32] = &[];

const FLASH_RANGES: &[Range<u32>] = &[
    0x42000000..0x44000000, // IROM
    0x42000000..0x44000000, // DROM
];

#[cfg(feature = "serialport")]
const UART_CLKDIV_REG: u32 = 0x6000_0014; // UART0_BASE_REG + 0x14
#[cfg(feature = "serialport")]
const UART_CLKDIV_MASK: u32 = 0xfffff;
#[cfg(feature = "serialport")]
const XTAL_CLK_DIVIDER: u32 = 1;

const PARAMS: Esp32Params = Esp32Params::new(
    0x2000,
    0x1_0000,
    0x3f_0000,
    CHIP_ID,
    FlashFrequency::_40Mhz,
    include_bytes!("../../resources/bootloaders/esp32c5-bootloader.bin"),
    None,
);

/// ESP32-C5 Target
pub struct Esp32c5;

impl Esp32c5 {
    /// Check if the magic value contains the specified value
    pub fn has_magic_value(value: u32) -> bool {
        CHIP_DETECT_MAGIC_VALUES.contains(&value)
    }
}

impl ReadEFuse for Esp32c5 {
    fn efuse_reg(&self) -> u32 {
        0x600B4800
    }

    fn block0_offset(&self) -> u32 {
        0x2C
    }

    fn block_size(&self, block: usize) -> u32 {
        efuse::BLOCK_SIZES[block]
    }
}

impl Target for Esp32c5 {
    fn chip(&self) -> Chip {
        Chip::Esp32c5
    }

    fn addr_is_flash(&self, addr: u32) -> bool {
        FLASH_RANGES.iter().any(|range| range.contains(&addr))
    }

    #[cfg(feature = "serialport")]
    fn chip_features(&self, _connection: &mut Connection) -> Result<Vec<&str>, Error> {
        Ok(vec![
            "Wi-Fi 6 (dual-band)",
            "BT 5 (LE)",
            "IEEE802.15.4",
            "Single Core + LP Core",
            "240MHz",
        ])
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
    fn crystal_freq(&self, connection: &mut Connection) -> Result<XtalFrequency, Error> {
        let uart_div = connection.read_reg(UART_CLKDIV_REG)? & UART_CLKDIV_MASK;
        let est_xtal = (connection.baud()? * uart_div) / 1_000_000 / XTAL_CLK_DIVIDER;
        let norm_xtal = if est_xtal > 45 {
            XtalFrequency::_48MHz
        } else {
            XtalFrequency::_40Mhz
        };

        Ok(norm_xtal)
    }

    fn flash_image<'a>(
        &self,
        elf_data: &'a [u8],
        flash_data: FlashData,
        _chip_revision: Option<(u32, u32)>,
        xtal_freq: XtalFrequency,
    ) -> Result<IdfBootloaderFormat<'a>, Error> {
        if xtal_freq != XtalFrequency::_40Mhz {
            return Err(Error::UnsupportedFeature {
                chip: Chip::Esp32c5,
                feature: "the selected crystal frequency".into(),
            });
        }

        IdfBootloaderFormat::new(elf_data, Chip::Esp32c5, flash_data, PARAMS)
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

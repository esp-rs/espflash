use std::{collections::HashMap, ops::Range};

use log::debug;

#[cfg(feature = "serialport")]
use crate::{connection::Connection, targets::bytes_to_mac_addr};
use crate::{
    elf::FirmwareImage,
    flasher::{FlashData, FlashFrequency},
    image_format::IdfBootloaderFormat,
    targets::{Chip, Esp32Params, ReadEFuse, SpiRegisters, Target, XtalFrequency},
    Error,
};

const CHIP_DETECT_MAGIC_VALUES: &[u32] = &[
    0x6f51_306f, // ECO0
    0x7c41_a06f, // ECO1
];

const FLASH_RANGES: &[Range<u32>] = &[
    0x4200_0000..0x4240_0000, // IROM
    0x3c00_0000..0x3c40_0000, // DROM
];

// UART0_BASE_REG + 0x14
#[cfg(feature = "serialport")]
const UART_CLKDIV_REG: u32 = 0x6000_0014;
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

impl ReadEFuse for Esp32c2 {
    fn efuse_reg(&self) -> u32 {
        0x6000_8800
    }
}

impl Target for Esp32c2 {
    fn addr_is_flash(&self, addr: u32) -> bool {
        FLASH_RANGES.iter().any(|range| range.contains(&addr))
    }

    #[cfg(feature = "serialport")]
    fn chip_features(&self, _connection: &mut Connection) -> Result<Vec<&str>, Error> {
        Ok(vec!["WiFi", "BLE"])
    }

    #[cfg(feature = "serialport")]
    fn major_chip_version(&self, connection: &mut Connection) -> Result<u32, Error> {
        Ok(self.read_efuse(connection, 17)? >> 20 & 0x3)
    }

    #[cfg(feature = "serialport")]
    fn minor_chip_version(&self, connection: &mut Connection) -> Result<u32, Error> {
        Ok(self.read_efuse(connection, 17)? >> 16 & 0xf)
    }

    #[cfg(feature = "serialport")]
    fn crystal_freq(&self, connection: &mut Connection) -> Result<XtalFrequency, Error> {
        let uart_div = connection.read_reg(UART_CLKDIV_REG)? & UART_CLKDIV_MASK;
        let est_xtal = (connection.get_baud()? * uart_div) / 1_000_000 / XTAL_CLK_DIVIDER;
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

    fn get_flash_image<'a>(
        &self,
        image: &'a dyn FirmwareImage<'a>,
        flash_data: FlashData,
        _chip_revision: Option<(u32, u32)>,
        xtal_freq: XtalFrequency,
    ) -> Result<IdfBootloaderFormat<'a>, Error> {
        let booloader: &'static [u8] = match xtal_freq {
            XtalFrequency::_40Mhz => {
                debug!("Using 40MHz bootloader");
                include_bytes!("../../resources/bootloaders/esp32c2-bootloader.bin")
            }
            XtalFrequency::_26Mhz => {
                debug!("Using 26MHz bootloader");
                include_bytes!("../../resources/bootloaders/esp32c2_26-bootloader.bin")
            }
            _ => {
                return Err(Error::UnsupportedFeature {
                    chip: Chip::Esp32c2,
                    feature: "the selected crystal frequency".into(),
                })
            }
        };

        let params = Esp32Params::new(
            0x0,
            0x1_0000,
            0x1f_0000,
            12,
            FlashFrequency::_30Mhz,
            booloader,
        );

        IdfBootloaderFormat::new(
            image,
            Chip::Esp32c2,
            flash_data.min_chip_rev,
            params,
            flash_data.partition_table,
            flash_data.partition_table_offset,
            flash_data.target_app_partition,
            flash_data.bootloader,
            flash_data.flash_settings,
        )
    }

    #[cfg(feature = "serialport")]
    /// What is the MAC address?
    fn mac_address(&self, connection: &mut Connection) -> Result<String, Error> {
        let word5 = self.read_efuse(connection, 16)?;
        let word6 = self.read_efuse(connection, 17)?;

        let bytes = ((word6 as u64) << 32) | word5 as u64;
        let bytes = bytes.to_be_bytes();
        let bytes = &bytes[2..];

        Ok(bytes_to_mac_addr(bytes))
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

use std::ops::Range;

use super::{
    Chip,
    Esp32Params,
    ReadEFuse,
    SpiRegisters,
    Target,
    XtalFrequency,
    efuse::esp32 as efuse,
};
#[cfg(feature = "serialport")]
use crate::connection::Connection;
use crate::{
    Error,
    flasher::{FlashData, FlashFrequency},
    image_format::IdfBootloaderFormat,
};

pub(crate) const CHIP_ID: u16 = 0;

const CHIP_DETECT_MAGIC_VALUES: &[u32] = &[0x00f0_1d83];

const FLASH_RANGES: &[Range<u32>] = &[
    0x400d_0000..0x4040_0000, // IROM
    0x3f40_0000..0x3f80_0000, // DROM
];

#[cfg(feature = "serialport")]
const UART_CLKDIV_REG: u32 = 0x3ff4_0014; // UART0_BASE_REG + 0x14
#[cfg(feature = "serialport")]
const UART_CLKDIV_MASK: u32 = 0xfffff;
#[cfg(feature = "serialport")]
const XTAL_CLK_DIVIDER: u32 = 1;

/// ESP32 Target
pub struct Esp32;

impl Esp32 {
    /// Check if the magic value contains the specified value
    pub fn has_magic_value(value: u32) -> bool {
        CHIP_DETECT_MAGIC_VALUES.contains(&value)
    }

    /// Return the package version based on the eFuses
    #[cfg(feature = "serialport")]
    fn package_version(&self, connection: &mut Connection) -> Result<u32, Error> {
        let word3 = self.read_efuse_raw(connection, 0, 3)?;

        let pkg_version = (word3 >> 9) & 0x7;
        let pkg_version = pkg_version + (((word3 >> 2) & 0x1) << 3);

        Ok(pkg_version)
    }
}

impl ReadEFuse for Esp32 {
    fn efuse_reg(&self) -> u32 {
        0x3FF5_A000
    }

    fn block0_offset(&self) -> u32 {
        0x0
    }

    fn block_size(&self, block: usize) -> u32 {
        efuse::BLOCK_SIZES[block]
    }
}

impl Target for Esp32 {
    fn chip(&self) -> Chip {
        Chip::Esp32
    }

    fn addr_is_flash(&self, addr: u32) -> bool {
        FLASH_RANGES.iter().any(|range| range.contains(&addr))
    }

    #[cfg(feature = "serialport")]
    fn chip_features(&self, connection: &mut Connection) -> Result<Vec<&str>, Error> {
        let word3 = self.read_efuse_raw(connection, 0, 3)?;
        let word4 = self.read_efuse_raw(connection, 0, 4)?;
        let word6 = self.read_efuse_raw(connection, 0, 6)?;

        let mut features = vec!["WiFi"];

        let chip_ver_dis_bt = word3 & 0x2;
        if chip_ver_dis_bt == 0 {
            features.push("BT");
        }

        let chip_ver_dis_app_cpu = word3 & 0x1;
        if chip_ver_dis_app_cpu == 0 {
            features.push("Dual Core");
        } else {
            features.push("Single Core");
        }

        let chip_cpu_freq_rated = word3 & (1 << 13);
        if chip_cpu_freq_rated != 0 {
            let chip_cpu_freq_low = word3 & (1 << 12);
            if chip_cpu_freq_low != 0 {
                features.push("160MHz");
            } else {
                features.push("240MHz");
            }
        }

        let pkg_version = self.package_version(connection)?;
        if [2, 4, 5, 6].contains(&pkg_version) {
            features.push("Embedded Flash");
        }
        if pkg_version == 6 {
            features.push("Embedded PSRAM");
        }

        let adc_vref = (word4 >> 8) & 0x1;
        if adc_vref != 0 {
            features.push("VRef calibration in efuse");
        }

        let blk3_part_res = (word3 >> 14) & 0x1;
        if blk3_part_res != 0 {
            features.push("BLK3 partially reserved");
        }

        let coding_scheme = word6 & 0x3;
        features.push(match coding_scheme {
            0 => "Coding Scheme None",
            1 => "Coding Scheme 3/4",
            2 => "Coding Scheme Repeat (UNSUPPORTED)",
            _ => "Coding Scheme Invalid",
        });

        Ok(features)
    }

    #[cfg(feature = "serialport")]
    fn major_chip_version(&self, connection: &mut Connection) -> Result<u32, Error> {
        let apb_ctl_date = connection.read_reg(0x3FF6_607C)?;

        let word3 = self.read_efuse_raw(connection, 0, 3)?;
        let word5 = self.read_efuse_raw(connection, 0, 5)?;

        let rev_bit0 = (word3 >> 15) & 0x1;
        let rev_bit1 = (word5 >> 20) & 0x1;
        let rev_bit2 = (apb_ctl_date >> 31) & 0x1;

        let combine_value = (rev_bit2 << 2) | (rev_bit1 << 1) | rev_bit0;

        match combine_value {
            1 => Ok(1),
            3 => Ok(2),
            7 => Ok(3),
            _ => Ok(0),
        }
    }

    #[cfg(feature = "serialport")]
    fn minor_chip_version(&self, connection: &mut Connection) -> Result<u32, Error> {
        let word5 = self.read_efuse_raw(connection, 0, 5)?;
        let minor = (word5 >> 24) & 0x3;

        Ok(minor)
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

    fn flash_image<'a>(
        &self,
        elf_data: &'a [u8],
        flash_data: FlashData,
        _chip_revision: Option<(u32, u32)>,
        xtal_freq: XtalFrequency,
    ) -> Result<IdfBootloaderFormat<'a>, Error> {
        let bootloader: &'static [u8] = match xtal_freq {
            XtalFrequency::_40Mhz => {
                include_bytes!("../../resources/bootloaders/esp32-bootloader.bin")
            }
            XtalFrequency::_26Mhz => {
                include_bytes!("../../resources/bootloaders/esp32_26-bootloader.bin")
            }
            _ => {
                return Err(Error::UnsupportedFeature {
                    chip: Chip::Esp32,
                    feature: "the selected crystal frequency".into(),
                });
            }
        };

        let params = Esp32Params::new(
            0x1000,
            0x1_0000,
            0x3f_0000,
            CHIP_ID,
            FlashFrequency::_40Mhz,
            bootloader,
            None,
        );

        IdfBootloaderFormat::new(elf_data, Chip::Esp32, flash_data, params)
    }

    fn spi_registers(&self) -> SpiRegisters {
        SpiRegisters {
            base: 0x3ff4_2000,
            usr_offset: 0x1c,
            usr1_offset: 0x20,
            usr2_offset: 0x24,
            w0_offset: 0x80,
            mosi_length_offset: Some(0x28),
            miso_length_offset: Some(0x2c),
        }
    }

    fn supported_build_targets(&self) -> &[&str] {
        &["xtensa-esp32-none-elf", "xtensa-esp32-espidf"]
    }
}

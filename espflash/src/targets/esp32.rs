#[cfg(feature = "serialport")]
use std::collections::HashMap;
use std::ops::Range;

#[cfg(feature = "serialport")]
use crate::connection::Connection;
use crate::{
    flasher::{FlashData, FlashFrequency},
    image_format::IdfBootloaderFormat,
    targets::{Chip, EfuseField, Esp32Params, ReadEFuse, SpiRegisters, Target, XtalFrequency},
    Error,
};

pub(crate) const CHIP_ID: u16 = 0;

const CHIP_DETECT_MAGIC_VALUES: &[u32] = &[0x00f0_1d83];

const FLASH_RANGES: &[Range<u32>] = &[
    0x400d_0000..0x4040_0000, // IROM
    0x3f40_0000..0x3f80_0000, // DROM
];

// UART0_BASE_REG + 0x14
#[cfg(feature = "serialport")]
const UART_CLKDIV_REG: u32 = 0x3ff4_0014;
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

    #[cfg(feature = "serialport")]
    /// Return the package version based on the eFuses
    fn package_version(&self, connection: &mut Connection) -> Result<u32, Error> {
        let fields = self.common_fields();
        let pkg_version = self.read_field(connection, fields["CHIP_VER_PKG"])?;
        let pkg_version_4bit = self.read_field(connection, fields["CHIP_VER_PKG_4BIT"])?;

        Ok(pkg_version + (pkg_version_4bit << 3))
    }
}

impl ReadEFuse for Esp32 {
    fn efuse_reg(&self) -> u32 {
        0x3ff5_a000
    }

    #[cfg(feature = "serialport")]
    fn common_fields(&self) -> HashMap<&'static str, EfuseField> {
        let mut fields = HashMap::new();

        // MAC address fields
        fields.insert(
            "MAC_FACTORY_0",
            EfuseField {
                word_offset: 1,
                bit_offset: 0,
                bit_count: 32,
            },
        );
        fields.insert(
            "MAC_FACTORY_1",
            EfuseField {
                word_offset: 2,
                bit_offset: 0,
                bit_count: 16,
            },
        );

        // Chip version fields
        fields.insert(
            "CHIP_VER_REV1",
            EfuseField {
                word_offset: 3,
                bit_offset: 15,
                bit_count: 1,
            },
        );
        fields.insert(
            "CHIP_VERSION",
            EfuseField {
                word_offset: 3,
                bit_offset: 12,
                bit_count: 2,
            },
        );
        fields.insert(
            "CHIP_VER_REV2",
            EfuseField {
                word_offset: 5,
                bit_offset: 20,
                bit_count: 1,
            },
        );
        fields.insert(
            "CHIP_CPU_FREQ_RATED",
            EfuseField {
                word_offset: 3,
                bit_offset: 13,
                bit_count: 1,
            },
        );
        fields.insert(
            "CHIP_CPU_FREQ_LOW",
            EfuseField {
                word_offset: 3,
                bit_offset: 12,
                bit_count: 1,
            },
        );
        fields.insert(
            "CHIP_VER_PKG",
            EfuseField {
                word_offset: 3,
                bit_offset: 9,
                bit_count: 3,
            },
        );
        fields.insert(
            "CHIP_VER_PKG_4BIT",
            EfuseField {
                word_offset: 3,
                bit_offset: 2,
                bit_count: 1,
            },
        );
        fields.insert(
            "CODING_SCHEME",
            EfuseField {
                word_offset: 6,
                bit_offset: 0,
                bit_count: 2,
            },
        );

        // Feature bits
        fields.insert(
            "CHIP_VER_DIS_APP_CPU",
            EfuseField {
                word_offset: 3,
                bit_offset: 0,
                bit_count: 1,
            },
        );
        fields.insert(
            "CHIP_VER_DIS_BT",
            EfuseField {
                word_offset: 3,
                bit_offset: 1,
                bit_count: 1,
            },
        );
        fields.insert(
            "ADC_VREF",
            EfuseField {
                word_offset: 4,
                bit_offset: 8,
                bit_count: 1,
            },
        );
        fields.insert(
            "BLK3_PART_RESERVE",
            EfuseField {
                word_offset: 3,
                bit_offset: 14,
                bit_count: 1,
            },
        );
        fields.insert(
            "MINOR_VERSION",
            EfuseField {
                word_offset: 5,
                bit_offset: 24,
                bit_count: 2,
            },
        );

        fields
    }
}

impl Target for Esp32 {
    fn addr_is_flash(&self, addr: u32) -> bool {
        FLASH_RANGES.iter().any(|range| range.contains(&addr))
    }

    #[cfg(feature = "serialport")]
    fn chip_features(&self, connection: &mut Connection) -> Result<Vec<&str>, Error> {
        let fields = self.common_fields();

        let mut features = vec!["WiFi"];

        let chip_ver_dis_bt = self.read_field(connection, fields["CHIP_VER_DIS_BT"])?;
        if chip_ver_dis_bt == 0 {
            features.push("BT");
        }

        let chip_ver_dis_app_cpu = self.read_field(connection, fields["CHIP_VER_DIS_APP_CPU"])?;
        if chip_ver_dis_app_cpu == 0 {
            features.push("Dual Core");
        } else {
            features.push("Single Core");
        }

        let chip_cpu_freq_rated = self.read_field(connection, fields["CHIP_CPU_FREQ_RATED"])?;
        if chip_cpu_freq_rated != 0 {
            let chip_cpu_freq_low = self.read_field(connection, fields["CHIP_CPU_FREQ_LOW"])?;
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

        let adc_vref = self.read_field(connection, fields["ADC_VREF"])?;
        if adc_vref != 0 {
            features.push("VRef calibration in efuse");
        }

        let blk3_part_res = self.read_field(connection, fields["BLK3_PART_RESERVE"])?;
        if blk3_part_res != 0 {
            features.push("BLK3 partially reserved");
        }

        let coding_scheme = self.read_field(connection, fields["CODING_SCHEME"])?;
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
        let fields = self.common_fields();

        let rev_bit0 = self.read_field(connection, fields["CHIP_VER_REV1"])?;
        let rev_bit1 = self.read_field(connection, fields["CHIP_VER_REV2"])?;
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
        let fields = self.common_fields();
        self.read_field(connection, fields["MINOR_VERSION"])
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
                })
            }
        };

        let params = Esp32Params::new(
            0x1000,
            0x1_0000,
            0x3f_0000,
            CHIP_ID,
            FlashFrequency::_40Mhz,
            bootloader,
        );

        IdfBootloaderFormat::new(elf_data, Chip::Esp32, flash_data, params)
    }

    #[cfg(feature = "serialport")]
    fn mac_address(&self, connection: &mut Connection) -> Result<String, Error> {
        let fields = self.common_fields();
        self.read_mac_address_from_words(
            connection,
            fields["MAC_FACTORY_0"],
            fields["MAC_FACTORY_1"],
        )
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

use std::ops::Range;

use crate::{
    connection::Connection,
    elf::FirmwareImage,
    error::{Error, UnsupportedImageFormatError},
    flasher::FlashData,
    image_format::{Esp8266Format, ImageFormat, ImageFormatKind},
    targets::{bytes_to_mac_addr, Chip, ReadEFuse, SpiRegisters, Target, XtalFrequency},
};

const CHIP_DETECT_MAGIC_VALUES: &[u32] = &[0xfff0_c101];

#[allow(clippy::single_range_in_vec_init)]
const FLASH_RANGES: &[Range<u32>] = &[
    0x40200000..0x40300000, // IROM
];

const UART_CLKDIV_REG: u32 = 0x6000_0014;
const UART_CLKDIV_MASK: u32 = 0xfffff;

const XTAL_CLK_DIVIDER: u32 = 2;

/// ESP8266 Target
pub struct Esp8266;

impl Esp8266 {
    /// Check if the magic value contains the specified value
    pub fn has_magic_value(value: u32) -> bool {
        CHIP_DETECT_MAGIC_VALUES.contains(&value)
    }
}

impl ReadEFuse for Esp8266 {
    fn efuse_reg(&self) -> u32 {
        0x3ff0_0050
    }
}

impl Target for Esp8266 {
    fn addr_is_flash(&self, addr: u32) -> bool {
        FLASH_RANGES.iter().any(|range| range.contains(&addr))
    }

    fn chip_features(&self, _connection: &mut Connection) -> Result<Vec<&str>, Error> {
        Ok(vec!["WiFi"])
    }

    fn major_chip_version(&self, _connection: &mut Connection) -> Result<u32, Error> {
        Err(Error::UnsupportedFeature {
            chip: Chip::Esp8266,
            feature: "reading the major chip version".into(),
        })
    }

    fn minor_chip_version(&self, _connection: &mut Connection) -> Result<u32, Error> {
        Err(Error::UnsupportedFeature {
            chip: Chip::Esp8266,
            feature: "reading the minor chip version".into(),
        })
    }

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

    fn get_flash_image<'a>(
        &self,
        image: &'a dyn FirmwareImage<'a>,
        flash_data: FlashData,
        _chip_revision: Option<(u32, u32)>,
        _xtal_freq: XtalFrequency,
    ) -> Result<Box<dyn ImageFormat<'a> + 'a>, Error> {
        let image_format = flash_data
            .image_format
            .unwrap_or(ImageFormatKind::EspBootloader);

        match image_format {
            ImageFormatKind::EspBootloader => Ok(Box::new(Esp8266Format::new(
                image,
                flash_data.flash_settings,
            )?)),
            _ => Err(UnsupportedImageFormatError::new(image_format, Chip::Esp8266, None).into()),
        }
    }

    fn mac_address(&self, connection: &mut Connection) -> Result<String, Error> {
        let word0 = self.read_efuse(connection, 0)?;
        let word1 = self.read_efuse(connection, 1)?;
        let word3 = self.read_efuse(connection, 3)?;

        // First determine the OUI portion of the MAC address
        let mut bytes = if word3 != 0 {
            vec![
                ((word3 >> 16) & 0xff) as u8,
                ((word3 >> 8) & 0xff) as u8,
                (word3 & 0xff) as u8,
            ]
        } else if ((word1 >> 16) & 0xff) == 0 {
            vec![0x18, 0xfe, 0x34]
        } else {
            vec![0xac, 0xd0, 0x74]
        };

        // Add the remaining NIC portion of the MAC address
        bytes.push(((word1 >> 8) & 0xff) as u8);
        bytes.push((word1 & 0xff) as u8);
        bytes.push(((word0 >> 24) & 0xff) as u8);

        Ok(bytes_to_mac_addr(&bytes))
    }

    fn spi_registers(&self) -> SpiRegisters {
        SpiRegisters {
            base: 0x6000_0200,
            usr_offset: 0x1c,
            usr1_offset: 0x20,
            usr2_offset: 0x24,
            w0_offset: 0x40,
            mosi_length_offset: None,
            miso_length_offset: None,
        }
    }

    fn supported_build_targets(&self) -> &[&str] {
        &["xtensa-esp8266-none-elf"]
    }
}

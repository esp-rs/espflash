use std::ops::Range;

use super::{Chip, ReadEFuse, SpiRegisters, Target, XtalFrequency, efuse::esp32s3 as efuse};
#[cfg(feature = "serialport")]
use crate::connection::Connection;
use crate::{
    Error,
    flasher::{FlashData, FlashFrequency},
    image_format::{IdfBootloaderFormat, ImageFormat, ImageFormatKind},
};

pub(crate) const CHIP_ID: u16 = 9;

const CHIP_DETECT_MAGIC_VALUES: &[u32] = &[0x9];

const FLASH_RANGES: &[Range<u32>] = &[
    0x4200_0000..0x4400_0000, // IROM
    0x3c00_0000..0x3e00_0000, // DROM
];

/// ESP32-S2 Target
pub struct Esp32s3;

impl Esp32s3 {
    #[cfg(feature = "serialport")]
    /// Return the major BLK version based on eFuses
    fn blk_version_major(&self, connection: &mut Connection) -> Result<u32, Error> {
        self.read_efuse(connection, efuse::BLK_VERSION_MAJOR)
    }

    #[cfg(feature = "serialport")]
    /// Return the minor BLK version based on eFuses
    fn blk_version_minor(&self, connection: &mut Connection) -> Result<u32, Error> {
        self.read_efuse(connection, efuse::BLK_VERSION_MINOR)
    }

    /// Check if the magic value contains the specified value
    pub fn has_magic_value(value: u32) -> bool {
        CHIP_DETECT_MAGIC_VALUES.contains(&value)
    }
}

impl ReadEFuse for Esp32s3 {
    fn efuse_reg(&self) -> u32 {
        0x6000_7000
    }

    fn block0_offset(&self) -> u32 {
        0x2D
    }

    fn block_size(&self, block: usize) -> u32 {
        efuse::BLOCK_SIZES[block]
    }
}

impl Target for Esp32s3 {
    fn chip(&self) -> Chip {
        Chip::Esp32s3
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
        // Workaround: The major version field was allocated to other purposes when
        // block version is v1.1. Luckily only chip v0.0 have this kind of block version
        // and efuse usage.
        if self.minor_chip_version(connection)? == 0
            && self.blk_version_major(connection)? == 1
            && self.blk_version_minor(connection)? == 1
        {
            Ok(0)
        } else {
            self.read_efuse(connection, efuse::WAFER_VERSION_MAJOR)
        }
    }

    #[cfg(feature = "serialport")]
    fn minor_chip_version(&self, connection: &mut Connection) -> Result<u32, Error> {
        let hi = self.read_efuse(connection, efuse::WAFER_VERSION_MINOR_HI)?;
        let lo = self.read_efuse(connection, efuse::WAFER_VERSION_MINOR_LO)?;

        Ok((hi << 3) + lo)
    }

    #[cfg(feature = "serialport")]
    fn crystal_freq(&self, _connection: &mut Connection) -> Result<XtalFrequency, Error> {
        // The ESP32-S3's XTAL has a fixed frequency of 40MHz.
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
        match format {
            ImageFormatKind::EspIdf => {
                let idf = IdfBootloaderFormat::new(
                    elf_data,
                    Chip::Esp32s3,
                    flash_data,
                    xtal_freq,
                    0x1_0000,
                    0x10_0000,
                    FlashFrequency::_40Mhz,
                )?;
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
        &["xtensa-esp32s3-none-elf", "xtensa-esp32s3-espidf"]
    }
}

#[cfg(feature = "serialport")]
impl super::RtcWdtReset for Esp32s3 {
    fn wdt_wprotect(&self) -> u32 {
        0x6000_80B0
    }

    fn wdt_config0(&self) -> u32 {
        0x6000_8098
    }

    fn wdt_config1(&self) -> u32 {
        0x6000_809C
    }

    fn can_rtc_wdt_reset(&self, connection: &mut Connection) -> Result<bool, Error> {
        const GPIO_STRAP: u32 = 0x6000_4038;
        const OPTION1: u32 = 0x6000_812C;
        const GPIO_STRAP_SPI_BOOT_MASK: u32 = 1 << 3; // Not download mode
        const FORCE_DOWNLOAD_BOOT_MASK: u32 = 0x1;

        Ok(
            connection.read_reg(GPIO_STRAP)? & GPIO_STRAP_SPI_BOOT_MASK == 0 // GPIO0 low
                && connection.read_reg(OPTION1)? & FORCE_DOWNLOAD_BOOT_MASK == 0,
        )
    }
}

#[cfg(feature = "serialport")]
impl super::UsbOtg for Esp32s3 {
    fn uartdev_buf_no(&self) -> u32 {
        0x3FCE_F14C
    }

    fn uartdev_buf_no_usb_otg(&self) -> u32 {
        3
    }
}

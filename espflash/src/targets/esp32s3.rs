#[cfg(feature = "serialport")]
use crate::{Error, connection::Connection};

pub(crate) const CHIP_ID: u16 = 9;

const CHIP_DETECT_MAGIC_VALUES: &[u32] = &[0x9];

/// ESP32-S3 Target
pub struct Esp32s3;

impl Esp32s3 {
    /// Check if the magic value contains the specified value
    pub fn has_magic_value(value: u32) -> bool {
        CHIP_DETECT_MAGIC_VALUES.contains(&value)
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

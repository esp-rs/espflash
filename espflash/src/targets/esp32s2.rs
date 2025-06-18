#[cfg(feature = "serialport")]
use crate::{Error, connection::Connection};

pub(crate) const CHIP_ID: u16 = 2;

const CHIP_DETECT_MAGIC_VALUES: &[u32] = &[0x0000_07c6];

/// ESP32-S2 Target
pub struct Esp32s2;

impl Esp32s2 {
    /// Check if the magic value contains the specified value
    pub fn has_magic_value(value: u32) -> bool {
        CHIP_DETECT_MAGIC_VALUES.contains(&value)
    }
}


#[cfg(feature = "serialport")]
impl super::RtcWdtReset for Esp32s2 {
    fn wdt_wprotect(&self) -> u32 {
        0x3F40_80AC
    }

    fn wdt_config0(&self) -> u32 {
        0x3F40_8094
    }

    fn wdt_config1(&self) -> u32 {
        0x3F40_8098
    }

    fn can_rtc_wdt_reset(&self, connection: &mut Connection) -> Result<bool, Error> {
        const GPIO_STRAP: u32 = 0x3F40_4038;
        const OPTION1: u32 = 0x3F40_8128;
        const GPIO_STRAP_SPI_BOOT_MASK: u32 = 1 << 3;
        const FORCE_DOWNLOAD_BOOT_MASK: u32 = 0x1;

        Ok(
            connection.read_reg(GPIO_STRAP)? & GPIO_STRAP_SPI_BOOT_MASK == 0 // GPIO0 low
                && connection.read_reg(OPTION1)? & FORCE_DOWNLOAD_BOOT_MASK == 0,
        )
    }
}

#[cfg(feature = "serialport")]
impl super::UsbOtg for Esp32s2 {
    fn uartdev_buf_no(&self) -> u32 {
        0x3FFF_FD14
    }

    fn uartdev_buf_no_usb_otg(&self) -> u32 {
        2
    }
}

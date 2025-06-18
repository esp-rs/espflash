#[cfg(feature = "serialport")]
use crate::{Error, connection::Connection};

pub(crate) const CHIP_ID: u16 = 18;

const CHIP_DETECT_MAGIC_VALUES: &[u32] = &[0x0, 0x0ADDBAD0];

/// ESP32-P4 Target
pub struct Esp32p4;

impl Esp32p4 {
    /// Check if the magic value contains the specified value
    pub fn has_magic_value(value: u32) -> bool {
        CHIP_DETECT_MAGIC_VALUES.contains(&value)
    }
}

#[cfg(feature = "serialport")]
impl super::RtcWdtReset for Esp32p4 {
    fn wdt_wprotect(&self) -> u32 {
        0x5011_6018
    }

    fn wdt_config0(&self) -> u32 {
        0x5011_6000
    }

    fn wdt_config1(&self) -> u32 {
        0x5011_6004
    }

    fn can_rtc_wdt_reset(&self, _connection: &mut Connection) -> Result<bool, Error> {
        Ok(true)
    }
}

#[cfg(feature = "serialport")]
impl super::UsbOtg for Esp32p4 {
    fn uartdev_buf_no(&self) -> u32 {
        0x4FF3_FEC8
    }

    fn uartdev_buf_no_usb_otg(&self) -> u32 {
        5
    }
}

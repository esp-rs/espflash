pub(crate) const CHIP_ID: u16 = 5;

const CHIP_DETECT_MAGIC_VALUES: &[u32] = &[
    0x6921_506f, // ECO1 + ECO2
    0x1b31_506f, // ECO3
    0x4881_606F, // ECO6
    0x4361_606f, // ECO7
];

/// ESP32-C3 Target
pub struct Esp32c3;

impl Esp32c3 {
    /// Check if the magic value contains the specified value
    pub fn has_magic_value(value: u32) -> bool {
        CHIP_DETECT_MAGIC_VALUES.contains(&value)
    }
}

#[cfg(feature = "serialport")]
impl super::RtcWdtReset for Esp32c3 {
    fn wdt_wprotect(&self) -> u32 {
        0x6000_80A8
    }

    fn wdt_config0(&self) -> u32 {
        0x6000_8090
    }

    fn wdt_config1(&self) -> u32 {
        0x6000_8094
    }

    fn can_rtc_wdt_reset(&self, _connection: &mut crate::connection::Connection) -> Result<bool, crate::error::Error> {
        Ok(true)
    }
}

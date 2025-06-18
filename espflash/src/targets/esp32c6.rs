pub(crate) const CHIP_ID: u16 = 13;

const CHIP_DETECT_MAGIC_VALUES: &[u32] = &[0x2CE0_806F];

/// ESP32-C6 Target
pub struct Esp32c6;

impl Esp32c6 {
    /// Check if the magic value contains the specified value
    pub fn has_magic_value(value: u32) -> bool {
        CHIP_DETECT_MAGIC_VALUES.contains(&value)
    }
}

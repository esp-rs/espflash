pub(crate) const CHIP_ID: u16 = 16;

const CHIP_DETECT_MAGIC_VALUES: &[u32] = &[0xD7B7_3E80];

/// ESP32-H2 Target
pub struct Esp32h2;

impl Esp32h2 {
    /// Check if the magic value contains the specified value.
    pub fn has_magic_value(value: u32) -> bool {
        CHIP_DETECT_MAGIC_VALUES.contains(&value)
    }
}

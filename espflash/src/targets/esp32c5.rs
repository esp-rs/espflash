pub(crate) const CHIP_ID: u16 = 23;

const CHIP_DETECT_MAGIC_VALUES: &[u32] = &[];

/// ESP32-C5 Target
pub struct Esp32c5;

impl Esp32c5 {
    /// Check if the magic value contains the specified value
    pub fn has_magic_value(value: u32) -> bool {
        CHIP_DETECT_MAGIC_VALUES.contains(&value)
    }
}



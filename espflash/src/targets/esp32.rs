pub(crate) const CHIP_ID: u16 = 0;

const CHIP_DETECT_MAGIC_VALUES: &[u32] = &[0x00f0_1d83];

/// ESP32 Target
pub struct Esp32;

impl Esp32 {
    /// Check if the magic value contains the specified value
    pub fn has_magic_value(value: u32) -> bool {
        CHIP_DETECT_MAGIC_VALUES.contains(&value)
    }
}

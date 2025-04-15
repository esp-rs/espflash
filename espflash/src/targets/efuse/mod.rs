pub(crate) mod esp32;
pub(crate) mod esp32c2;
pub(crate) mod esp32c3;
pub(crate) mod esp32c6;
pub(crate) mod esp32h2;
pub(crate) mod esp32p4;
pub(crate) mod esp32s2;
pub(crate) mod esp32s3;

#[derive(Debug, Clone, serde::Deserialize)]
pub struct EfuseField {
    pub(crate) block: u32,
    pub(crate) word: u32,
    pub(crate) bit_start: u32,
    pub(crate) bit_count: u32,
}

impl EfuseField {
    const fn new(block: u32, word: u32, bit_start: u32, bit_count: u32) -> Self {
        Self {
            block,
            word,
            bit_start,
            bit_count,
        }
    }
}

#![allow(clippy::empty_docs)]

pub mod esp32;
pub mod esp32c2;
pub mod esp32c3;
pub mod esp32c5;
pub mod esp32c6;
pub mod esp32h2;
pub mod esp32p4;
pub mod esp32s2;
pub mod esp32s3;

#[allow(unused)]
#[derive(Debug, Clone, serde::Deserialize)]
pub struct EfuseField {
    pub block: u32,
    pub word: u32,
    pub bit_start: u32,
    pub bit_count: u32,
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

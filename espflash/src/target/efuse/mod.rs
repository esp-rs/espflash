//! eFuse field definitions for all target devices.
//!
//! Fields can be read from a connected device, writing is not currently
//! supported.

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

/// An eFuse field which can be read from a target device.
#[allow(unused)]
#[derive(Debug, Clone, serde::Deserialize)]
pub struct EfuseField {
    /// The block in which the field is located.
    pub block: u32,
    /// The word offset of the field.
    pub word: u32,
    /// The bit offset of the start of the field.
    pub bit_start: u32,
    /// The bit width of the field.
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

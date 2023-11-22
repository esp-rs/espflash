use base64::{engine::general_purpose, Engine as _};
use serde::{Deserialize, Serialize};

use crate::targets::Chip;

/// Flash stub object (deserialized from TOML, converted from JSON as used by `esptool.py`)
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct FlashStub {
    /// Entry point (address)
    entry: u32,
    /// Text (base64 encoded)
    text: String,
    /// Start of text section address
    text_start: u32,
    /// Data
    data: String,
    /// Start of data section address
    data_start: u32,
}

// Include stub objects in binary
const STUB_32: &str = include_str!("../../resources/stubs/stub_flasher_32.toml");
const STUB_32C2: &str = include_str!("../../resources/stubs/stub_flasher_32c2.toml");
const STUB_32C3: &str = include_str!("../../resources/stubs/stub_flasher_32c3.toml");
const STUB_32C6: &str = include_str!("../../resources/stubs/stub_flasher_32c6.toml");
const STUB_32S2: &str = include_str!("../../resources/stubs/stub_flasher_32s2.toml");
const STUB_32S3: &str = include_str!("../../resources/stubs/stub_flasher_32s3.toml");
const STUB_32H2: &str = include_str!("../../resources/stubs/stub_flasher_32h2.toml");
const STUB_8266: &str = include_str!("../../resources/stubs/stub_flasher_8266.toml");

impl FlashStub {
    /// Fetch flash stub for the provided chip
    pub fn get(chip: Chip) -> FlashStub {
        let s = match chip {
            Chip::Esp32 => STUB_32,
            Chip::Esp32c2 => STUB_32C2,
            Chip::Esp32c3 => STUB_32C3,
            Chip::Esp32c6 => STUB_32C6,
            Chip::Esp32s2 => STUB_32S2,
            Chip::Esp32s3 => STUB_32S3,
            Chip::Esp32h2 => STUB_32H2,
            Chip::Esp8266 => STUB_8266,
        };

        let stub: FlashStub = toml::from_str(s).unwrap();

        stub
    }

    /// Fetch stub entry point
    pub fn entry(&self) -> u32 {
        self.entry
    }

    /// Fetch text start address and bytes
    pub fn text(&self) -> (u32, Vec<u8>) {
        let v = general_purpose::STANDARD.decode(&self.text).unwrap();
        (self.text_start, v)
    }

    /// Fetch data start address and bytes
    pub fn data(&self) -> (u32, Vec<u8>) {
        let v = general_purpose::STANDARD.decode(&self.data).unwrap();
        (self.data_start, v)
    }
}

#[cfg(test)]
mod tests {
    use strum::IntoEnumIterator;

    use super::FlashStub;
    use crate::targets::Chip;

    #[test]
    fn check_stub_encodings() {
        for c in Chip::iter() {
            // Stub must be valid json
            let s = FlashStub::get(c);

            // Data decoded from b64
            let _ = s.text();
            let _ = s.data();
        }
    }
}

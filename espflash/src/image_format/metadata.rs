use std::{collections::HashMap, error::Error};

use object::{File, Object, ObjectSection, ObjectSymbol};

/// Image format metadata.
#[derive(Debug, Clone)]
pub struct Metadata {
    symbols: HashMap<String, Vec<u8>>,
}

impl Metadata {
    /// Creates an empty [`Metadata`].
    fn empty() -> Self {
        Self {
            symbols: HashMap::new(),
        }
    }

    /// Creates a new [`Metadata`] from bytes.
    pub fn from_bytes(bytes: Option<&[u8]>) -> Self {
        match Self::try_from(bytes) {
            Ok(metadata) => metadata,
            Err(_) => Self::empty(),
        }
    }

    /// Tries to create a new [`Metadata`] from bytes.
    fn try_from(bytes: Option<&[u8]>) -> Result<Self, Box<dyn Error>> {
        const METADATA_SECTION: &str = ".espressif.metadata";

        let Some(bytes) = bytes else {
            return Ok(Self::empty());
        };

        let object = File::parse(bytes)?;
        if object.section_by_name(METADATA_SECTION).is_none() {
            return Ok(Self::empty());
        }

        let mut this = Self::empty();
        for symbol in object.symbols() {
            let Some(sym_section_idx) = symbol.section_index() else {
                continue;
            };
            let sym_section = object.section_by_index(sym_section_idx)?;
            if sym_section.name().ok() != Some(METADATA_SECTION) {
                // Skip symbols that are not in the metadata section.
                continue;
            }

            let name = symbol.name()?.to_string();
            let data = sym_section
                .data_range(symbol.address(), symbol.size())?
                .map(|b| b.to_vec());

            if let Some(data) = data {
                this.symbols.insert(name, data);
            }
        }

        Ok(this)
    }

    /// Reads a string from the metadata.
    fn read_string<'f>(&'f self, name: &str) -> Option<&'f str> {
        self.symbols
            .get(name)
            .and_then(|data| std::str::from_utf8(data).ok())
    }

    /// Returns the chip name.
    pub fn chip_name(&self) -> Option<&str> {
        self.read_string("build_info.CHIP_NAME")
    }

    /// Returns the log format.
    pub fn log_format(&self) -> Option<&str> {
        self.read_string("espflash.LOG_FORMAT")
    }
}

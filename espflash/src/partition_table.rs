use md5::{Context, Digest};
use regex::Regex;
use serde::{Deserialize, Deserializer};

use crate::error::PartitionTableError;
use std::io::Write;

const MAX_PARTITION_LENGTH: usize = 0xC00;
const PARTITION_TABLE_SIZE: usize = 0x1000;
const MAX_PARTITION_TABLE_ENTRIES: usize = 95;

#[derive(Copy, Clone, Debug, Deserialize)]
#[repr(u8)]
#[allow(dead_code)]
pub enum Type {
    #[serde(alias = "app")]
    App = 0x00,
    #[serde(alias = "data")]
    Data = 0x01,
}

#[derive(Copy, Clone, Debug, Deserialize)]
#[repr(u8)]
#[allow(dead_code)]
pub enum AppType {
    #[serde(alias = "factory")]
    Factory = 0x00,
    #[serde(alias = "ota_0")]
    Ota0 = 0x10,
    #[serde(alias = "ota_1")]
    Ota1 = 0x11,
    #[serde(alias = "ota_2")]
    Ota2 = 0x12,
    #[serde(alias = "ota_3")]
    Ota3 = 0x13,
    #[serde(alias = "ota_4")]
    Ota4 = 0x14,
    #[serde(alias = "ota_5")]
    Ota5 = 0x15,
    #[serde(alias = "ota_6")]
    Ota6 = 0x16,
    #[serde(alias = "ota_7")]
    Ota7 = 0x17,
    #[serde(alias = "ota_8")]
    Ota8 = 0x18,
    #[serde(alias = "ota_9")]
    Ota9 = 0x19,
    #[serde(alias = "ota_10")]
    Ota10 = 0x1a,
    #[serde(alias = "ota_11")]
    Ota11 = 0x1b,
    #[serde(alias = "ota_12")]
    Ota12 = 0x1c,
    #[serde(alias = "ota_13")]
    Ota13 = 0x1d,
    #[serde(alias = "ota_14")]
    Ota14 = 0x1e,
    #[serde(alias = "ota_15")]
    Ota15 = 0x1f,
    #[serde(alias = "test")]
    Test = 0x20,
}

#[derive(Copy, Clone, Debug, Deserialize)]
#[repr(u8)]
#[allow(dead_code)]
pub enum DataType {
    #[serde(alias = "ota")]
    Ota = 0x00,
    #[serde(alias = "phy")]
    Phy = 0x01,
    #[serde(alias = "nvs")]
    Nvs = 0x02,
    #[serde(alias = "coredump")]
    CoreDump = 0x03,
    #[serde(alias = "nvs_keys")]
    NvsKeys = 0x04,
    #[serde(alias = "efuse")]
    EFuse = 0x05,
    #[serde(alias = "undefined")]
    Undefined = 0x06,
    #[serde(alias = "esphttpd")]
    EspHttpd = 0x80,
    #[serde(alias = "fat")]
    Fat = 0x81,
    #[serde(alias = "spiffs")]
    Spiffs = 0x82,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
#[serde(untagged)]
pub enum SubType {
    App(AppType),
    Data(DataType),
}

impl SubType {
    fn as_u8(&self) -> u8 {
        match self {
            SubType::App(ty) => *ty as u8,
            SubType::Data(ty) => *ty as u8,
        }
    }
}

#[derive(Debug)]
pub struct PartitionTable {
    partitions: Vec<Partition>,
}

impl PartitionTable {
    /// Create a basic partition table with NVS, PHY init data, and the app
    /// partition
    pub fn basic(
        nvs_offset: u32,
        nvs_size: u32,
        phy_init_data_offset: u32,
        phy_init_data_size: u32,
        app_offset: u32,
        app_size: u32,
    ) -> Self {
        PartitionTable {
            partitions: vec![
                Partition::new(
                    String::from("nvs"),
                    SubType::Data(DataType::Nvs),
                    nvs_offset,
                    nvs_size,
                    None,
                ),
                Partition::new(
                    String::from("phy_init"),
                    SubType::Data(DataType::Phy),
                    phy_init_data_offset,
                    phy_init_data_size,
                    None,
                ),
                Partition::new(
                    String::from("factory"),
                    SubType::App(AppType::Factory),
                    app_offset,
                    app_size,
                    None,
                ),
            ],
        }
    }

    /// Attempt to parse a partition table from the given string. For more
    /// information on the partition table CSV format see:
    /// https://docs.espressif.com/projects/esp-idf/en/latest/esp32/api-guides/partition-tables.html
    pub fn try_from_str<S: Into<String>>(data: S) -> Result<Self, PartitionTableError> {
        let data = data.into();
        let mut reader = csv::ReaderBuilder::new()
            .comment(Some(b'#'))
            .has_headers(false)
            .trim(csv::Trim::All)
            .from_reader(data.trim().as_bytes());

        let mut partitions = Vec::with_capacity(MAX_PARTITION_TABLE_ENTRIES);
        for partition in reader.deserialize() {
            let partition: Partition =
                partition.map_err(|e| PartitionTableError::new(e, data.clone()))?;
            partitions.push(partition);
        }

        Ok(Self { partitions })
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut result = Vec::with_capacity(PARTITION_TABLE_SIZE);
        self.save(&mut result).unwrap();
        result
    }

    pub fn save<W: Write>(&self, writer: &mut W) -> std::io::Result<()> {
        let mut hasher = HashWriter::new(writer);
        for partition in &self.partitions {
            partition.save(&mut hasher)?;
        }

        let (writer, hash) = hasher.compute();

        writer.write_all(&[
            0xEB, 0xEB, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
            0xFF, 0xFF,
        ])?;
        writer.write_all(&hash.0)?;

        let written = self.partitions.len() * PARTITION_SIZE + 32;
        for _ in 0..(MAX_PARTITION_LENGTH - written) {
            writer.write_all(&[0xFF])?;
        }

        Ok(())
    }
}

const PARTITION_SIZE: usize = 32;

#[derive(Debug, Deserialize)]
struct Partition {
    #[serde(deserialize_with = "deserialize_partition_name")]
    name: String,
    ty: Type,
    sub_type: SubType,
    #[serde(deserialize_with = "deserialize_partition_offset_or_size")]
    offset: u32,
    #[serde(deserialize_with = "deserialize_partition_offset_or_size")]
    size: u32,
    flags: Option<u32>,
}

impl Partition {
    pub fn new(
        name: String,
        sub_type: SubType,
        offset: u32,
        size: u32,
        flags: Option<u32>,
    ) -> Self {
        Partition {
            name,
            ty: match sub_type {
                SubType::App(_) => Type::App,
                SubType::Data(_) => Type::Data,
            },
            sub_type,
            offset,
            size,
            flags,
        }
    }

    pub fn save<W: Write>(&self, writer: &mut W) -> std::io::Result<()> {
        writer.write_all(&[0xAA, 0x50])?;
        writer.write_all(&[self.ty as u8, self.sub_type.as_u8()])?;
        writer.write_all(&self.offset.to_le_bytes())?;
        writer.write_all(&self.size.to_le_bytes())?;

        let mut name_bytes = [0u8; 16];
        for (source, dest) in self.name.bytes().take(16).zip(name_bytes.iter_mut()) {
            *dest = source;
        }
        writer.write_all(&name_bytes)?;

        let flags = match &self.flags {
            Some(f) => f.to_le_bytes(),
            None => 0u32.to_le_bytes(),
        };
        writer.write_all(&flags)?;

        Ok(())
    }
}

fn deserialize_partition_name<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    // Partition names longer than 16 characters are truncated.
    // https://docs.espressif.com/projects/esp-idf/en/latest/esp32/api-guides/partition-tables.html#name-field
    const MAX_LENGTH: usize = 16;

    let buf = String::deserialize(deserializer)?;
    let maybe_truncated = match buf.as_str().char_indices().nth(MAX_LENGTH) {
        Some((idx, _)) => String::from(&buf[..idx]),
        None => buf,
    };

    Ok(maybe_truncated)
}

fn deserialize_partition_offset_or_size<'de, D>(deserializer: D) -> Result<u32, D::Error>
where
    D: Deserializer<'de>,
{
    use serde::de::Error;

    let buf = String::deserialize(deserializer)?;
    let re = Regex::new(r"(?i)^(\d+)([km]{1})$").unwrap();

    // NOTE: Partitions of type 'app' must be placed at offsets aligned to 0x10000
    //       (64K).
    // TODO: The specification states that offsets may be left blank, however that
    //       is not presently supported in this implementation.
    if buf.starts_with("0x") {
        // Hexadecimal format
        let src = buf.trim_start_matches("0x");
        let size = u32::from_str_radix(src, 16).unwrap();

        Ok(size)
    } else if let Ok(size) = buf.parse::<u32>() {
        // Decimal format
        Ok(size)
    } else if let Some(captures) = re.captures(&buf) {
        // Size multiplier format (1k, 2M, etc.)
        let digits = captures.get(1).unwrap().as_str().parse::<u32>().unwrap();
        let multiplier = match captures.get(2).unwrap().as_str() {
            "k" | "K" => 1024,
            "m" | "M" => 1024 * 1024,
            _ => unreachable!(),
        };

        Ok(digits * multiplier)
    } else {
        Err(Error::custom("invalid partition size/offset format"))
    }
}

struct HashWriter<W: Write> {
    inner: W,
    hasher: Context,
}

impl<W: Write> Write for HashWriter<W> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.hasher.write_all(buf)?;
        self.inner.write(buf)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.inner.flush()
    }
}

impl<W: Write> HashWriter<W> {
    pub fn new(inner: W) -> Self {
        HashWriter {
            inner,
            hasher: Context::new(),
        }
    }

    pub fn compute(self) -> (W, Digest) {
        (self.inner, self.hasher.compute())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PTABLE_0: &str = "
# ESP-IDF Partition Table
# Name,   Type, SubType, Offset,  Size, Flags
nvs,      data, nvs,     0x9000,  0x6000,
phy_init, data, phy,     0xf000,  0x1000,
factory,  app,  factory, 0x10000, 1M,
";

    const PTABLE_1: &str = "
# ESP-IDF Partition Table
# Name,   Type, SubType, Offset,  Size, Flags
nvs,      data, nvs,     0x9000,  0x4000,
otadata,  data, ota,     0xd000,  0x2000,
phy_init, data, phy,     0xf000,  0x1000,
factory,  app,  factory, 0x10000,  1M,
ota_0,    app,  ota_0,   0x110000, 1M,
ota_1,    app,  ota_1,   0x210000, 1M,
";

    #[test]
    fn test_basic() {
        use std::fs::read;
        const NVS_ADDR: u32 = 0x9000;
        const PHY_INIT_DATA_ADDR: u32 = 0xf000;
        const APP_ADDR: u32 = 0x10000;

        const NVS_SIZE: u32 = 0x6000;
        const PHY_INIT_DATA_SIZE: u32 = 0x1000;
        const APP_SIZE: u32 = 0x3f0000;

        let expected = read("./tests/data/partitions.bin").unwrap();
        let table = PartitionTable::basic(
            NVS_ADDR,
            NVS_SIZE,
            PHY_INIT_DATA_ADDR,
            PHY_INIT_DATA_SIZE,
            APP_ADDR,
            APP_SIZE,
        );

        let result = table.to_bytes();

        assert_eq!(expected.len(), result.len());
        assert_eq!(expected, result.as_slice());
    }

    #[test]
    fn test_from_str() {
        let pt0 = PartitionTable::try_from_str(PTABLE_0);
        assert!(pt0.is_ok());

        let pt1 = PartitionTable::try_from_str(PTABLE_1);
        assert!(pt1.is_ok());
    }
}

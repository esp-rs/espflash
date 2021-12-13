use std::{
    cmp::{max, min},
    fmt::{Display, Formatter, Write as _},
    io::Write,
    ops::Rem,
};

use md5::{Context, Digest};
use regex::Regex;
use serde::{Deserialize, Deserializer, Serialize};

use crate::error::{
    CSVError, DuplicatePartitionsError, InvalidSubTypeError, NoFactoryAppError,
    OverlappingPartitionsError, PartitionTableError, UnalignedPartitionError,
};

const MAX_PARTITION_LENGTH: usize = 0xC00;
const PARTITION_TABLE_SIZE: usize = 0x1000;

#[derive(Copy, Clone, Debug, Deserialize, Serialize, PartialEq)]
#[repr(u8)]
#[allow(dead_code)]
#[serde(rename_all = "lowercase")]
pub enum Type {
    App = 0x00,
    Data = 0x01,
}

impl Type {
    pub fn subtype_hint(&self) -> String {
        match self {
            Type::App => "'factory', 'ota_0' through 'ota_15' and 'test'".into(),
            Type::Data => {
                use DataType::*;
                let types = [
                    Ota, Phy, Nvs, CoreDump, NvsKeys, EFuse, EspHttpd, Fat, Spiffs,
                ];

                let mut out = format!("'{}'", serde_plain::to_string(&types[0]).unwrap());
                for ty in &types[1..types.len() - 2] {
                    let ser = serde_plain::to_string(&ty).unwrap();
                    write!(&mut out, ", '{}'", ser).unwrap();
                }

                let ser = serde_plain::to_string(&types[types.len() - 1]).unwrap();
                write!(&mut out, " and '{}'", ser).unwrap();

                out
            }
        }
    }
}

impl Display for Type {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", serde_plain::to_string(self).unwrap())
    }
}

#[derive(Copy, Clone, Debug, Deserialize, Serialize, PartialEq)]
#[repr(u8)]
#[allow(dead_code)]
#[serde(rename_all = "lowercase")]
pub enum AppType {
    Factory = 0x00,
    #[serde(rename = "ota_0")]
    Ota0 = 0x10,
    #[serde(rename = "ota_1")]
    Ota1 = 0x11,
    #[serde(rename = "ota_2")]
    Ota2 = 0x12,
    #[serde(rename = "ota_3")]
    Ota3 = 0x13,
    #[serde(rename = "ota_4")]
    Ota4 = 0x14,
    #[serde(rename = "ota_5")]
    Ota5 = 0x15,
    #[serde(rename = "ota_6")]
    Ota6 = 0x16,
    #[serde(rename = "ota_7")]
    Ota7 = 0x17,
    #[serde(rename = "ota_8")]
    Ota8 = 0x18,
    #[serde(rename = "ota_9")]
    Ota9 = 0x19,
    #[serde(rename = "ota_10")]
    Ota10 = 0x1a,
    #[serde(rename = "ota_11")]
    Ota11 = 0x1b,
    #[serde(rename = "ota_12")]
    Ota12 = 0x1c,
    #[serde(rename = "ota_13")]
    Ota13 = 0x1d,
    #[serde(rename = "ota_14")]
    Ota14 = 0x1e,
    #[serde(rename = "ota_15")]
    Ota15 = 0x1f,
    Test = 0x20,
}

#[derive(Copy, Clone, Debug, Deserialize, Serialize, PartialEq)]
#[repr(u8)]
#[allow(dead_code)]
#[serde(rename_all = "lowercase")]
pub enum DataType {
    Ota = 0x00,
    Phy = 0x01,
    Nvs = 0x02,
    CoreDump = 0x03,
    NvsKeys = 0x04,
    EFuse = 0x05,
    Undefined = 0x06,
    EspHttpd = 0x80,
    Fat = 0x81,
    Spiffs = 0x82,
}

#[derive(Debug, Deserialize, PartialEq, Copy, Clone)]
#[allow(dead_code)]
#[serde(untagged)]
pub enum SubType {
    App(AppType),
    Data(DataType),
}

impl Display for SubType {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let ser = match self {
            SubType::App(sub) => serde_plain::to_string(sub),
            SubType::Data(sub) => serde_plain::to_string(sub),
        }
        .unwrap();
        write!(f, "{}", ser)
    }
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

        // Default offset is 0x8000 in esp-idf, partition table size is 0x1000
        let mut offset = 0x9000; 
        let mut partitions = Vec::with_capacity(data.lines().count());
        for record in reader.records() {
            let record = record.map_err(|e| CSVError::new(e, data.clone()))?;
            let position = record.position();
            let mut partition: DeserializedPartition = record
                .deserialize(None)
                .map_err(|e| CSVError::new(e, data.clone()))?;

            partition.fixup_offset(&mut offset);

            let mut partition = Partition::from(partition);
            partition.line = position.map(|pos| pos.line() as usize);
            partitions.push(partition);
        }

        let table = Self { partitions };
        table.validate(&data)?;
        Ok(table)
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

    pub fn find(&self, name: &str) -> Option<&Partition> {
        self.partitions.iter().find(|&p| p.name == name)
    }

    fn validate(&self, source: &str) -> Result<(), PartitionTableError> {
        for partition in &self.partitions {
            if let Some(line) = &partition.line {
                let expected_type = match partition.sub_type {
                    SubType::App(_) => Type::App,
                    SubType::Data(_) => Type::Data,
                };

                if expected_type != partition.ty {
                    return Err(InvalidSubTypeError::new(
                        source,
                        *line,
                        partition.ty,
                        partition.sub_type,
                    )
                    .into());
                }

                if partition.ty == Type::App && partition.offset.rem(0x10000) != 0 {
                    return Err(UnalignedPartitionError::new(source, *line).into());
                }
            }
        }

        for partition1 in &self.partitions {
            for partition2 in &self.partitions {
                if let (Some(line1), Some(line2)) = (&partition1.line, &partition2.line) {
                    if line1 != line2 {
                        if partition1.overlaps(partition2) {
                            return Err(
                                OverlappingPartitionsError::new(source, *line1, *line2).into()
                            );
                        }

                        if partition1.name == partition2.name {
                            return Err(DuplicatePartitionsError::new(
                                source, *line1, *line2, "name",
                            )
                            .into());
                        }

                        if partition1.sub_type == partition2.sub_type {
                            return Err(DuplicatePartitionsError::new(
                                source, *line1, *line2, "sub-type",
                            )
                            .into());
                        }
                    }
                }
            }
        }

        if self.find("factory").is_none() {
            return Err(PartitionTableError::NoFactoryApp(NoFactoryAppError::new(
                source,
            )));
        }

        Ok(())
    }
}

const PARTITION_SIZE: usize = 32;

#[derive(Debug, Deserialize)]
pub struct DeserializedPartition {
    #[serde(deserialize_with = "deserialize_partition_name")]
    name: String,
    ty: Type,
    sub_type: SubType,
    #[serde(deserialize_with = "deserialize_partition_offset")]
    offset: Option<u32>,
    #[serde(deserialize_with = "deserialize_partition_size")]
    size: u32,
    flags: Option<u32>,
}

impl DeserializedPartition {
    fn align(offset: u32, ty: Type) -> u32 {
        let pad = match ty {
            Type::App => 0x10000,
            Type::Data => 4,
        };

        if offset % pad != 0 {
            offset + pad - (offset % pad)
        } else {
            offset
        }
    }

    fn fixup_offset(&mut self, offset: &mut u32) {
        if self.offset.is_none() {
            self.offset = Some(Self::align(*offset, self.ty));
        }

        *offset = self.offset.unwrap() + self.size;
    }
}

impl From<DeserializedPartition> for Partition {
    fn from(part: DeserializedPartition) -> Self {
        Partition {
            name: part.name,
            ty: part.ty,
            sub_type: part.sub_type,
            offset: part.offset.unwrap(),
            size: part.size,
            flags: part.flags,
            line: None,
        }
    }
}

#[derive(Debug)]
pub struct Partition {
    name: String,
    ty: Type,
    sub_type: SubType,
    offset: u32,
    size: u32,
    flags: Option<u32>,
    line: Option<usize>,
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
            line: None,
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

    pub fn offset(&self) -> u32 {
        self.offset
    }

    fn overlaps(&self, other: &Partition) -> bool {
        max(self.offset, other.offset) < min(self.offset + self.size, other.offset + other.size)
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

fn deserialize_partition_offset_or_size<'de, D>(deserializer: D) -> Result<Option<u32>, D::Error>
where
    D: Deserializer<'de>,
{
    use serde::de::Error;

    let buf = String::deserialize(deserializer)?;
    let re = Regex::new(r"(?i)^(\d+)([km]{1})$").unwrap();

    // NOTE: Partitions of type 'app' must be placed at offsets aligned to 0x10000
    //       (64K).
    if buf.trim().is_empty() {
        Ok(None)
    } else if buf.starts_with("0x") {
        // Hexadecimal format
        let src = buf.trim_start_matches("0x");
        let size = u32::from_str_radix(src, 16).unwrap();

        Ok(Some(size))
    } else if let Ok(size) = buf.parse::<u32>() {
        // Decimal format
        Ok(Some(size))
    } else if let Some(captures) = re.captures(&buf) {
        // Size multiplier format (1k, 2M, etc.)
        let digits = captures.get(1).unwrap().as_str().parse::<u32>().unwrap();
        let multiplier = match captures.get(2).unwrap().as_str() {
            "k" | "K" => 1024,
            "m" | "M" => 1024 * 1024,
            _ => unreachable!(),
        };

        Ok(Some(digits * multiplier))
    } else {
        Err(Error::custom("invalid partition size/offset format"))
    }
}

fn deserialize_partition_offset<'de, D>(deserializer: D) -> Result<Option<u32>, D::Error>
where
    D: Deserializer<'de>,
{
    deserialize_partition_offset_or_size(deserializer)
}

fn deserialize_partition_size<'de, D>(deserializer: D) -> Result<u32, D::Error>
where
    D: Deserializer<'de>,
{
    use serde::de::Error;
    let deserialized = deserialize_partition_offset_or_size(deserializer)?;
    deserialized.ok_or_else(|| Error::custom("invalid partition size/offset format"))
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

const PTABLE_2: &str = "
# ESP-IDF Partition Table
# Name,   Type, SubType, Offset,  Size, Flags
nvs,      data, nvs,           ,  0x4000,
phy_init, data, phy,           ,  0x1000,
factory,  app,  factory,       ,  1M,
";

const PTABLE_3: &str = "
# ESP-IDF Partition Table
# Name,   Type, SubType, Offset,  Size, Flags
nvs,      data, nvs,    0x10000,  0x4000,
phy_init, data, phy,           ,  0x1000,
factory,  app,  factory,       ,  1M,
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

    #[test]
    fn blank_offsets_are_filled_in() {
        let pt2 = PartitionTable::try_from_str(PTABLE_2).expect("Failed to parse partition table with blank offsets");
        
        assert_eq!(3, pt2.partitions.len());
        assert_eq!(0x4000, pt2.partitions[0].size);
        assert_eq!(0x1000, pt2.partitions[1].size);
        assert_eq!(0x100000, pt2.partitions[2].size);

        assert_eq!(0x9000, pt2.partitions[0].offset);
        assert_eq!(0xd000, pt2.partitions[1].offset);
        assert_eq!(0x10000, pt2.partitions[2].offset);
    }

    #[test]
    fn first_offsets_are_respected() {
        let pt3 = PartitionTable::try_from_str(PTABLE_3).expect("Failed to parse partition table with blank offsets");
        
        assert_eq!(3, pt3.partitions.len());
        assert_eq!(0x4000, pt3.partitions[0].size);
        assert_eq!(0x1000, pt3.partitions[1].size);
        assert_eq!(0x100000, pt3.partitions[2].size);

        assert_eq!(0x10000, pt3.partitions[0].offset);
        assert_eq!(0x14000, pt3.partitions[1].offset);
        assert_eq!(0x20000, pt3.partitions[2].offset);
    }
}

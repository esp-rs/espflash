use std::{
    cmp::{max, min},
    fmt::{Display, Formatter, Write as _},
    io::{Cursor, Write},
    ops::Rem,
};

use binread::{BinRead, BinReaderExt};
use comfy_table::{modifiers, presets::UTF8_FULL, Attribute, Cell, Color, Table};
use md5::{Context, Digest};
use regex::Regex;
use serde::{Deserialize, Deserializer, Serialize};
use strum::IntoEnumIterator;
use strum_macros::EnumIter;

use crate::error::{
    CSVError, DuplicatePartitionsError, InvalidChecksum, InvalidPartitionTable,
    InvalidSubTypeError, LengthNotMultipleOf32, NoAppError, NoEndMarker,
    OverlappingPartitionsError, PartitionTableError, UnalignedPartitionError,
};

const MAX_PARTITION_LENGTH: usize = 0xC00;
const PARTITION_TABLE_SIZE: usize = 0x1000;
const PARTITION_SIZE: usize = 32;
const PARTITION_ALIGNMENT: u32 = 0x10000;
const MAGIC_BYTES: &[u8] = &[0xAA, 0x50];
const MD5_PART_MAGIC_BYTES: &[u8] = &[
    0xEB, 0xEB, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
];
const END_MARKER: [u8; 32] = [0xFF; 32];

#[derive(Debug, Clone, Copy, PartialEq, Eq, BinRead, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
#[br(little, repr = u8)]
pub enum CoreType {
    App = 0x00,
    Data = 0x01,
}

impl CoreType {
    pub fn subtype_hint(&self) -> String {
        match self {
            CoreType::App => "'factory', 'ota_0' through 'ota_15', and 'test'".into(),
            CoreType::Data => {
                let types = DataType::iter()
                    .map(|dt| format!("'{}'", serde_plain::to_string(&dt).unwrap()))
                    .collect::<Vec<_>>();

                let mut out = types[0..types.len() - 2].join(", ");
                write!(&mut out, ", and {}", types[types.len() - 1]).unwrap();

                out
            }
        }
    }
}

impl Display for CoreType {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        write!(f, "{}", serde_plain::to_string(self).unwrap())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, BinRead, Deserialize, Serialize)]
#[serde(untagged)]
pub enum Type {
    CoreType(CoreType),
    Custom(u8),
}

impl Type {
    pub fn subtype_hint(&self) -> String {
        match self {
            Type::CoreType(ty) => ty.subtype_hint(),
            Type::Custom(_) => "0x00-0xFE".into(),
        }
    }

    pub fn as_u8(&self) -> u8 {
        match self {
            Type::CoreType(ty) => *ty as u8,
            Type::Custom(ty) => *ty,
        }
    }
}

impl Display for Type {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        let s = match *self {
            Type::CoreType(ty) => serde_plain::to_string(&ty).unwrap(),
            Type::Custom(ty) => format!("{:#04x}", ty),
        };

        write!(f, "{}", s)
    }
}

#[derive(Copy, Clone, Debug, Deserialize, Serialize, PartialEq, Eq, BinRead)]
#[repr(u8)]
#[br(little, repr = u8)]
pub enum AppType {
    #[serde(rename = "factory")]
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
    #[serde(rename = "test")]
    Test = 0x20,
}

#[derive(Copy, Clone, Debug, Deserialize, EnumIter, Serialize, PartialEq, Eq, BinRead)]
#[repr(u8)]
#[br(little, repr = u8)]
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

impl DataType {
    fn is_multiple_allowed(self) -> bool {
        matches!(self, Self::Fat | Self::Spiffs)
    }
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Copy, Clone, BinRead)]
#[serde(untagged)]
pub enum SubType {
    App(AppType),
    Data(DataType),
    #[serde(deserialize_with = "deserialize_custom_partition_sub_type")]
    Custom(u8),
}

impl Display for SubType {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        let s = match self {
            SubType::App(sub) => serde_plain::to_string(sub).unwrap(),
            SubType::Data(sub) => serde_plain::to_string(sub).unwrap(),
            SubType::Custom(sub) => format!("{:#04x}", sub),
        };

        write!(f, "{}", s)
    }
}

impl SubType {
    fn as_u8(&self) -> u8 {
        match self {
            SubType::App(ty) => *ty as u8,
            SubType::Data(ty) => *ty as u8,
            SubType::Custom(ty) => *ty as u8,
        }
    }

    fn is_multiple_allowed(self) -> bool {
        match self {
            SubType::App(_) => false,
            SubType::Data(ty) => ty.is_multiple_allowed(),
            SubType::Custom(_) => true,
        }
    }
}

#[derive(Copy, Clone, Debug, Deserialize, Serialize, PartialEq, Eq, BinRead)]
#[repr(u8)]
#[br(little, repr = u8)]
#[serde(rename_all = "lowercase")]
pub enum Flags {
    Encrypted = 0x1,
}

impl Flags {
    pub fn as_u32(&self) -> u32 {
        *self as u32
    }
}

#[derive(Debug, PartialEq, Eq, Serialize)]
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
                    Type::CoreType(CoreType::Data),
                    SubType::Data(DataType::Nvs),
                    nvs_offset,
                    nvs_size,
                    None,
                ),
                Partition::new(
                    String::from("phy_init"),
                    Type::CoreType(CoreType::Data),
                    SubType::Data(DataType::Phy),
                    phy_init_data_offset,
                    phy_init_data_size,
                    None,
                ),
                Partition::new(
                    String::from("factory"),
                    Type::CoreType(CoreType::App),
                    SubType::App(AppType::Factory),
                    app_offset,
                    app_size,
                    None,
                ),
            ],
        }
    }

    /// Attempt to parse either a binary or CSV partition table from the given
    /// input.
    ///
    /// For more information on the partition table format see:
    /// <https://docs.espressif.com/projects/esp-idf/en/latest/esp32/api-guides/partition-tables.html>
    pub fn try_from<S>(data: S) -> Result<Self, PartitionTableError>
    where
        S: Into<Vec<u8>>,
    {
        let input: Vec<u8> = data.into();

        // If a partition table was detected from ESP-IDF (eg. using `esp-idf-sys`) then
        // it will be passed in its _binary_ form. Otherwise, it will be provided as a
        // CSV.
        if let Ok(part_table) = Self::try_from_bytes(&*input) {
            Ok(part_table)
        } else if let Ok(part_table) =
            Self::try_from_str(String::from_utf8(input).map_err(|_| InvalidPartitionTable)?)
        {
            Ok(part_table)
        } else {
            Err(InvalidPartitionTable.into())
        }
    }

    /// Attempt to parse a CSV partition table from the given string.
    ///
    /// For more information on the partition table format see:
    /// <https://docs.espressif.com/projects/esp-idf/en/latest/esp32/api-guides/partition-tables.html>
    pub fn try_from_str<S>(data: S) -> Result<Self, PartitionTableError>
    where
        S: Into<String>,
    {
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

    /// Attempt to parse a binary partition table from the given bytes.
    ///
    /// For more information on the partition table format see:
    /// <https://docs.espressif.com/projects/esp-idf/en/latest/esp32/api-guides/partition-tables.html>
    pub fn try_from_bytes<S>(data: S) -> Result<Self, PartitionTableError>
    where
        S: Into<Vec<u8>>,
    {
        let data = data.into();
        if data.len() % 32 != 0 {
            return Err(PartitionTableError::LengthNotMultipleOf32(
                LengthNotMultipleOf32 {},
            ));
        }
        let mut md5 = Context::new();

        let mut partitions = vec![];
        for line in data.chunks_exact(PARTITION_SIZE) {
            if line.starts_with(MD5_PART_MAGIC_BYTES) {
                // The first 16 bytes are just the marker. The next 16 bytes is the actual md5
                // string.
                let digest_in_file = &line[16..32];
                let digest_computed = *md5.clone().compute();
                if digest_computed != digest_in_file {
                    return Err(PartitionTableError::InvalidChecksum(InvalidChecksum {}));
                }
            } else if line == END_MARKER {
                let table = Self { partitions };
                return Ok(table);
            } else {
                let mut reader = Cursor::new(line);
                let mut part: Partition = reader.read_le().unwrap();
                part.fixup_sub_type();

                partitions.push(part);
                md5.consume(line);
            }
        }
        Err(PartitionTableError::NoEndMarker(NoEndMarker {}))
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut result = Vec::with_capacity(PARTITION_TABLE_SIZE);
        self.save_bin(&mut result).unwrap();

        result
    }

    /// Write binary form of partition table into `writer`.
    pub fn save_bin<W>(&self, writer: &mut W) -> std::io::Result<()>
    where
        W: Write,
    {
        let mut hasher = HashWriter::new(writer);
        for partition in &self.partitions {
            partition.save_bin(&mut hasher)?;
        }

        let (writer, hash) = hasher.compute();

        writer.write_all(MD5_PART_MAGIC_BYTES)?;
        writer.write_all(&hash.0)?;

        let written = self.partitions.len() * PARTITION_SIZE + 32;
        for _ in 0..(MAX_PARTITION_LENGTH - written) {
            writer.write_all(&[0xFF])?;
        }

        Ok(())
    }

    /// Write CSV form of partition table into `writer`.
    pub fn save_csv<W>(&self, writer: &mut W) -> std::io::Result<()>
    where
        W: Write,
    {
        writeln!(writer, "# ESP-IDF Partition Table")?;
        writeln!(writer, "# Name,   Type, SubType, Offset,  Size, Flags")?;
        let mut csv = csv::Writer::from_writer(writer);
        for partition in &self.partitions {
            partition.save_csv(&mut csv)?;
        }

        Ok(())
    }

    pub fn find(&self, name: &str) -> Option<&Partition> {
        self.partitions.iter().find(|&p| p.name == name)
    }

    pub fn find_by_type(&self, ty: Type) -> Option<&Partition> {
        self.partitions.iter().find(|&p| p.ty == ty)
    }

    pub fn find_by_subtype(&self, ty: Type, sub_type: SubType) -> Option<&Partition> {
        self.partitions
            .iter()
            .find(|&p| p.ty == ty && p.sub_type == sub_type)
    }

    fn validate(&self, source: &str) -> Result<(), PartitionTableError> {
        for partition in &self.partitions {
            if let Some(line) = &partition.line {
                let expected_type = match partition.sub_type {
                    SubType::App(_) => Some(Type::CoreType(CoreType::App)),
                    SubType::Data(_) => Some(Type::CoreType(CoreType::Data)),
                    SubType::Custom(_) => None,
                };

                if (expected_type.is_some() && expected_type != Some(partition.ty))
                    || (expected_type.is_none() && !matches!(partition.ty, Type::Custom(_)))
                {
                    return Err(InvalidSubTypeError::new(
                        source,
                        *line,
                        partition.ty,
                        partition.sub_type,
                    )
                    .into());
                }

                if partition.ty == Type::CoreType(CoreType::App)
                    && partition.offset.rem(PARTITION_ALIGNMENT) != 0
                {
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

                        if partition1.sub_type == partition2.sub_type
                            && !SubType::is_multiple_allowed(partition1.sub_type)
                        {
                            return Err(DuplicatePartitionsError::new(
                                source, *line1, *line2, "sub-type",
                            )
                            .into());
                        }
                    }
                }
            }
        }

        if self.find_by_type(Type::CoreType(CoreType::App)).is_none() {
            return Err(PartitionTableError::NoApp(NoAppError::new(source)));
        }

        Ok(())
    }

    pub fn pretty_print(&self) {
        let mut table = Table::new();
        table
            .load_preset(UTF8_FULL)
            .apply_modifier(modifiers::UTF8_ROUND_CORNERS)
            .set_header(vec![
                Cell::new("Name")
                    .fg(Color::Green)
                    .add_attribute(Attribute::Bold),
                Cell::new("Type")
                    .fg(Color::Cyan)
                    .add_attribute(Attribute::Bold),
                Cell::new("SubType")
                    .fg(Color::Magenta)
                    .add_attribute(Attribute::Bold),
                Cell::new("Offset")
                    .fg(Color::Red)
                    .add_attribute(Attribute::Bold),
                Cell::new("Size")
                    .fg(Color::Yellow)
                    .add_attribute(Attribute::Bold),
                Cell::new("Flags")
                    .fg(Color::DarkCyan)
                    .add_attribute(Attribute::Bold),
            ]);
        for part in &self.partitions {
            table.add_row(vec![
                Cell::new(&part.name).fg(Color::Green),
                Cell::new(&part.ty.to_string()).fg(Color::Cyan),
                Cell::new(&part.sub_type.to_string()).fg(Color::Magenta),
                Cell::new(&format!("{:#x}", part.offset)).fg(Color::Red),
                Cell::new(&format!("{:#x} ({}KiB)", part.size, part.size / 1024)).fg(Color::Yellow),
                Cell::new(
                    &part
                        .flags
                        .map(|x| format!("{:#x}", x.as_u32()))
                        .unwrap_or_default(),
                )
                .fg(Color::DarkCyan),
            ]);
        }
        println!("{table}");
    }
}

#[derive(Debug, Deserialize)]
pub struct DeserializedPartition {
    #[serde(deserialize_with = "deserialize_partition_name")]
    name: String,
    #[serde(deserialize_with = "deserialize_partition_type")]
    ty: Type,
    sub_type: SubType,
    #[serde(deserialize_with = "deserialize_partition_offset")]
    offset: Option<u32>,
    #[serde(deserialize_with = "deserialize_partition_size")]
    size: u32,
    flags: Option<Flags>,
}

impl DeserializedPartition {
    fn align(offset: u32, ty: Type) -> u32 {
        let pad = match ty {
            Type::CoreType(CoreType::App) => PARTITION_ALIGNMENT,
            _ => 4,
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

#[derive(Debug, Clone, Eq, BinRead, Serialize)]
#[br(magic = b"\xAA\x50", assert(!name.is_empty()))]
pub struct Partition {
    ty: Type,
    sub_type: SubType,
    offset: u32,
    pub(crate) size: u32,
    #[br(count = 16)]
    #[br(map = |s: Vec<u8>| String::from_utf8_lossy(&s).trim_matches(char::from(0)).to_string())]
    name: String,
    #[br(try)]
    flags: Option<Flags>,
    #[br(ignore)]
    line: Option<usize>,
}

// For partial equality operations we want to ignore the `line` field
// altogether, so we cannot automatically derive an implementation of
// `PartialEq`
impl PartialEq for Partition {
    fn eq(&self, other: &Self) -> bool {
        self.ty == other.ty
            && self.sub_type == other.sub_type
            && self.offset == other.offset
            && self.size == other.size
            && self.name == other.name
            && self.flags == other.flags
    }
}

impl Partition {
    pub fn new(
        name: String,
        ty: Type,
        sub_type: SubType,
        offset: u32,
        size: u32,
        flags: Option<Flags>,
    ) -> Self {
        Partition {
            name,
            ty,
            sub_type,
            offset,
            size,
            flags,
            line: None,
        }
    }

    pub fn save_bin<W>(&self, writer: &mut W) -> std::io::Result<()>
    where
        W: Write,
    {
        writer.write_all(MAGIC_BYTES)?;
        writer.write_all(&[self.ty.as_u8(), self.sub_type.as_u8()])?;
        writer.write_all(&self.offset.to_le_bytes())?;
        writer.write_all(&self.size.to_le_bytes())?;

        let mut name_bytes = [0u8; 16];
        for (source, dest) in self.name.bytes().take(16).zip(name_bytes.iter_mut()) {
            *dest = source;
        }
        writer.write_all(&name_bytes)?;

        let flags = match &self.flags {
            Some(f) => f.as_u32().to_le_bytes(),
            None => 0u32.to_le_bytes(),
        };
        writer.write_all(&flags)?;

        Ok(())
    }

    pub fn save_csv<W>(&self, csv: &mut csv::Writer<W>) -> std::io::Result<()>
    where
        W: Write,
    {
        csv.write_record(&[
            &self.name,
            &self.ty.to_string(),
            &self.sub_type.to_string(),
            &format!("{:#x}", self.offset),
            &format!("{:#x}", self.size),
            &self
                .flags
                .map(|x| format!("{:#x}", x.as_u32()))
                .unwrap_or_default(),
        ])?;
        Ok(())
    }

    pub fn offset(&self) -> u32 {
        self.offset
    }

    pub fn size(&self) -> u32 {
        self.size
    }

    pub fn flags(&self) -> Option<Flags> {
        self.flags
    }

    pub fn fixup_sub_type(&mut self) {
        if matches!(self.ty, Type::Custom(_)) && !matches!(self.sub_type, SubType::Custom(_)) {
            self.sub_type = SubType::Custom(self.sub_type.as_u8());
        }
    }

    fn overlaps(&self, other: &Partition) -> bool {
        max(self.offset, other.offset) < min(self.offset + self.size, other.offset + other.size)
    }
}

impl From<DeserializedPartition> for Partition {
    fn from(p: DeserializedPartition) -> Self {
        Partition {
            name: p.name,
            ty: p.ty,
            sub_type: p.sub_type,
            offset: p.offset.unwrap(),
            size: p.size,
            flags: p.flags,
            line: None,
        }
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

fn deserialize_partition_type<'de, D>(deserializer: D) -> Result<Type, D::Error>
where
    D: Deserializer<'de>,
{
    use serde::de::Error;

    let buf = String::deserialize(deserializer)?;

    match buf.trim() {
        "app" => Ok(Type::CoreType(CoreType::App)),
        "data" => Ok(Type::CoreType(CoreType::Data)),
        value => match parse_int::parse::<u8>(value) {
            Ok(int) => match int {
                0x00 => Ok(Type::CoreType(CoreType::App)),
                0x01 => Ok(Type::CoreType(CoreType::Data)),
                value => Ok(Type::Custom(value)),
            },
            Err(_) => Err(Error::custom("invalid partition type")),
        },
    }
}

fn deserialize_custom_partition_sub_type<'de, D>(deserializer: D) -> Result<u8, D::Error>
where
    D: Deserializer<'de>,
{
    use serde::de::Error;

    let buf = String::deserialize(deserializer)?;
    let buf = buf.trim();

    parse_int::parse::<u8>(buf).map_err(|_| Error::custom("invalid data sub-type"))
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

    deserialize_partition_offset_or_size(deserializer)?
        .ok_or_else(|| Error::custom("invalid partition size/offset format"))
}

struct HashWriter<W> {
    inner: W,
    hasher: Context,
}

impl<W> Write for HashWriter<W>
where
    W: Write,
{
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.hasher.write_all(buf)?;
        self.inner.write(buf)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.inner.flush()
    }
}

impl<W> HashWriter<W>
where
    W: Write,
{
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
factory,  app,  factory, 0x10000, 1M, encrypted
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

    const PTABLE_SPIFFS: &str = "
# ESP-IDF Partition Table
# Name,   Type, SubType, Offset,  Size, Flags
nvs,      data, nvs,     0x9000,  0x4000,
otadata,  data, ota,     0xd000,  0x2000,
phy_init, data, phy,     0xf000,  0x1000,
factory,  app,  factory, 0x10000,  1M,
a,        data,  spiffs, 0x110000, 1M,
b,        data,  spiffs, 0x210000, 1M,
";

    const PTABLE_NO_FACTORY: &str = "
# ESP-IDF Partition Table
# Name,   Type, SubType, Offset,  Size, Flags
nvs,      data, nvs,     0x9000,  0x4000,
otadata,  data, ota,     0xd000,  0x2000,
phy_init, data, phy,     0xf000,  0x1000,
ota_0,    app,  ota_0,   , 1M,
ota_1,    app,  ota_1,   , 1M,
";

    const PTABLE_NO_APP: &str = "
# ESP-IDF Partition Table
# Name,   Type, SubType, Offset,  Size, Flags
nvs,      data, nvs,     0x9000,  0x4000,
otadata,  data, ota,     0xd000,  0x2000,
phy_init, data, phy,     0xf000,  0x1000,
";

    const PTABLE_CUSTOM_PARTITIONS: &str = "
# ESP-IDF Partition Table
# Name,   Type, SubType, Offset,  Size, Flags
nvs,      data, nvs,     0x9000,   0x6000,
phy_init, data, phy,     0xf000,   0x1000,
factory,  app,  factory, 0x10000,  0x100000,
custom,   0x40, 0x00,    0xf00000, 0x100000,    
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
    fn test_from() {
        let pt0 = PartitionTable::try_from(PTABLE_0);
        assert!(pt0.is_ok());

        let pt0 = pt0.unwrap();
        let nvs = pt0.find("nvs").unwrap();
        let fac = pt0.find("factory").unwrap();
        assert_eq!(nvs.flags(), None);
        assert_eq!(fac.flags(), Some(Flags::Encrypted));

        let pt1 = PartitionTable::try_from(PTABLE_1);
        assert!(pt1.is_ok());

        let pt_spiffs = PartitionTable::try_from(PTABLE_SPIFFS);
        assert!(pt_spiffs.is_ok());

        PartitionTable::try_from(PTABLE_NO_FACTORY)
            .expect("Failed to parse partition table without factory partition");

        PartitionTable::try_from(PTABLE_NO_APP)
            .expect_err("Failed to reject partition table without factory or ota partition");

        use std::fs::{read, read_to_string};
        let binary_table = read("./tests/data/partitions.bin").unwrap();
        let binary_parsed = PartitionTable::try_from_bytes(binary_table).unwrap();

        let csv_table = read_to_string("./tests/data/partitions.csv").unwrap();
        let csv_parsed = PartitionTable::try_from(csv_table).unwrap();

        assert_eq!(binary_parsed, csv_parsed);

        let pt_custom = PartitionTable::try_from(PTABLE_CUSTOM_PARTITIONS);
        assert!(pt_custom.is_ok());

        let ptc = pt_custom.unwrap();
        let custom = ptc.find("custom").unwrap();
        assert_eq!(custom.ty, Type::Custom(0x40));
        assert_eq!(custom.sub_type, SubType::Custom(0x00));
    }

    #[test]
    fn test_from_str() {
        let pt0 = PartitionTable::try_from_str(PTABLE_0);
        assert!(pt0.is_ok());

        let pt0 = pt0.unwrap();
        let nvs = pt0.find("nvs").unwrap();
        let fac = pt0.find("factory").unwrap();
        assert_eq!(nvs.flags(), None);
        assert_eq!(fac.flags(), Some(Flags::Encrypted));

        let pt1 = PartitionTable::try_from_str(PTABLE_1);
        assert!(pt1.is_ok());

        let pt_spiffs = PartitionTable::try_from_str(PTABLE_SPIFFS);
        assert!(pt_spiffs.is_ok());

        PartitionTable::try_from_str(PTABLE_NO_FACTORY)
            .expect("Failed to parse partition table without factory partition");

        PartitionTable::try_from_str(PTABLE_NO_APP)
            .expect_err("Failed to reject partition table without factory or ota partition");
    }

    #[test]
    fn test_from_bytes() {
        use std::fs::{read, read_to_string};
        let binary_table = read("./tests/data/partitions.bin").unwrap();
        let binary_parsed = PartitionTable::try_from_bytes(binary_table).unwrap();

        let csv_table = read_to_string("./tests/data/partitions.csv").unwrap();
        let csv_parsed = PartitionTable::try_from_str(csv_table).unwrap();

        assert_eq!(binary_parsed, csv_parsed);
    }

    #[test]
    fn test_from_csv_to_bin_and_back() {
        let pt_basic = PartitionTable::try_from_str(PTABLE_0).unwrap();

        let mut data = Vec::new();
        pt_basic.save_bin(&mut data).unwrap();
        let pt_from_bytes = PartitionTable::try_from_bytes(data).unwrap();

        assert_eq!(pt_basic, pt_from_bytes);

        let pt_custom = PartitionTable::try_from_str(PTABLE_CUSTOM_PARTITIONS).unwrap();

        let mut data = Vec::new();
        pt_custom.save_bin(&mut data).unwrap();
        let pt_from_bytes = PartitionTable::try_from_bytes(data).unwrap();

        assert_eq!(pt_custom, pt_from_bytes);
    }

    #[test]
    fn blank_offsets_are_filled_in() {
        let pt2 = PartitionTable::try_from_str(PTABLE_2)
            .expect("Failed to parse partition table with blank offsets");

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
        let pt3 = PartitionTable::try_from_str(PTABLE_3)
            .expect("Failed to parse partition table with blank offsets");

        assert_eq!(3, pt3.partitions.len());
        assert_eq!(0x4000, pt3.partitions[0].size);
        assert_eq!(0x1000, pt3.partitions[1].size);
        assert_eq!(0x100000, pt3.partitions[2].size);

        assert_eq!(0x10000, pt3.partitions[0].offset);
        assert_eq!(0x14000, pt3.partitions[1].offset);
        assert_eq!(0x20000, pt3.partitions[2].offset);
    }
}

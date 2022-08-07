use std::{
    borrow::Cow,
    cmp::Ordering,
    fmt::{Debug, Formatter},
    mem::take,
    ops::AddAssign,
    str::FromStr,
};

use strum_macros::{Display, EnumVariantNames};
use xmas_elf::{
    program::Type,
    sections::{SectionData, ShType},
    ElfFile,
};

use crate::{
    chip::Chip,
    error::{ElfError, Error},
};

pub const ESP_CHECKSUM_MAGIC: u8 = 0xef;

#[derive(Copy, Clone, Debug, EnumVariantNames)]
#[strum(serialize_all = "UPPERCASE")]
pub enum FlashMode {
    Qio,
    Qout,
    Dio,
    Dout,
}

impl FromStr for FlashMode {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mode = match s.to_uppercase().as_str() {
            "QIO" => FlashMode::Qio,
            "QOUT" => FlashMode::Qout,
            "DIO" => FlashMode::Dio,
            "DOUT" => FlashMode::Dout,
            _ => return Err(Error::InvalidFlashMode(s.to_string())),
        };

        Ok(mode)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Display, EnumVariantNames)]
#[repr(u8)]
pub enum FlashFrequency {
    #[strum(serialize = "12M")]
    Flash12M,
    #[strum(serialize = "15M")]
    Flash15M,
    #[strum(serialize = "16M")]
    Flash16M,
    #[strum(serialize = "20M")]
    Flash20M,
    #[strum(serialize = "24M")]
    Flash24M,
    #[strum(serialize = "26M")]
    Flash26M,
    #[strum(serialize = "30M")]
    Flash30M,
    #[strum(serialize = "40M")]
    Flash40M,
    #[strum(serialize = "48M")]
    Flash48M,
    #[strum(serialize = "60M")]
    Flash60M,
    #[strum(serialize = "80M")]
    Flash80M,
}

impl FromStr for FlashFrequency {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        use FlashFrequency::*;

        let freq = match s.to_uppercase().as_str() {
            "12M" => Flash12M,
            "15M" => Flash15M,
            "16M" => Flash16M,
            "20M" => Flash20M,
            "24M" => Flash24M,
            "26M" => Flash26M,
            "30M" => Flash30M,
            "40M" => Flash40M,
            "48M" => Flash48M,
            "60M" => Flash60M,
            "80M" => Flash80M,
            _ => return Err(Error::InvalidFlashFrequency(s.to_string())),
        };

        Ok(freq)
    }
}

pub trait FirmwareImage<'a> {
    fn entry(&self) -> u32;
    fn segments(&'a self) -> Box<dyn Iterator<Item = CodeSegment<'a>> + 'a>;
    fn segments_with_load_addresses(&'a self) -> Box<dyn Iterator<Item = CodeSegment<'a>> + 'a>;

    fn rom_segments(&'a self, chip: Chip) -> Box<dyn Iterator<Item = CodeSegment<'a>> + 'a> {
        Box::new(
            self.segments()
                .filter(move |segment| chip.addr_is_flash(segment.addr)),
        )
    }

    fn ram_segments(&'a self, chip: Chip) -> Box<dyn Iterator<Item = CodeSegment<'a>> + 'a> {
        Box::new(
            self.segments()
                .filter(move |segment| !chip.addr_is_flash(segment.addr)),
        )
    }
}

pub struct ElfFirmwareImage<'a> {
    elf: ElfFile<'a>,
}

impl<'a> ElfFirmwareImage<'a> {
    pub fn new(elf: ElfFile<'a>) -> Self {
        Self { elf }
    }
}

impl<'a> TryFrom<&'a [u8]> for ElfFirmwareImage<'a> {
    type Error = Error;

    fn try_from(value: &'a [u8]) -> Result<Self, Self::Error> {
        let elf = ElfFile::new(value).map_err(ElfError::from)?;

        let image = ElfFirmwareImage::new(elf);

        Ok(image)
    }
}

impl<'a> FirmwareImage<'a> for ElfFirmwareImage<'a> {
    fn entry(&self) -> u32 {
        self.elf.header.pt2.entry_point() as u32
    }

    fn segments(&'a self) -> Box<dyn Iterator<Item = CodeSegment<'a>> + 'a> {
        Box::new(
            self.elf
                .section_iter()
                .filter(|header| {
                    header.size() > 0
                        && header.get_type() == Ok(ShType::ProgBits)
                        && header.offset() > 0
                        && header.address() > 0
                })
                .flat_map(move |header| {
                    let addr = header.address() as u32;
                    let data = match header.get_data(&self.elf) {
                        Ok(SectionData::Undefined(data)) => data,
                        _ => return None,
                    };
                    Some(CodeSegment::new(addr, data))
                }),
        )
    }

    fn segments_with_load_addresses(&'a self) -> Box<dyn Iterator<Item = CodeSegment<'a>> + 'a> {
        Box::new(
            self.elf
                .program_iter()
                .filter(|header| {
                    header.file_size() > 0
                        && header.get_type() == Ok(Type::Load)
                        && header.offset() > 0
                })
                .flat_map(move |header| {
                    let addr = header.physical_addr() as u32;
                    let from = header.offset() as usize;
                    let to = header.offset() as usize + header.file_size() as usize;
                    let data = &self.elf.input[from..to];
                    Some(CodeSegment::new(addr, data))
                }),
        )
    }
}

#[derive(Eq, Clone, Default)]
/// A segment of code from the source elf
pub struct CodeSegment<'a> {
    pub addr: u32,
    data: Cow<'a, [u8]>,
}

impl<'a> CodeSegment<'a> {
    pub fn new(addr: u32, data: &'a [u8]) -> Self {
        let mut segment = CodeSegment {
            addr,
            data: Cow::Borrowed(data),
        };
        segment.pad_align(4);
        segment
    }

    /// Split of the first `count` bytes into a new segment, adjusting the
    /// remaining segment as needed
    pub fn split_off(&mut self, count: usize) -> Self {
        if count < self.data.len() {
            let (head, tail) = match take(&mut self.data) {
                Cow::Borrowed(data) => {
                    let (head, tail) = data.split_at(count);
                    (Cow::Borrowed(head), Cow::Borrowed(tail))
                }
                Cow::Owned(mut data) => {
                    let tail = data.split_off(count);
                    (Cow::Owned(data), Cow::Owned(tail))
                }
            };
            let new = CodeSegment {
                addr: self.addr,
                data: head,
            };
            self.addr += count as u32;
            self.data = tail;
            new
        } else {
            let new = self.clone();
            self.addr += self.size();
            self.data = Cow::Borrowed(&[]);
            new
        }
    }

    pub fn size(&self) -> u32 {
        self.data.len() as u32
    }

    pub fn data(&self) -> &[u8] {
        self.data.as_ref()
    }

    pub fn pad_align(&mut self, align: usize) {
        let padding = (align - self.data.len() % align) % align;
        if padding > 0 {
            let mut data = take(&mut self.data).into_owned();
            data.extend_from_slice(&[0; 4][0..padding]);
            self.data = Cow::Owned(data);
        }
    }
}

impl<'a> AddAssign<&'_ [u8]> for CodeSegment<'a> {
    fn add_assign(&mut self, rhs: &'_ [u8]) {
        let mut data = take(&mut self.data).into_owned();
        data.extend_from_slice(rhs);
        self.data = Cow::Owned(data);
    }
}

impl<'a> AddAssign<&'_ CodeSegment<'_>> for CodeSegment<'a> {
    fn add_assign(&mut self, rhs: &'_ CodeSegment<'_>) {
        let mut data = take(&mut self.data).into_owned();
        // pad or truncate
        #[allow(clippy::suspicious_op_assign_impl)]
        data.resize((rhs.addr - self.addr) as usize, 0);
        data.extend_from_slice(rhs.data());
        self.data = Cow::Owned(data);
    }
}

impl Debug for CodeSegment<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CodeSegment")
            .field("addr", &self.addr)
            .field("size", &self.size())
            .finish()
    }
}

impl PartialEq for CodeSegment<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.addr.eq(&other.addr)
    }
}

impl PartialOrd for CodeSegment<'_> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.addr.partial_cmp(&other.addr)
    }
}

impl Ord for CodeSegment<'_> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.addr.cmp(&other.addr)
    }
}

#[derive(Clone)]
/// A segment of data to write to the flash
pub struct RomSegment<'a> {
    pub addr: u32,
    pub data: Cow<'a, [u8]>,
}

impl<'a> RomSegment<'a> {
    pub fn borrow<'b>(&'b self) -> RomSegment<'b>
    where
        'a: 'b,
    {
        RomSegment {
            addr: self.addr,
            data: Cow::Borrowed(self.data.as_ref()),
        }
    }
}

impl<'a> From<CodeSegment<'a>> for RomSegment<'a> {
    fn from(segment: CodeSegment<'a>) -> Self {
        RomSegment {
            addr: segment.addr,
            data: segment.data,
        }
    }
}

pub fn update_checksum(data: &[u8], mut checksum: u8) -> u8 {
    for byte in data {
        checksum ^= *byte;
    }

    checksum
}

pub fn merge_adjacent_segments(mut segments: Vec<CodeSegment>) -> Vec<CodeSegment> {
    segments.sort();

    let mut merged: Vec<CodeSegment> = Vec::with_capacity(segments.len());
    for segment in segments {
        match merged.last_mut() {
            Some(last) if last.addr + last.size() == segment.addr => {
                *last += segment.data();
            }
            _ => {
                merged.push(segment);
            }
        }
    }

    merged
}

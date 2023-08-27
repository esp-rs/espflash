//! ELF (Executable and Linkable Format) file operations

use std::{
    borrow::Cow,
    cmp::Ordering,
    fmt::{Debug, Formatter},
    mem::take,
    ops::AddAssign,
};

use xmas_elf::{
    program::Type,
    sections::{SectionData, ShType},
    ElfFile,
};

use crate::{
    error::{ElfError, Error},
    targets::Chip,
};

/// Operations for working with firmware images
pub trait FirmwareImage<'a> {
    /// Firmware image entry point
    fn entry(&self) -> u32;

    /// Firmware image segments
    fn segments(&'a self) -> Box<dyn Iterator<Item = CodeSegment<'a>> + 'a>;

    /// Firmware image segments, with their associated load addresses
    fn segments_with_load_addresses(&'a self) -> Box<dyn Iterator<Item = CodeSegment<'a>> + 'a>;

    /// Firmware image ROM segments
    fn rom_segments(&'a self, chip: Chip) -> Box<dyn Iterator<Item = CodeSegment<'a>> + 'a> {
        Box::new(
            self.segments()
                .filter(move |segment| chip.into_target().addr_is_flash(segment.addr)),
        )
    }

    /// Firmware image RAM segments
    fn ram_segments(&'a self, chip: Chip) -> Box<dyn Iterator<Item = CodeSegment<'a>> + 'a> {
        Box::new(
            self.segments()
                .filter(move |segment| !chip.into_target().addr_is_flash(segment.addr)),
        )
    }
}

/// A firmware image built from an ELF file
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
/// A segment of code from the source ELF
pub struct CodeSegment<'a> {
    /// Base address of the code segment
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

    /// Return the size of the segment
    pub fn size(&self) -> u32 {
        self.data.len() as u32
    }

    /// Return the data of the segment
    pub fn data(&self) -> &[u8] {
        self.data.as_ref()
    }

    /// Pad the segment to the given alignment
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
        Some(self.cmp(other))
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
    /// ROM address at which the segment begins
    pub addr: u32,
    /// Segment data
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

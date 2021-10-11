use std::borrow::Cow;
use std::cmp::Ordering;

use crate::chip::Chip;
use crate::error::{ElfError, Error};
use crate::flasher::FlashSize;
use std::fmt::{Debug, Formatter};
use std::mem::take;
use std::ops::AddAssign;
use xmas_elf::sections::{SectionData, ShType};
use xmas_elf::ElfFile;

pub const ESP_CHECKSUM_MAGIC: u8 = 0xef;

#[derive(Copy, Clone)]
#[allow(dead_code)]
pub enum FlashMode {
    Qio,
    Qout,
    Dio,
    Dout,
}

#[derive(Copy, Clone)]
#[repr(u8)]
#[allow(dead_code)]
pub enum FlashFrequency {
    Flash40M = 0,
    Flash26M = 1,
    Flash20M = 2,
    Flash80M = 0xf,
}

pub struct FirmwareImage<'a> {
    pub entry: u32,
    pub elf: ElfFile<'a>,
    pub flash_mode: FlashMode,
    pub flash_size: FlashSize,
    pub flash_frequency: FlashFrequency,
}

impl<'a> FirmwareImage<'a> {
    pub fn from_data(data: &'a [u8]) -> Result<Self, Error> {
        let elf = ElfFile::new(data).map_err(ElfError::from)?;
        Ok(Self::from_elf(elf))
    }

    pub fn from_elf(elf: ElfFile<'a>) -> Self {
        FirmwareImage {
            entry: elf.header.pt2.entry_point() as u32,
            elf,
            flash_mode: FlashMode::Dio,
            flash_size: FlashSize::Flash4Mb,
            flash_frequency: FlashFrequency::Flash40M,
        }
    }

    pub fn entry(&self) -> u32 {
        self.elf.header.pt2.entry_point() as u32
    }

    pub fn segments(&'a self) -> impl Iterator<Item = CodeSegment<'a>> + 'a {
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
            })
    }

    pub fn rom_segments(&'a self, chip: Chip) -> impl Iterator<Item = CodeSegment<'a>> + 'a {
        self.segments()
            .filter(move |segment| chip.addr_is_flash(segment.addr))
    }

    pub fn ram_segments(&'a self, chip: Chip) -> impl Iterator<Item = CodeSegment<'a>> + 'a {
        self.segments()
            .filter(move |segment| !chip.addr_is_flash(segment.addr))
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

    /// Split of the first `count` bytes into a new segment, adjusting the remaining segment as needed
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

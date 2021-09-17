use std::borrow::Cow;
use std::cmp::Ordering;

use crate::chip::Chip;
use crate::flasher::FlashSize;
use std::fmt::{Debug, Formatter};
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
    pub fn from_data(data: &'a [u8]) -> Result<Self, &'static str> {
        Ok(Self::from_elf(ElfFile::new(data)?))
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

    pub fn segments(&'a self) -> impl Iterator<Item = CodeSegment> + 'a {
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

    pub fn rom_segments(&'a self, chip: Chip) -> impl Iterator<Item = CodeSegment> + 'a {
        self.segments()
            .filter(move |segment| chip.addr_is_flash(segment.addr))
    }

    pub fn ram_segments(&'a self, chip: Chip) -> impl Iterator<Item = CodeSegment> + 'a {
        self.segments()
            .filter(move |segment| !chip.addr_is_flash(segment.addr))
    }
}

#[derive(Eq, Clone)]
/// A segment of code from the source elf
pub struct CodeSegment {
    pub addr: u32,
    data: Vec<u8>,
}

impl CodeSegment {
    pub fn new(addr: u32, data: &[u8]) -> Self {
        let mut data = data.to_vec();

        // pad to 4 byte
        let padding = (4 - data.len() % 4) % 4;
        data.extend_from_slice(&[0; 4][0..padding]);

        CodeSegment { addr, data }
    }

    /// Split of the first `count` bytes into a new segment, adjusting the remaining segment as needed
    pub fn split_off(&mut self, count: usize) -> Self {
        if count < self.data.len() {
            let rest = self.data.split_off(count);
            let new = CodeSegment {
                addr: self.addr,
                data: self.data.split_off(0),
            };
            self.addr += count as u32;
            self.data = rest;
            new
        } else {
            let new = self.clone();
            self.addr += self.size();
            self.data = Vec::new();
            new
        }
    }

    pub fn add(&mut self, data: &[u8]) {
        self.data.extend_from_slice(data);
    }

    pub fn size(&self) -> u32 {
        self.data.len() as u32
    }

    pub fn data(&self) -> &[u8] {
        self.data.as_ref()
    }
}

impl Debug for CodeSegment {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CodeSegment")
            .field("addr", &self.addr)
            .field("size", &self.size())
            .finish()
    }
}

impl PartialEq for CodeSegment {
    fn eq(&self, other: &Self) -> bool {
        self.addr.eq(&other.addr)
    }
}

impl PartialOrd for CodeSegment {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.addr.partial_cmp(&other.addr)
    }
}

impl Ord for CodeSegment {
    fn cmp(&self, other: &Self) -> Ordering {
        self.addr.cmp(&other.addr)
    }
}

/// A segment of data to write to the flash
pub struct RomSegment<'a> {
    pub addr: u32,
    pub data: Cow<'a, [u8]>,
}

impl From<CodeSegment> for RomSegment<'static> {
    fn from(segment: CodeSegment) -> Self {
        RomSegment {
            addr: segment.addr,
            data: Cow::Owned(segment.data),
        }
    }
}

pub fn update_checksum(data: &[u8], mut checksum: u8) -> u8 {
    for byte in data {
        checksum ^= *byte;
    }

    checksum
}

pub fn merge_segments(mut segments: Vec<CodeSegment>) -> Vec<CodeSegment> {
    segments.sort();

    let mut merged: Vec<CodeSegment> = Vec::with_capacity(segments.len());
    for segment in segments {
        match merged.last_mut() {
            Some(last) if last.addr + last.size() == segment.addr => {
                last.add(segment.data());
            }
            _ => {
                merged.push(segment);
            }
        }
    }

    merged
}

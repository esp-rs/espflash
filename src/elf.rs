use crate::chip::Chip;
use std::borrow::Cow;
use xmas_elf::program::{SegmentData, Type};
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
pub enum FlashSize {
    Flash1MB = 0x00,
    Flash2MB = 0x10,
    Flash4MB = 0x20,
    Flash8MB = 0x30,
    Flash16MB = 0x40,
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
            flash_size: FlashSize::Flash4MB,
            flash_frequency: FlashFrequency::Flash40M,
        }
    }

    pub fn entry(&self) -> u32 {
        self.elf.header.pt2.entry_point() as u32
    }

    pub fn segments(&'a self) -> impl Iterator<Item = CodeSegment<'a>> + 'a {
        self.elf
            .program_iter()
            .filter(|header| header.file_size() > 0 && header.get_type() == Ok(Type::Load))
            .flat_map(move |header| {
                let addr = header.virtual_addr() as u32;
                let size = header.file_size() as u32;
                let data = match header.get_data(&self.elf) {
                    Ok(SegmentData::Undefined(data)) => data,
                    _ => return None,
                };
                Some(CodeSegment { addr, data, size })
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

#[derive(Debug)]
/// A segment of code from the source elf
pub struct CodeSegment<'a> {
    pub addr: u32,
    pub size: u32,
    pub data: &'a [u8],
}

/// A segment of data to write to the flash
pub struct RomSegment<'a> {
    pub addr: u32,
    pub data: Cow<'a, [u8]>,
}

pub fn update_checksum(data: &[u8], mut checksum: u8) -> u8 {
    for byte in data.as_ref() {
        checksum ^= *byte;
    }

    checksum
}

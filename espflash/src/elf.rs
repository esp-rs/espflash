use std::{
    borrow::Cow,
    cmp::Ordering,
    fmt::{Debug, Formatter},
    mem::take,
    ops::AddAssign,
    str::FromStr,
};

use strum_macros::EnumVariantNames;
use xmas_elf::{
    program::Type,
    sections::{SectionData, ShType},
    ElfFile,
};

use crate::{
    chip::Chip,
    error::{ElfError, Error},
    flasher::FlashSize,
};

pub const ESP_CHECKSUM_MAGIC: u8 = 0xef;

#[derive(Copy, Clone, EnumVariantNames)]
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

#[derive(Copy, Clone, EnumVariantNames)]
#[repr(u8)]
pub enum FlashFrequency {
    #[strum(serialize = "20M")]
    Flash20M = 0x2,
    #[strum(serialize = "26M")]
    Flash26M = 0x1,
    #[strum(serialize = "40M")]
    Flash40M = 0x0,
    #[strum(serialize = "80M")]
    Flash80M = 0xf,
}

impl FromStr for FlashFrequency {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let freq = match s.to_uppercase().as_str() {
            "20M" => FlashFrequency::Flash20M,
            "26M" => FlashFrequency::Flash26M,
            "40M" => FlashFrequency::Flash40M,
            "80M" => FlashFrequency::Flash80M,
            _ => return Err(Error::InvalidFlashFrequency(s.to_string())),
        };

        Ok(freq)
    }
}

pub struct FirmwareImage<'a> {
    pub entry: u32,
    pub elf: ElfFile<'a>,
    pub flash_mode: Option<FlashMode>,
    pub flash_size: Option<FlashSize>,
    pub flash_frequency: Option<FlashFrequency>,
}

impl<'a> FirmwareImage<'a> {
    pub fn new(
        elf: ElfFile<'a>,
        flash_mode: Option<FlashMode>,
        flash_size: Option<FlashSize>,
        flash_frequency: Option<FlashFrequency>,
    ) -> Self {
        Self {
            entry: elf.header.pt2.entry_point() as u32,
            elf,
            flash_mode,
            flash_size,
            flash_frequency,
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

    pub fn segments_with_load_addresses(&'a self) -> impl Iterator<Item = CodeSegment<'a>> + 'a {
        self.elf
            .program_iter()
            .filter(|header| {
                header.file_size() > 0 && header.get_type() == Ok(Type::Load) && header.offset() > 0
            })
            .flat_map(move |header| {
                let addr = header.physical_addr() as u32;
                let from = header.offset() as usize;
                let to = header.offset() as usize + header.file_size() as usize;
                let data = &self.elf.input[from..to];
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

pub struct FirmwareImageBuilder<'a> {
    data: &'a [u8],
    pub flash_mode: Option<FlashMode>,
    pub flash_size: Option<FlashSize>,
    pub flash_freq: Option<FlashFrequency>,
}

impl<'a> FirmwareImageBuilder<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Self {
            data,
            flash_mode: None,
            flash_size: None,
            flash_freq: None,
        }
    }

    pub fn flash_mode(mut self, flash_mode: Option<FlashMode>) -> Self {
        self.flash_mode = flash_mode;
        self
    }

    pub fn flash_size(mut self, flash_size: Option<FlashSize>) -> Self {
        self.flash_size = flash_size;
        self
    }

    pub fn flash_freq(mut self, flash_freq: Option<FlashFrequency>) -> Self {
        self.flash_freq = flash_freq;
        self
    }

    pub fn build(&self) -> Result<FirmwareImage<'a>, Error> {
        let elf = ElfFile::new(self.data).map_err(ElfError::from)?;

        let image = FirmwareImage::new(elf, self.flash_mode, self.flash_size, self.flash_freq);

        Ok(image)
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

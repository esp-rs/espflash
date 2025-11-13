//! Binary application image formats

use std::{
    borrow::Cow,
    cmp::Ordering,
    collections::HashMap,
    fmt::{Debug, Formatter},
    mem::take,
    ops::AddAssign,
};

use object::{
    Endianness, Object as _, ObjectSection as _,
    elf::SHT_PROGBITS,
    read::elf::{ElfFile32 as ElfFile, SectionHeader},
};
use serde::{Deserialize, Serialize};

pub use self::metadata::Metadata;
use crate::{image_format::idf::IdfBootloaderFormat, target::Chip};

pub mod idf;
mod metadata;

/// Supported binary application image formats
#[cfg_attr(feature = "cli", derive(clap::ValueEnum))]
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum ImageFormatKind {
    /// ESP-IDF application image format
    ///
    /// See: <https://docs.espressif.com/projects/esp-idf/en/stable/esp32/api-reference/system/app_image_format.html>
    #[default]
    EspIdf,
}

/// Binary application image format data
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[non_exhaustive]
pub enum ImageFormat<'a> {
    /// ESP-IDF application image format
    ///
    /// See: <https://docs.espressif.com/projects/esp-idf/en/stable/esp32/api-reference/system/app_image_format.html>
    EspIdf(IdfBootloaderFormat<'a>),
}

impl<'a> ImageFormat<'a> {
    /// Returns all flashable data segments
    pub fn flash_segments(self) -> Vec<Segment<'a>> {
        match self {
            ImageFormat::EspIdf(idf) => idf.flash_segments().collect(),
        }
    }

    /// Returns all data segments required for OTA updates
    pub fn ota_segments(self) -> Vec<Segment<'a>> {
        match self {
            ImageFormat::EspIdf(idf) => idf.ota_segments().collect(),
        }
    }

    /// Returns metadata about the application image
    pub fn metadata(&self) -> HashMap<&str, String> {
        match self {
            ImageFormat::EspIdf(idf) => idf.metadata(),
        }
    }
}

impl<'a> From<IdfBootloaderFormat<'a>> for ImageFormat<'a> {
    fn from(idf: IdfBootloaderFormat<'a>) -> Self {
        Self::EspIdf(idf)
    }
}

/// A segment of code from the source ELF
#[derive(Default, Clone, Eq, Deserialize, Serialize)]
pub struct Segment<'a> {
    /// Base address of the code segment
    pub addr: u32,
    /// Segment data
    pub data: Cow<'a, [u8]>,
}

impl<'a> Segment<'a> {
    /// Creates a new [`Segment`].
    pub fn new(addr: u32, data: &'a [u8]) -> Self {
        // Do not pad the data here, as it might result in overlapping segments
        // in the ELF file. The padding should be done after merging adjacent segments.
        Segment {
            addr,
            data: Cow::Borrowed(data),
        }
    }

    /// Splits off the first `count` bytes into a new segment, adjusting the
    /// remaining segment as needed.
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
            let new = Segment {
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

    /// Borrow the segment for the given lifetime
    pub fn borrow<'b>(&'b self) -> Segment<'b>
    where
        'a: 'b,
    {
        Segment {
            addr: self.addr,
            data: Cow::Borrowed(self.data.as_ref()),
        }
    }
}

impl AddAssign<&'_ [u8]> for Segment<'_> {
    fn add_assign(&mut self, rhs: &'_ [u8]) {
        let mut data = take(&mut self.data).into_owned();
        data.extend_from_slice(rhs);
        self.data = Cow::Owned(data);
    }
}

#[allow(clippy::suspicious_op_assign_impl)]
impl AddAssign<&'_ Segment<'_>> for Segment<'_> {
    fn add_assign(&mut self, rhs: &'_ Segment<'_>) {
        let mut data = take(&mut self.data).into_owned();
        // Pad or truncate:
        data.resize((rhs.addr - self.addr) as usize, 0);
        data.extend_from_slice(rhs.data());
        self.data = Cow::Owned(data);
    }
}

impl Debug for Segment<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CodeSegment")
            .field("addr", &self.addr)
            .field("size", &self.size())
            .finish()
    }
}

impl PartialEq for Segment<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.addr.eq(&other.addr)
    }
}

impl PartialOrd for Segment<'_> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Segment<'_> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.addr.cmp(&other.addr)
    }
}

/// Returns an iterator over all RAM segments for a given chip and ELF file.
pub(crate) fn ram_segments<'a>(
    chip: Chip,
    elf: &'a ElfFile<'a>,
) -> impl Iterator<Item = Segment<'a>> {
    segments(elf).filter(move |segment| !chip.addr_is_flash(segment.addr))
}

/// Returns an iterator over all ROM segments for a given chip and ELF file.
pub(crate) fn rom_segments<'a>(
    chip: Chip,
    elf: &'a ElfFile<'a>,
) -> impl Iterator<Item = Segment<'a>> {
    segments(elf).filter(move |segment| chip.addr_is_flash(segment.addr))
}

fn segments<'a>(elf: &'a ElfFile<'a>) -> impl Iterator<Item = Segment<'a>> {
    elf.sections()
        .filter(|section| {
            let header = section.elf_section_header();

            section.size() > 0
                && header.sh_type(Endianness::Little) == SHT_PROGBITS
                && header.sh_offset.get(Endianness::Little) > 0
                && section.address() > 0
                && !is_empty(section.flags())
        })
        .flat_map(move |section| match section.data() {
            Ok(data) => Some(Segment::new(section.address() as u32, data)),
            _ => None,
        })
}

fn is_empty(flags: object::SectionFlags) -> bool {
    match flags {
        object::SectionFlags::None => true,
        object::SectionFlags::Elf { sh_flags } => sh_flags == 0,
        _ => unreachable!(),
    }
}

#[cfg(test)]
mod test {
    use object::read::elf::ElfFile;

    use super::segments;

    #[test]
    fn test_overlapping_sections_are_removed() {
        let elf_data: Vec<u8> = std::fs::read(
            "tests/data/esp_hal_binary_with_overlapping_defmt_and_embedded_test_sections",
        )
        .unwrap();

        let elf = ElfFile::parse(elf_data.as_slice()).unwrap();
        let segments = segments(&elf).collect::<Vec<_>>();

        let expected = [
            // (address, size)
            (0x3F400020, 256),   // .rodata_desc
            (0x3F400120, 29152), // .rodata
            (0x3FFB0000, 3716),  // .data
            (0x40080000, 1024),  // .vectors
            (0x40080400, 5088),  // .rwtext
            (0x400D0020, 62654), // .text
        ];

        assert_eq!(segments.len(), expected.len());

        for seg in segments {
            let addr_and_len = (seg.addr, seg.size());
            assert!(
                expected.contains(&addr_and_len),
                "Unexpected section: {addr_and_len:x?}"
            )
        }
    }
}

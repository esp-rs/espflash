//! Binary application image formats

use std::{
    borrow::Cow,
    cmp::Ordering,
    fmt::{Debug, Formatter},
    mem::take,
    ops::AddAssign,
};

use xmas_elf::{
    sections::{SectionData, ShType},
    ElfFile,
};

pub use self::esp_idf::IdfBootloaderFormat;
use crate::targets::Chip;

mod esp_idf;

/// A segment of code from the source ELF
#[derive(Default, Clone, Eq)]
pub struct Segment<'a> {
    /// Base address of the code segment
    pub addr: u32,
    /// Segment data
    pub data: Cow<'a, [u8]>,
}

impl<'a> Segment<'a> {
    pub fn new(addr: u32, data: &'a [u8]) -> Self {
        let mut segment = Segment {
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
) -> Box<dyn Iterator<Item = Segment<'a>> + 'a> {
    Box::new(segments(elf).filter(move |segment| !chip.into_target().addr_is_flash(segment.addr)))
}

/// Returns an iterator over all ROM segments for a given chip and ELF file.
pub(crate) fn rom_segments<'a>(
    chip: Chip,
    elf: &'a ElfFile<'a>,
) -> Box<dyn Iterator<Item = Segment<'a>> + 'a> {
    Box::new(segments(elf).filter(move |segment| chip.into_target().addr_is_flash(segment.addr)))
}

fn segments<'a>(elf: &'a ElfFile<'a>) -> Box<dyn Iterator<Item = Segment<'a>> + 'a> {
    Box::new(
        elf.section_iter()
            .filter(|header| {
                header.size() > 0
                    && header.get_type() == Ok(ShType::ProgBits)
                    && header.offset() > 0
                    && header.address() > 0
            })
            .flat_map(move |header| {
                let addr = header.address() as u32;
                match header.get_data(elf) {
                    Ok(SectionData::Undefined(data)) => Some(Segment::new(addr, data)),
                    _ => None,
                }
            }),
    )
}

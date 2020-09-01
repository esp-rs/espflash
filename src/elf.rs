use xmas_elf::program::{SegmentData, Type};
use xmas_elf::ElfFile;

const IROM_MAP_START: u32 = 0x40200000;
const IROM_MAP_END: u32 = 0x40300000;

pub struct FirmwareImage<'a> {
    elf: ElfFile<'a>,
}

impl<'a> FirmwareImage<'a> {
    pub fn from_data(data: &'a [u8]) -> Result<Self, &'static str> {
        Ok(Self::from_elf(ElfFile::new(data)?))
    }

    pub fn from_elf(elf: ElfFile<'a>) -> Self {
        FirmwareImage { elf }
    }

    pub fn entry(&self) -> u32 {
        self.elf.header.pt2.entry_point() as u32
    }

    fn segments(&'a self) -> impl Iterator<Item = CodeSegment<'a>> + 'a {
        self.elf
            .program_iter()
            .filter(|section| {
                section.file_size() > 0
                    && section.get_type() == Ok(Type::Load)
                    && section.flags().is_execute()
            })
            .flat_map(move |header| {
                let addr = header.physical_addr() as u32;
                let size = header.mem_size() as u32;
                let data = match header.get_data(&self.elf) {
                    Ok(SegmentData::Undefined(data)) => data,
                    _ => return None,
                };
                Some(CodeSegment { addr, data, size })
            })
    }

    pub fn rom_segments(&'a self) -> impl Iterator<Item = CodeSegment<'a>> + 'a {
        self.segments().filter(|segment| segment.is_rom())
    }

    pub fn ram_segments(&'a self) -> impl Iterator<Item = CodeSegment<'a>> + 'a {
        self.segments().filter(|segment| !segment.is_rom())
    }
}

#[derive(Debug)]
pub struct CodeSegment<'a> {
    pub addr: u32,
    pub size: u32,
    pub data: &'a [u8],
}

impl<'a> CodeSegment<'a> {
    pub fn is_rom(&self) -> bool {
        self.addr >= IROM_MAP_START && self.addr < IROM_MAP_END
    }
}

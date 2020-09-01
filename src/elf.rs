use crate::Error;
use bytemuck::__core::iter::once;
use byteorder::{LittleEndian, WriteBytesExt};
use std::borrow::Cow;
use std::io::Write;
use xmas_elf::program::{SegmentData, Type};
use xmas_elf::ElfFile;

pub const IROM_MAP_START: u32 = 0x40200000;
const IROM_MAP_END: u32 = 0x40300000;

const ESP8266V1_MAGIC: u8 = 0xe9;
const ESP_CHECKSUM_MAGIC: u8 = 0xef;

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
    entry: u32,
    elf: ElfFile<'a>,
    flash_mode: FlashMode,
    flash_size: FlashSize,
    flash_frequency: FlashFrequency,
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

    pub fn save<Target: ESPTarget<'a>>(
        &'a self,
    ) -> impl Iterator<Item = Result<ImageSegment<'a>, Error>> {
        Target::save(self)
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

pub struct ImageSegment<'a> {
    pub addr: u32,
    pub data: Cow<'a, [u8]>,
}

pub trait ESPTarget<'a> {
    type Iter: Iterator<Item = Result<ImageSegment<'a>, Error>>;

    fn save(image: &'a FirmwareImage) -> Self::Iter;
}

pub struct ESP8266V1;

impl<'a> ESPTarget<'a> for ESP8266V1 {
    type Iter = std::iter::Chain<
        std::option::IntoIter<std::result::Result<ImageSegment<'a>, Error>>,
        std::iter::Once<std::result::Result<ImageSegment<'a>, Error>>,
    >;

    fn save(image: &'a FirmwareImage) -> Self::Iter {
        // irom goes into a separate plain bin
        let irom_data = image
            .rom_segments()
            .next()
            .map(|segment| {
                Ok(ImageSegment {
                    addr: segment.addr,
                    data: Cow::Borrowed(segment.data),
                })
            })
            .into_iter();

        // my kingdom for a try {} block
        fn common<'a>(image: &'a FirmwareImage) -> Result<ImageSegment<'a>, Error> {
            let mut common_data = Vec::with_capacity(
                image
                    .ram_segments()
                    .map(|segment| segment.size as usize)
                    .sum(),
            );
            // common header
            common_data.write_u8(ESP8266V1_MAGIC)?;
            common_data.write_u8(image.ram_segments().count() as u8)?;
            common_data.write_u8(image.flash_mode as u8)?;
            common_data.write_u8(image.flash_size as u8 + image.flash_frequency as u8)?;
            common_data.write_u32::<LittleEndian>(image.entry)?;

            let mut total_len = 8;

            let mut checksum = ESP_CHECKSUM_MAGIC;

            for segment in image.ram_segments() {
                let data = segment.data;
                let padding = 4 - data.len() % 4;
                common_data.write_u32::<LittleEndian>(segment.addr)?;
                common_data.write_u32::<LittleEndian>((data.len() + padding) as u32)?;
                common_data.write(data)?;
                for _ in 0..padding {
                    common_data.write_u8(0)?;
                }
                total_len += 8 + data.len() + padding;
                checksum = update_checksum(data, checksum);
            }

            let padding = 15 - (total_len % 16);
            for _ in 0..padding {
                common_data.write_u8(0)?;
            }

            common_data.write_u8(checksum)?;

            Ok(ImageSegment {
                addr: 0,
                data: Cow::Owned(common_data),
            })
        }

        irom_data.chain(once(common(image)))
    }
}

pub fn update_checksum(data: &[u8], mut checksum: u8) -> u8 {
    for byte in data.as_ref() {
        checksum ^= *byte;
    }

    checksum
}

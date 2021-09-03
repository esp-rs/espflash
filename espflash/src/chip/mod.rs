use bytemuck::{bytes_of, Pod, Zeroable};

use crate::{
    elf::{update_checksum, CodeSegment, FirmwareImage, RomSegment},
    flasher::FlashSize,
    Error,
};

use std::{io::Write, str::FromStr};

pub use esp32::Esp32;
pub use esp8266::Esp8266;

mod esp32;
mod esp8266;

const ESP_MAGIC: u8 = 0xE9;
const WP_PIN_DISABLED: u8 = 0xEE;

pub trait ChipType {
    const CHIP_DETECT_MAGIC_VALUE: u32;
    const CHIP_DETECT_MAGIC_VALUE2: u32 = 0x0; // give default value, as most chips don't only have one

    const SPI_REGISTERS: SpiRegisters;

    /// Get the firmware segments for writing an image to flash
    fn get_flash_segments<'a>(
        image: &'a FirmwareImage,
    ) -> Box<dyn Iterator<Item = Result<RomSegment<'a>, Error>> + 'a>;

    fn addr_is_flash(addr: u32) -> bool;
}

pub struct SpiRegisters {
    base: u32,
    usr_offset: u32,
    usr1_offset: u32,
    usr2_offset: u32,
    w0_offset: u32,
    mosi_length_offset: Option<u32>,
    miso_length_offset: Option<u32>,
}

impl SpiRegisters {
    pub fn cmd(&self) -> u32 {
        self.base
    }

    pub fn usr(&self) -> u32 {
        self.base + self.usr_offset
    }

    pub fn usr1(&self) -> u32 {
        self.base + self.usr1_offset
    }

    pub fn usr2(&self) -> u32 {
        self.base + self.usr2_offset
    }

    pub fn w0(&self) -> u32 {
        self.base + self.w0_offset
    }

    pub fn mosi_length(&self) -> Option<u32> {
        self.mosi_length_offset.map(|offset| self.base + offset)
    }

    pub fn miso_length(&self) -> Option<u32> {
        self.miso_length_offset.map(|offset| self.base + offset)
    }
}

#[derive(Copy, Clone, Zeroable, Pod)]
#[repr(C)]
struct ExtendedHeader {
    wp_pin: u8,
    clk_q_drv: u8,
    d_cs_drv: u8,
    gd_wp_drv: u8,
    chip_id: u16,
    min_rev: u8,
    padding: [u8; 8],
    append_digest: u8,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum Chip {
    Esp32,
    Esp8266,
}

impl Chip {
    pub fn from_magic(magic: u32) -> Option<Self> {
        match magic {
            Esp32::CHIP_DETECT_MAGIC_VALUE => Some(Chip::Esp32),
            Esp8266::CHIP_DETECT_MAGIC_VALUE => Some(Chip::Esp8266),
            _ => None,
        }
    }

    pub fn get_flash_segments<'a>(
        &self,
        image: &'a FirmwareImage,
    ) -> Box<dyn Iterator<Item = Result<RomSegment<'a>, Error>> + 'a> {
        match self {
            Chip::Esp32 => Esp32::get_flash_segments(image),
            Chip::Esp8266 => Esp8266::get_flash_segments(image),
        }
    }

    pub fn addr_is_flash(&self, addr: u32) -> bool {
        match self {
            Chip::Esp32 => Esp32::addr_is_flash(addr),
            Chip::Esp8266 => Esp8266::addr_is_flash(addr),
        }
    }

    pub fn spi_registers(&self) -> SpiRegisters {
        match self {
            Chip::Esp32 => Esp32::SPI_REGISTERS,
            Chip::Esp8266 => Esp8266::SPI_REGISTERS,
        }
    }
}

impl FromStr for Chip {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "esp32" => Ok(Chip::Esp32),
            "esp8266" => Ok(Chip::Esp8266),
            _ => Err(Error::UnrecognizedChip),
        }
    }
}

#[derive(Copy, Clone, Zeroable, Pod, Debug)]
#[repr(C)]
struct EspCommonHeader {
    magic: u8,
    segment_count: u8,
    flash_mode: u8,
    flash_config: u8,
    entry: u32,
}

#[derive(Copy, Clone, Zeroable, Pod, Debug)]
#[repr(C)]
struct SegmentHeader {
    addr: u32,
    length: u32,
}

fn encode_flash_size(size: FlashSize) -> Result<u8, Error> {
    match size {
        FlashSize::Flash256Kb => Err(Error::UnsupportedFlash(size as u8)),
        FlashSize::Flash512Kb => Err(Error::UnsupportedFlash(size as u8)),
        FlashSize::Flash1Mb => Ok(0x00),
        FlashSize::Flash2Mb => Ok(0x10),
        FlashSize::Flash4Mb => Ok(0x20),
        FlashSize::Flash8Mb => Ok(0x30),
        FlashSize::Flash16Mb => Ok(0x40),
        FlashSize::FlashRetry => Err(Error::UnsupportedFlash(size as u8)),
    }
}

const IROM_ALIGN: u32 = 65536;
const SEG_HEADER_LEN: u32 = 8;

/// Actual alignment (in data bytes) required for a segment header: positioned
/// so that after we write the next 8 byte header, file_offs % IROM_ALIGN ==
/// segment.addr % IROM_ALIGN
///
/// (this is because the segment's vaddr may not be IROM_ALIGNed, more likely is
/// aligned IROM_ALIGN+0x18 to account for the binary file header
fn get_segment_padding(offset: usize, segment: &CodeSegment) -> u32 {
    let align_past = (segment.addr % IROM_ALIGN) - SEG_HEADER_LEN;
    let pad_len = (IROM_ALIGN - ((offset as u32) % IROM_ALIGN)) + align_past;
    if pad_len == 0 || pad_len == IROM_ALIGN {
        0
    } else if pad_len > SEG_HEADER_LEN {
        pad_len - SEG_HEADER_LEN
    } else {
        pad_len + IROM_ALIGN - SEG_HEADER_LEN
    }
}

fn save_flash_segment(
    data: &mut Vec<u8>,
    segment: &CodeSegment,
    checksum: u8,
) -> Result<u8, Error> {
    let end_pos = (data.len() + segment.data.len()) as u32 + SEG_HEADER_LEN;
    let segment_reminder = end_pos % IROM_ALIGN;

    let checksum = save_segment(data, segment, checksum)?;

    if segment_reminder < 0x24 {
        // Work around a bug in ESP-IDF 2nd stage bootloader, that it didn't map the
        // last MMU page, if an IROM/DROM segment was < 0x24 bytes over the page
        // boundary.
        data.write_all(&[0u8; 0x24][0..(0x24 - segment_reminder as usize)])?;
    }
    Ok(checksum)
}

fn save_segment(data: &mut Vec<u8>, segment: &CodeSegment, checksum: u8) -> Result<u8, Error> {
    let padding = (4 - segment.data.len() % 4) % 4;

    let header = SegmentHeader {
        addr: segment.addr,
        length: (segment.data.len() + padding) as u32,
    };
    data.write_all(bytes_of(&header))?;
    data.write_all(segment.data)?;

    let padding = &[0u8; 4][0..padding];
    data.write_all(padding)?;

    Ok(update_checksum(segment.data, checksum))
}

use bytemuck::{bytes_of, Pod, Zeroable};
use strum_macros::Display;

use crate::{
    elf::{update_checksum, CodeSegment, FirmwareImage, RomSegment},
    error::{ChipDetectError, FlashDetectError},
    flash_target::{Esp32CompressedTarget, Esp8266Target, FlashTarget, RamTarget},
    flasher::{FlashSize, SpiAttachParams},
    Error, PartitionTable,
};

use std::io::Write;

use crate::flash_target::{Esp32Target, FailOver};
pub use esp32::Esp32;
pub use esp32c3::Esp32c3;
pub use esp32s2::Esp32s2;
pub use esp8266::Esp8266;

mod esp32;
mod esp32c3;
mod esp32s2;
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
        bootloader: Option<Vec<u8>>,
        partition_table: Option<PartitionTable>,
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

#[derive(Debug, Copy, Clone, Eq, PartialEq, Display)]
pub enum Chip {
    #[strum(serialize = "ESP32")]
    Esp32,
    #[strum(serialize = "ESP32-C3")]
    Esp32c3,
    #[strum(serialize = "ESP32-S2")]
    Esp32s2,
    #[strum(serialize = "ESP8266")]
    Esp8266,
}

impl Chip {
    pub fn from_magic(magic: u32) -> Result<Self, ChipDetectError> {
        match magic {
            Esp32::CHIP_DETECT_MAGIC_VALUE => Ok(Chip::Esp32),
            Esp32c3::CHIP_DETECT_MAGIC_VALUE | Esp32c3::CHIP_DETECT_MAGIC_VALUE2 => {
                Ok(Chip::Esp32c3)
            }
            Esp32s2::CHIP_DETECT_MAGIC_VALUE => Ok(Chip::Esp32s2),
            Esp8266::CHIP_DETECT_MAGIC_VALUE => Ok(Chip::Esp8266),
            _ => Err(ChipDetectError::from(magic)),
        }
    }

    pub fn get_flash_segments<'a>(
        &self,
        image: &'a FirmwareImage,
        bootloader: Option<Vec<u8>>,
        partition_table: Option<PartitionTable>,
    ) -> Box<dyn Iterator<Item = Result<RomSegment<'a>, Error>> + 'a> {
        match self {
            Chip::Esp32 => Esp32::get_flash_segments(image, bootloader, partition_table),
            Chip::Esp32c3 => Esp32c3::get_flash_segments(image, bootloader, partition_table),
            Chip::Esp32s2 => Esp32s2::get_flash_segments(image, bootloader, partition_table),
            Chip::Esp8266 => Esp8266::get_flash_segments(image, None, None),
        }
    }

    pub fn addr_is_flash(&self, addr: u32) -> bool {
        match self {
            Chip::Esp32 => Esp32::addr_is_flash(addr),
            Chip::Esp32c3 => Esp32c3::addr_is_flash(addr),
            Chip::Esp32s2 => Esp32s2::addr_is_flash(addr),
            Chip::Esp8266 => Esp8266::addr_is_flash(addr),
        }
    }

    pub fn spi_registers(&self) -> SpiRegisters {
        match self {
            Chip::Esp32 => Esp32::SPI_REGISTERS,
            Chip::Esp32c3 => Esp32c3::SPI_REGISTERS,
            Chip::Esp32s2 => Esp32s2::SPI_REGISTERS,
            Chip::Esp8266 => Esp8266::SPI_REGISTERS,
        }
    }

    pub fn ram_target(&self) -> Box<dyn FlashTarget> {
        Box::new(RamTarget::new())
    }

    pub fn flash_target(&self, spi_params: SpiAttachParams) -> Box<dyn FlashTarget> {
        match self {
            Chip::Esp8266 => Box::new(Esp8266Target::new()),
            _ => Box::new(FailOver::new(
                Esp32CompressedTarget::new(*self, spi_params),
                Esp32Target::new(*self, spi_params),
            )),
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

// Note that this function ONLY applies to the ESP32 and variants; the ESP8266
// has defined its own version rather than using this implementation.
fn encode_flash_size(size: FlashSize) -> Result<u8, FlashDetectError> {
    match size {
        FlashSize::Flash256Kb => Err(FlashDetectError::from(size as u8)),
        FlashSize::Flash512Kb => Err(FlashDetectError::from(size as u8)),
        FlashSize::Flash1Mb => Ok(0x00),
        FlashSize::Flash2Mb => Ok(0x10),
        FlashSize::Flash4Mb => Ok(0x20),
        FlashSize::Flash8Mb => Ok(0x30),
        FlashSize::Flash16Mb => Ok(0x40),
        FlashSize::FlashRetry => Err(FlashDetectError::from(size as u8)),
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

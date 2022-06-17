use std::{borrow::Cow, io::Write, iter::once, mem::size_of};

use bytemuck::bytes_of;

use super::encode_flash_frequency;
use crate::{
    elf::{update_checksum, CodeSegment, FirmwareImage, RomSegment, ESP_CHECKSUM_MAGIC},
    error::{Error, FlashDetectError},
    flasher::FlashSize,
    image_format::{EspCommonHeader, ImageFormat, SegmentHeader, ESP_MAGIC},
    Chip, FlashFrequency, FlashMode,
};

/// Image format for flashing to esp8266 chips
pub struct Esp8266Format<'a> {
    irom_data: Option<RomSegment<'a>>,
    flash_segment: RomSegment<'a>,
}

impl<'a> Esp8266Format<'a> {
    pub fn new(
        image: &'a dyn FirmwareImage<'a>,
        flash_mode: Option<FlashMode>,
        flash_size: Option<FlashSize>,
        flash_freq: Option<FlashFrequency>,
    ) -> Result<Self, Error> {
        // IROM goes into a separate plain binary
        let irom_data = merge_rom_segments(image.rom_segments(Chip::Esp8266));

        let mut common_data = Vec::with_capacity(
            image
                .ram_segments(Chip::Esp8266)
                .map(|segment| segment.size() as usize)
                .sum(),
        );

        // Common header
        let flash_size = flash_size.unwrap_or(FlashSize::Flash4Mb);
        let flash_freq = flash_freq.unwrap_or(FlashFrequency::Flash40M);

        let header = EspCommonHeader {
            magic: ESP_MAGIC,
            segment_count: image.ram_segments(Chip::Esp8266).count() as u8,
            flash_mode: flash_mode.unwrap_or(FlashMode::Dio) as u8,
            flash_config: encode_flash_size(flash_size)?
                + encode_flash_frequency(Chip::Esp8266, flash_freq)?,
            entry: image.entry(),
        };
        common_data.write_all(bytes_of(&header))?;

        let mut total_len = 8;
        let mut checksum = ESP_CHECKSUM_MAGIC;

        for segment in image.ram_segments(Chip::Esp8266) {
            let data = segment.data();
            let padding = 4 - data.len() % 4;
            let segment_header = SegmentHeader {
                addr: segment.addr,
                length: (data.len() + padding) as u32,
            };
            total_len += size_of::<SegmentHeader>() as u32 + segment_header.length;
            common_data.write_all(bytes_of(&segment_header))?;
            common_data.write_all(data)?;

            let padding = &[0u8; 4][0..padding];
            common_data.write_all(padding)?;
            checksum = update_checksum(data, checksum);
        }

        let padding = 15 - (total_len % 16);
        let padding = &[0u8; 16][0..padding as usize];
        common_data.write_all(padding)?;
        common_data.write_all(&[checksum])?;

        let flash_segment = RomSegment {
            addr: 0,
            data: Cow::Owned(common_data),
        };

        Ok(Self {
            irom_data,
            flash_segment,
        })
    }
}

impl<'a> ImageFormat<'a> for Esp8266Format<'a> {
    fn flash_segments<'b>(&'b self) -> Box<dyn Iterator<Item = RomSegment<'b>> + 'b>
    where
        'a: 'b,
    {
        Box::new(
            self.irom_data
                .iter()
                .map(RomSegment::borrow)
                .chain(once(self.flash_segment.borrow())),
        )
    }

    fn ota_segments<'b>(&'b self) -> Box<dyn Iterator<Item = RomSegment<'b>> + 'b>
    where
        'a: 'b,
    {
        Box::new(
            self.irom_data
                .iter()
                .map(RomSegment::borrow)
                .chain(once(self.flash_segment.borrow())),
        )
    }
}

fn merge_rom_segments<'a>(
    mut segments: impl Iterator<Item = CodeSegment<'a>>,
) -> Option<RomSegment<'a>> {
    const IROM_MAP_START: u32 = 0x40200000;

    let first = segments.next()?;
    let data = if let Some(second) = segments.next() {
        let mut data = Vec::with_capacity(first.data().len() + second.data().len());
        data.extend_from_slice(first.data());

        for segment in once(second).chain(segments) {
            let padding_size = segment.addr as usize - first.addr as usize - data.len();
            data.resize(data.len() + padding_size, 0);
            data.extend_from_slice(segment.data());
        }

        data
    } else {
        first.data().into()
    };

    Some(RomSegment {
        addr: first.addr - IROM_MAP_START,
        data: Cow::Owned(data),
    })
}

fn encode_flash_size(size: FlashSize) -> Result<u8, FlashDetectError> {
    use FlashSize::*;

    match size {
        Flash256Kb => Ok(0x10),
        Flash512Kb => Ok(0x00),
        Flash1Mb => Ok(0x20),
        Flash2Mb => Ok(0x30),
        Flash4Mb => Ok(0x40),
        Flash8Mb => Ok(0x80),
        Flash16Mb => Ok(0x90),
        _ => Err(FlashDetectError::from(size as u8)),
    }
}

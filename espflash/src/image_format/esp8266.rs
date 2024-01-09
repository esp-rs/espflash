use std::{borrow::Cow, io::Write, iter::once, mem::size_of};

use bytemuck::bytes_of;

use crate::{
    elf::{CodeSegment, FirmwareImage, RomSegment},
    error::Error,
    flasher::FlashSettings,
    image_format::{
        update_checksum, ImageFormat, ImageHeader, SegmentHeader, ESP_CHECKSUM_MAGIC, ESP_MAGIC,
    },
    targets::Chip,
};

/// Image format for flashing to the ESP8266
pub struct Esp8266Format<'a> {
    irom_data: Option<RomSegment<'a>>,
    flash_segment: RomSegment<'a>,
    app_size: u32,
}

impl<'a> Esp8266Format<'a> {
    pub fn new(
        image: &'a dyn FirmwareImage<'a>,
        flash_settings: FlashSettings,
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
        let flash_mode = flash_settings.mode.unwrap_or_default() as u8;
        let segment_count = image.ram_segments(Chip::Esp8266).count() as u8;

        let mut header = ImageHeader {
            magic: ESP_MAGIC,
            segment_count,
            flash_mode,
            entry: image.entry(),
            ..Default::default()
        };
        header.write_flash_config(
            flash_settings.size.unwrap_or_default(),
            flash_settings.freq.unwrap_or_default(),
            Chip::Esp8266,
        )?;

        // Esp8266 does not have extended header
        let mut total_len = ImageHeader::COMMON_HEADER_LEN;
        common_data.write_all(&bytes_of(&header)[0..total_len as usize])?;

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

        let app_size = irom_data
            .clone()
            .map(|d| d.data.len() as u32)
            .unwrap_or_default()
            + flash_segment.data.len() as u32;

        Ok(Self {
            irom_data,
            flash_segment,
            app_size,
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

    fn app_size(&self) -> u32 {
        self.app_size
    }

    fn part_size(&self) -> Option<u32> {
        None
    }
}

fn merge_rom_segments<'a>(
    mut segments: impl Iterator<Item = CodeSegment<'a>>,
) -> Option<RomSegment<'a>> {
    const IROM_MAP_START: u32 = 0x4020_0000;

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

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;
    use crate::elf::ElfFirmwareImage;

    #[test]
    fn test_esp8266_image_format() {
        let input_bytes = fs::read("tests/resources/esp8266_hal_blinky").unwrap();
        let expected_bin = fs::read("tests/resources/esp8266_hal_blinky.bin").unwrap();

        let image = ElfFirmwareImage::try_from(input_bytes.as_slice()).unwrap();
        let flash_image = Esp8266Format::new(&image, FlashSettings::default()).unwrap();

        let segments = flash_image.flash_segments().collect::<Vec<_>>();
        let buf = segments[0].data.as_ref();

        assert_eq!(expected_bin.len(), buf.len());
        assert_eq!(expected_bin.as_slice(), buf);
    }
}

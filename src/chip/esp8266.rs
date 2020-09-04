use super::{ChipType, ESPCommonHeader, SegmentHeader, ESP_MAGIC};
use crate::chip::Chip;
use crate::elf::{update_checksum, FirmwareImage, RomSegment, ESP_CHECKSUM_MAGIC};
use crate::Error;
use bytemuck::bytes_of;
use std::borrow::Cow;
use std::io::Write;
use std::iter::once;
use std::mem::size_of;

pub const IROM_MAP_START: u32 = 0x40200000;
const IROM_MAP_END: u32 = 0x40300000;

pub struct ESP8266;

impl ChipType for ESP8266 {
    const DATE_REG1_VALUE: u32 = 0x00062000;
    const DATE_REG2_VALUE: u32 = 0;

    fn addr_is_flash(addr: u32) -> bool {
        addr >= IROM_MAP_START && addr < IROM_MAP_END
    }

    fn get_flash_segments<'a>(
        image: &'a FirmwareImage,
    ) -> Box<dyn Iterator<Item = Result<RomSegment<'a>, Error>> + 'a> {
        // irom goes into a separate plain bin
        let irom_data = image.rom_segments(Chip::Esp8266).map(|segment| {
            Ok(RomSegment {
                addr: segment.addr - IROM_MAP_START,
                data: Cow::Borrowed(segment.data),
            })
        });

        // my kingdom for a try {} block
        fn common<'a>(image: &'a FirmwareImage) -> Result<RomSegment<'a>, Error> {
            let mut common_data = Vec::with_capacity(
                image
                    .ram_segments(Chip::Esp8266)
                    .map(|segment| segment.size as usize)
                    .sum(),
            );
            // common header
            let header = ESPCommonHeader {
                magic: ESP_MAGIC,
                segment_count: image.ram_segments(Chip::Esp8266).count() as u8,
                flash_mode: image.flash_mode as u8,
                flash_config: image.flash_size as u8 + image.flash_frequency as u8,
                entry: image.entry,
            };
            common_data.write(bytes_of(&header))?;

            let mut total_len = 8;

            let mut checksum = ESP_CHECKSUM_MAGIC;

            for segment in image.ram_segments(Chip::Esp8266) {
                let data = segment.data;
                let padding = 4 - data.len() % 4;
                let segment_header = SegmentHeader {
                    addr: segment.addr,
                    length: (data.len() + padding) as u32,
                };
                total_len += size_of::<SegmentHeader>() as u32 + segment_header.length;
                common_data.write(bytes_of(&segment_header))?;
                common_data.write(data)?;

                let padding = &[0u8; 4][0..padding];
                common_data.write(padding)?;
                checksum = update_checksum(data, checksum);
            }

            let padding = 15 - (total_len % 16);
            let padding = &[0u8; 16][0..padding as usize];
            common_data.write(padding)?;

            common_data.write(&[checksum])?;

            Ok(RomSegment {
                addr: 0,
                data: Cow::Owned(common_data),
            })
        }

        Box::new(irom_data.chain(once(common(image))))
    }
}

#[test]
fn test_esp8266_rom() {
    use pretty_assertions::assert_eq;
    use std::fs::read;

    let input_bytes = read("./tests/data/esp8266").unwrap();
    let expected_bin = read("./tests/data/esp8266.bin").unwrap();

    let image = FirmwareImage::from_data(&input_bytes).unwrap();

    let segments = ESP8266::get_flash_segments(&image)
        .collect::<Result<Vec<_>, Error>>()
        .unwrap();

    assert_eq!(1, segments.len());
    let buff = segments[0].data.as_ref();
    assert_eq!(expected_bin.len(), buff.len());
    assert_eq!(expected_bin.as_slice(), buff);
}

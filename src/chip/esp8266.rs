use super::Chip;
use crate::elf::{update_checksum, FirmwareImage, RomSegment, ESP_CHECKSUM_MAGIC};
use crate::Error;
use bytemuck::__core::iter::once;
use bytemuck::{bytes_of, Pod, Zeroable};
use std::borrow::Cow;
use std::io::Write;
use std::mem::size_of;

const ESP8266_MAGIC: u8 = 0xe9;

#[derive(Copy, Clone, Zeroable, Pod)]
#[repr(C)]
struct ESP8266Header {
    magic: u8,
    segment_count: u8,
    flash_mode: u8,
    flash_config: u8,
    entry: u32,
}

#[derive(Copy, Clone, Zeroable, Pod)]
#[repr(C)]
struct ESP8266SegmentHeader {
    addr: u32,
    length: u32,
}

pub struct ESP8266;

impl Chip for ESP8266 {
    fn get_flash_segments<'a>(
        image: &'a FirmwareImage,
    ) -> Box<dyn Iterator<Item = Result<RomSegment<'a>, Error>> + 'a> {
        // irom goes into a separate plain bin
        let irom_data = image
            .rom_segments()
            .next()
            .map(|segment| {
                Ok(RomSegment {
                    addr: segment.addr,
                    data: Cow::Borrowed(segment.data),
                })
            })
            .into_iter();

        // my kingdom for a try {} block
        fn common<'a>(image: &'a FirmwareImage) -> Result<RomSegment<'a>, Error> {
            let mut common_data = Vec::with_capacity(
                image
                    .ram_segments()
                    .map(|segment| segment.size as usize)
                    .sum(),
            );
            // common header
            let header = ESP8266Header {
                magic: ESP8266_MAGIC,
                segment_count: image.ram_segments().count() as u8,
                flash_mode: image.flash_mode as u8,
                flash_config: image.flash_size as u8 + image.flash_frequency as u8,
                entry: image.entry,
            };
            common_data.write(bytes_of(&header))?;

            let mut total_len = 8;

            let mut checksum = ESP_CHECKSUM_MAGIC;

            for segment in image.ram_segments() {
                let data = segment.data;
                let padding = 4 - data.len() % 4;
                let segment_header = ESP8266SegmentHeader {
                    addr: segment.addr,
                    length: (data.len() + padding) as u32,
                };
                total_len += size_of::<ESP8266SegmentHeader>() as u32 + segment_header.length;
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
    use std::fs::read;

    let input_bytes = read("./tests/data/esp.elf").unwrap();
    let expected_bin = read("./tests/data/esp.bin").unwrap();

    let image = FirmwareImage::from_data(&input_bytes).unwrap();

    let segments = ESP8266::get_flash_segments(&image)
        .collect::<Result<Vec<_>, Error>>()
        .unwrap();

    assert_eq!(1, segments.len());
    let buff = segments[0].data.as_ref();
    assert_eq!(expected_bin.len(), buff.len());
    assert_eq!(expected_bin.as_slice(), buff);
}

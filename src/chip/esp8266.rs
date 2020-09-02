use super::Chip;
use crate::elf::{update_checksum, FirmwareImage, RomSegment, ESP_CHECKSUM_MAGIC};
use crate::Error;
use bytemuck::__core::iter::once;
use byteorder::{LittleEndian, WriteBytesExt};
use std::borrow::Cow;
use std::io::Write;

const ESP8266V1_MAGIC: u8 = 0xe9;

pub struct ESP8266;

impl<'a> Chip<'a> for ESP8266 {
    type Iter = std::iter::Chain<
        std::option::IntoIter<std::result::Result<RomSegment<'a>, Error>>,
        std::iter::Once<std::result::Result<RomSegment<'a>, Error>>,
    >;

    fn get_rom_segments(image: &'a FirmwareImage) -> Self::Iter {
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

            Ok(RomSegment {
                addr: 0,
                data: Cow::Owned(common_data),
            })
        }

        irom_data.chain(once(common(image)))
    }
}

use bytemuck::bytes_of;
use sha2::{Digest, Sha256};

use crate::{
    chip::{
        encode_flash_size, get_segment_padding, save_flash_segment, save_segment, Chip, ChipType,
        EspCommonHeader, ExtendedHeader, SegmentHeader, SpiRegisters, ESP_MAGIC, SEG_HEADER_LEN,
        WP_PIN_DISABLED,
    },
    elf::{FirmwareImage, RomSegment, ESP_CHECKSUM_MAGIC},
    partition_table::PartitionTable,
    Error,
};

use std::{borrow::Cow, io::Write, iter::once};

pub struct Esp32;

const IROM_MAP_START: u32 = 0x400d0000;
const IROM_MAP_END: u32 = 0x40400000;

const DROM_MAP_START: u32 = 0x3F400000;
const DROM_MAP_END: u32 = 0x3F800000;

const BOOT_ADDR: u32 = 0x1000;
const PARTION_ADDR: u32 = 0x8000;
const APP_ADDR: u32 = 0x10000;

impl ChipType for Esp32 {
    const CHIP_DETECT_MAGIC_VALUE: u32 = 0x00f01d83;

    const SPI_REGISTERS: SpiRegisters = SpiRegisters {
        base: 0x3ff42000,
        usr_offset: 0x1c,
        usr1_offset: 0x20,
        usr2_offset: 0x24,
        w0_offset: 0x80,
        mosi_length_offset: Some(0x28),
        miso_length_offset: Some(0x2c),
    };

    fn addr_is_flash(addr: u32) -> bool {
        (IROM_MAP_START..IROM_MAP_END).contains(&addr)
            || (DROM_MAP_START..DROM_MAP_END).contains(&addr)
    }

    fn get_flash_segments<'a>(
        image: &'a FirmwareImage,
    ) -> Box<dyn Iterator<Item = Result<RomSegment<'a>, Error>> + 'a> {
        let bootloader = include_bytes!("../../bootloader/esp32-bootloader.bin");

        let partition_table = PartitionTable::basic(0x10000, 0x3f0000).to_bytes();

        fn get_data<'a>(image: &'a FirmwareImage) -> Result<RomSegment<'a>, Error> {
            let mut data = Vec::new();

            let header = EspCommonHeader {
                magic: ESP_MAGIC,
                segment_count: 0,
                flash_mode: image.flash_mode as u8,
                flash_config: encode_flash_size(image.flash_size)? + image.flash_frequency as u8,
                entry: image.entry,
            };
            data.write_all(bytes_of(&header))?;

            let extended_header = ExtendedHeader {
                wp_pin: WP_PIN_DISABLED,
                clk_q_drv: 0,
                d_cs_drv: 0,
                gd_wp_drv: 0,
                chip_id: 0,
                min_rev: 0,
                padding: [0; 8],
                append_digest: 1,
            };
            data.write_all(bytes_of(&extended_header))?;

            let mut checksum = ESP_CHECKSUM_MAGIC;

            let _ = image.segments().collect::<Vec<_>>();

            let mut flash_segments: Vec<_> = image.rom_segments(Chip::Esp32).collect();
            flash_segments.sort();
            let mut ram_segments: Vec<_> = image.ram_segments(Chip::Esp32).collect();
            ram_segments.sort();
            let mut ram_segments = ram_segments.into_iter();

            let mut segment_count = 0;

            for segment in flash_segments {
                loop {
                    let pad_len = get_segment_padding(data.len(), &segment);
                    if pad_len > 0 {
                        if pad_len > SEG_HEADER_LEN {
                            if let Some(ram_segment) = ram_segments.next() {
                                checksum = save_segment(&mut data, &ram_segment, checksum)?;
                                segment_count += 1;
                                continue;
                            }
                        }
                        let pad_header = SegmentHeader {
                            addr: 0,
                            length: pad_len as u32,
                        };
                        data.write_all(bytes_of(&pad_header))?;
                        for _ in 0..pad_len {
                            data.write_all(&[0])?;
                        }
                        segment_count += 1;
                    } else {
                        break;
                    }
                }
                checksum = save_flash_segment(&mut data, &segment, checksum)?;
                segment_count += 1;
            }

            for segment in ram_segments {
                checksum = save_segment(&mut data, &segment, checksum)?;
                segment_count += 1;
            }

            let padding = 15 - (data.len() % 16);
            let padding = &[0u8; 16][0..padding as usize];
            data.write_all(padding)?;

            data.write_all(&[checksum])?;

            // since we added some dummy segments, we need to patch the segment count
            data[1] = segment_count as u8;

            let mut hasher = Sha256::new();
            hasher.update(&data);
            let hash = hasher.finalize();
            data.write_all(&hash)?;

            Ok(RomSegment {
                addr: APP_ADDR,
                data: Cow::Owned(data),
            })
        }

        Box::new(
            once(Ok(RomSegment {
                addr: BOOT_ADDR,
                data: Cow::Borrowed(bootloader),
            }))
            .chain(once(Ok(RomSegment {
                addr: PARTION_ADDR,
                data: Cow::Owned(partition_table),
            })))
            .chain(once(get_data(image))),
        )
    }
}

#[test]
fn test_esp32_rom() {
    use std::fs::read;

    let input_bytes = read("./tests/data/esp32").unwrap();
    let expected_bin = read("./tests/data/esp32.bin").unwrap();

    let image = FirmwareImage::from_data(&input_bytes).unwrap();

    let segments = Esp32::get_flash_segments(&image)
        .collect::<Result<Vec<_>, Error>>()
        .unwrap();

    assert_eq!(3, segments.len());
    let buff = segments[2].data.as_ref();
    assert_eq!(expected_bin.len(), buff.len());
    assert_eq!(&expected_bin.as_slice(), &buff);
}

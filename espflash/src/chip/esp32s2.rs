use std::borrow::Cow;
use std::io::Write;
use std::iter::once;

use crate::partition_table::PartitionTable;
use crate::chip::{Chip, ChipType, ESP_MAGIC, EspCommonHeader, ExtendedHeader, SegmentHeader, SpiRegisters, WP_PIN_DISABLED};
use crate::elf::{update_checksum, CodeSegment, FirmwareImage, RomSegment, ESP_CHECKSUM_MAGIC};
use crate::flasher::FlashSize;
use crate::Error;
use bytemuck::bytes_of;
use sha2::{Digest, Sha256};

pub struct Esp32s2;

const IROM_MAP_START: u32 = 0x40080000;
const IROM_MAP_END: u32 = 0x40b80000;

const DROM_MAP_START: u32 = 0x3F000000;
const DROM_MAP_END: u32 = 0x3F3F0000;

const BOOT_ADDR: u32 = 0x1000;
const PARTION_ADDR: u32 = 0x8000;
const APP_ADDR: u32 = 0x10000;

impl ChipType for Esp32s2 {
    const CHIP_DETECT_MAGIC_VALUE: u32 = 0x000007c6;

    const SPI_REGISTERS: SpiRegisters = SpiRegisters {
        base: 0x3f402000,
        usr_offset: 0x18,
        usr1_offset: 0x1c,
        usr2_offset: 0x20,
        w0_offset: 0x58,
        mosi_length_offset: Some(0x24),
        miso_length_offset: Some(0x28),
    };

    fn addr_is_flash(addr: u32) -> bool {
        (IROM_MAP_START..IROM_MAP_END).contains(&addr)
            || (DROM_MAP_START..DROM_MAP_END).contains(&addr)
    }

    fn get_flash_segments<'a>(
        image: &'a FirmwareImage,
    ) -> Box<dyn Iterator<Item = Result<RomSegment<'a>, Error>> + 'a> {
        let bootloader = include_bytes!("../../bootloader/esp32s2-bootloader.bin");

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

            let mut flash_segments: Vec<_> = image.rom_segments(Chip::Esp32s2).collect();
            flash_segments.sort();
            let mut ram_segments: Vec<_> = image.ram_segments(Chip::Esp32s2).collect();
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

#[test]
fn test_esp32_rom() {
    use std::fs::read;

    let input_bytes = read("./tests/data/esp32").unwrap();
    let expected_bin = read("./tests/data/esp32.bin").unwrap();

    let image = FirmwareImage::from_data(&input_bytes).unwrap();

    let segments = Esp32s2::get_flash_segments(&image)
        .collect::<Result<Vec<_>, Error>>()
        .unwrap();

    assert_eq!(3, segments.len());
    let buff = segments[2].data.as_ref();
    assert_eq!(expected_bin.len(), buff.len());
    assert_eq!(&expected_bin.as_slice(), &buff);
}

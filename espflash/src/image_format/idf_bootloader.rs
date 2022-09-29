use std::{borrow::Cow, io::Write, iter::once};

use bytemuck::{bytes_of, from_bytes, Pod, Zeroable};
use esp_idf_part::{Partition, PartitionTable, Type};
use sha2::{Digest, Sha256};

use super::{
    encode_flash_frequency, update_checksum, EspCommonHeader, ImageFormat, SegmentHeader,
    ESP_MAGIC, WP_PIN_DISABLED,
};
use crate::{
    elf::{CodeSegment, FirmwareImage, RomSegment, ESP_CHECKSUM_MAGIC},
    error::{Error, FlashDetectError},
    flasher::{FlashFrequency, FlashMode, FlashSize},
    targets::{Chip, Esp32Params},
};

const IROM_ALIGN: u32 = 0x10000;
const SEG_HEADER_LEN: u32 = 8;

#[derive(Debug, Default, Clone, Copy, Pod, Zeroable)]
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

/// Image format for ESP32 family chips using the second-stage bootloader from
/// ESP-IDF
pub struct IdfBootloaderFormat<'a> {
    params: Esp32Params,
    bootloader: Cow<'a, [u8]>,
    partition_table: PartitionTable,
    flash_segment: RomSegment<'a>,
}

impl<'a> IdfBootloaderFormat<'a> {
    pub fn new(
        image: &'a dyn FirmwareImage<'a>,
        chip: Chip,
        params: Esp32Params,
        partition_table: Option<PartitionTable>,
        bootloader: Option<Vec<u8>>,
        flash_mode: Option<FlashMode>,
        flash_size: Option<FlashSize>,
        flash_freq: Option<FlashFrequency>,
    ) -> Result<Self, Error> {
        let partition_table = partition_table
            .unwrap_or_else(|| params.default_partition_table(flash_size.map(|v| v.size())));
        let mut bootloader = if let Some(bytes) = bootloader {
            Cow::Owned(bytes)
        } else {
            Cow::Borrowed(params.default_bootloader)
        };

        let mut data = Vec::new();

        // fetch the generated header from the bootloader
        let mut header: EspCommonHeader = *from_bytes(&bootloader[0..8]);
        if header.magic != ESP_MAGIC {
            return Err(Error::InvalidBootloader);
        }

        // update the header if a user has specified any custom arguments
        if let Some(mode) = flash_mode {
            header.flash_mode = mode as u8;
            bootloader.to_mut()[2] = bytes_of(&header)[2];
        }

        match (flash_size, flash_freq) {
            (Some(s), Some(f)) => {
                header.flash_config = encode_flash_size(s)? + encode_flash_frequency(chip, f)?;
                bootloader.to_mut()[3] = bytes_of(&header)[3];
            }
            (Some(s), None) => {
                header.flash_config = encode_flash_size(s)? + (header.flash_config & 0x0F);
                bootloader.to_mut()[3] = bytes_of(&header)[3];
            }
            (None, Some(f)) => {
                header.flash_config =
                    (header.flash_config & 0xF0) + encode_flash_frequency(chip, f)?;
                bootloader.to_mut()[3] = bytes_of(&header)[3];
            }
            (None, None) => {} // nothing to update
        }

        // write the header of the app
        // use the same settings as the bootloader
        // just update the entry point
        header.entry = image.entry();
        data.write_all(bytes_of(&header))?;

        let extended_header = ExtendedHeader {
            wp_pin: WP_PIN_DISABLED,
            chip_id: params.chip_id,
            append_digest: 1,

            ..ExtendedHeader::default()
        };

        data.write_all(bytes_of(&extended_header))?;

        let flash_segments: Vec<_> = merge_adjacent_segments(image.rom_segments(chip).collect());
        let mut ram_segments: Vec<_> = merge_adjacent_segments(image.ram_segments(chip).collect());

        let mut checksum = ESP_CHECKSUM_MAGIC;
        let mut segment_count = 0;

        for segment in flash_segments {
            loop {
                let pad_len = get_segment_padding(data.len(), &segment);
                if pad_len > 0 {
                    if pad_len > SEG_HEADER_LEN {
                        if let Some(ram_segment) = ram_segments.first_mut() {
                            // save up to `pad_len` from the ram segment, any remaining bits in the
                            // ram segments will be saved later
                            let pad_segment = ram_segment.split_off(pad_len as usize);
                            checksum = save_segment(&mut data, &pad_segment, checksum)?;
                            if ram_segment.data().is_empty() {
                                ram_segments.remove(0);
                            }
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

            checksum = save_flash_segment(&mut data, segment, checksum)?;
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

        // The default partition table contains the "factory" partition, and if a user
        // provides a partition table via command-line then the validation step confirms
        // that at least one "app" partition is present. We prefer the "factory"
        // partition, and use any available "app" partitions if not present.
        let factory_partition = partition_table
            .find("factory")
            .or_else(|| partition_table.find_by_type(Type::App))
            .unwrap();

        check_partition_stats(factory_partition, &data)?;

        let flash_segment = RomSegment {
            addr: factory_partition.offset(),
            data: Cow::Owned(data),
        };

        Ok(Self {
            params,
            bootloader,
            partition_table,
            flash_segment,
        })
    }
}

impl<'a> ImageFormat<'a> for IdfBootloaderFormat<'a> {
    fn flash_segments<'b>(&'b self) -> Box<dyn Iterator<Item = RomSegment<'b>> + 'b>
    where
        'a: 'b,
    {
        Box::new(
            once(RomSegment {
                addr: self.params.boot_addr,
                data: Cow::Borrowed(&self.bootloader),
            })
            .chain(once(RomSegment {
                addr: self.params.partition_addr,
                data: Cow::Owned(self.partition_table.to_bin().unwrap()),
            }))
            .chain(once(self.flash_segment.borrow())),
        )
    }

    fn ota_segments<'b>(&'b self) -> Box<dyn Iterator<Item = RomSegment<'b>> + 'b>
    where
        'a: 'b,
    {
        Box::new(once(self.flash_segment.borrow()))
    }
}

fn encode_flash_size(size: FlashSize) -> Result<u8, FlashDetectError> {
    use FlashSize::*;

    match size {
        Flash1Mb => Ok(0x00),
        Flash2Mb => Ok(0x10),
        Flash4Mb => Ok(0x20),
        Flash8Mb => Ok(0x30),
        Flash16Mb => Ok(0x40),
        Flash32Mb => Ok(0x19),
        Flash64Mb => Ok(0x1a),
        Flash128Mb => Ok(0x21),
        _ => Err(FlashDetectError::from(size as u8)),
    }
}

fn check_partition_stats(part: &Partition, data: &Vec<u8>) -> Result<(), Error> {
    let perc = data.len() as f32 / part.size() as f32 * 100.0;
    println!(
        "App/part. size:    {}/{} bytes, {:.2}%",
        data.len(),
        part.size(),
        perc
    );

    if perc > 100.0 {
        return Err(Error::ElfTooBig);
    }

    Ok(())
}

/// Actual alignment (in data bytes) required for a segment header: positioned
/// so that after we write the next 8 byte header, file_offs % IROM_ALIGN ==
/// segment.addr % IROM_ALIGN
///
/// (this is because the segment's vaddr may not be IROM_ALIGNed, more likely is
/// aligned IROM_ALIGN+0x18 to account for the binary file header
fn get_segment_padding(offset: usize, segment: &CodeSegment) -> u32 {
    let align_past = (segment.addr - SEG_HEADER_LEN) % IROM_ALIGN;
    let pad_len = ((IROM_ALIGN - ((offset as u32) % IROM_ALIGN)) + align_past) % IROM_ALIGN;

    if pad_len == 0 || pad_len == IROM_ALIGN {
        0
    } else if pad_len > SEG_HEADER_LEN {
        pad_len - SEG_HEADER_LEN
    } else {
        pad_len + IROM_ALIGN - SEG_HEADER_LEN
    }
}

fn merge_adjacent_segments(mut segments: Vec<CodeSegment>) -> Vec<CodeSegment> {
    segments.sort();

    let mut merged: Vec<CodeSegment> = Vec::with_capacity(segments.len());
    for segment in segments {
        match merged.last_mut() {
            Some(last) if last.addr + last.size() == segment.addr => {
                *last += segment.data();
            }
            _ => {
                merged.push(segment);
            }
        }
    }

    merged
}

fn save_flash_segment(
    data: &mut Vec<u8>,
    mut segment: CodeSegment,
    checksum: u8,
) -> Result<u8, Error> {
    let end_pos = (data.len() + segment.data().len()) as u32 + SEG_HEADER_LEN;
    let segment_reminder = end_pos % IROM_ALIGN;

    if segment_reminder < 0x24 {
        // Work around a bug in ESP-IDF 2nd stage bootloader, that it didn't map the
        // last MMU page, if an IROM/DROM segment was < 0x24 bytes over the page
        // boundary.
        static PADDING: [u8; 0x24] = [0; 0x24];

        segment += &PADDING[0..(0x24 - segment_reminder as usize)];
    }

    let checksum = save_segment(data, &segment, checksum)?;

    Ok(checksum)
}

fn save_segment(data: &mut Vec<u8>, segment: &CodeSegment, checksum: u8) -> Result<u8, Error> {
    let padding = (4 - segment.size() % 4) % 4;
    let header = SegmentHeader {
        addr: segment.addr,
        length: segment.size() + padding,
    };

    data.write_all(bytes_of(&header))?;
    data.write_all(segment.data())?;

    let padding = &[0u8; 4][0..padding as usize];
    data.write_all(padding)?;

    Ok(update_checksum(segment.data(), checksum))
}

use std::{borrow::Cow, io::Write, iter::once, mem::size_of};

use bytemuck::{bytes_of, from_bytes};
use esp_idf_part::{Partition, PartitionTable, Type};
use sha2::{Digest, Sha256};

use crate::{
    elf::{CodeSegment, FirmwareImage, RomSegment},
    error::Error,
    flasher::FlashSettings,
    image_format::{
        update_checksum, ImageFormat, ImageHeader, SegmentHeader, ESP_CHECKSUM_MAGIC, ESP_MAGIC,
        WP_PIN_DISABLED,
    },
    targets::{Chip, Esp32Params},
};

const IROM_ALIGN: u32 = 0x10000;
const SEG_HEADER_LEN: u32 = 8;

/// Image format for ESP32 family chips using the second-stage bootloader from
/// ESP-IDF
pub struct IdfBootloaderFormat<'a> {
    params: Esp32Params,
    bootloader: Cow<'a, [u8]>,
    partition_table: PartitionTable,
    flash_segment: RomSegment<'a>,
    app_size: u32,
    part_size: u32,
    partition_table_offset: u32,
}

impl<'a> IdfBootloaderFormat<'a> {
    pub fn new(
        image: &'a dyn FirmwareImage<'a>,
        chip: Chip,
        min_rev_full: u16,
        params: Esp32Params,
        partition_table: Option<PartitionTable>,
        partition_table_offset: Option<u32>,
        target_app_partition: Option<String>,
        bootloader: Option<Vec<u8>>,
        flash_settings: FlashSettings,
    ) -> Result<Self, Error> {
        let partition_table = partition_table.unwrap_or_else(|| {
            params.default_partition_table(flash_settings.size.map(|v| v.size()))
        });
        let mut bootloader = if let Some(bytes) = bootloader {
            Cow::Owned(bytes)
        } else {
            Cow::Borrowed(params.default_bootloader)
        };

        // fetch the generated header from the bootloader
        let mut header: ImageHeader = *from_bytes(&bootloader[0..size_of::<ImageHeader>()]);
        if header.magic != ESP_MAGIC {
            return Err(Error::InvalidBootloader);
        }

        // update the header if a user has specified any custom arguments
        if let Some(mode) = flash_settings.mode {
            header.flash_mode = mode as u8;
        }

        header.write_flash_config(
            flash_settings.size.unwrap_or_default(),
            flash_settings.freq.unwrap_or(params.flash_freq),
            chip,
        )?;

        bootloader.to_mut().splice(
            0..size_of::<ImageHeader>(),
            bytes_of(&header).iter().copied(),
        );

        // write the header of the app
        // use the same settings as the bootloader
        // just update the entry point
        header.entry = image.entry();

        header.wp_pin = WP_PIN_DISABLED;
        header.chip_id = params.chip_id;
        header.min_chip_rev_full = min_rev_full;
        header.append_digest = 1;

        let mut data = bytes_of(&header).to_vec();

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
                        length: pad_len,
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
        let padding = &[0u8; 16][0..padding];
        data.write_all(padding)?;

        data.write_all(&[checksum])?;

        // since we added some dummy segments, we need to patch the segment count
        data[1] = segment_count as u8;

        let mut hasher = Sha256::new();
        hasher.update(&data);
        let hash = hasher.finalize();
        data.write_all(&hash)?;

        let target_app_partition: &Partition =
        // Use the target app partition if provided
        if let Some(target_partition) = target_app_partition {
            partition_table
                .find(&target_partition)
                .ok_or(Error::AppPartitionNotFound)?
        } else {

            // The default partition table contains the "factory" partition, and if a user
            // provides a partition table via command-line then the validation step confirms
            // that at least one "app" partition is present. We prefer the "factory"
            // partition, and use any available "app" partitions if not present.

            partition_table
                .find("factory")
                .or_else(|| partition_table.find_by_type(Type::App))
                .ok_or(Error::AppPartitionNotFound)?
        };

        let app_size = data.len() as u32;
        let part_size = target_app_partition.size();

        // The size of the application must not exceed the size of the factory
        // partition.
        if app_size as f32 / part_size as f32 > 1.0 {
            return Err(Error::ElfTooBig(app_size, part_size));
        }

        let flash_segment = RomSegment {
            addr: target_app_partition.offset(),
            data: Cow::Owned(data),
        };

        // If the user did not specify a partition offset, we need to assume that the partition
        // offset is (first partition offset) - 0x1000, since this is the most common case.
        let partition_table_offset = partition_table_offset.unwrap_or_else(|| {
            let partitions = partition_table.partitions();
            let first_partition = partitions
                .iter()
                .min_by(|a, b| a.offset().cmp(&b.offset()))
                .unwrap();
            first_partition.offset() - 0x1000
        });

        Ok(Self {
            params,
            bootloader,
            partition_table,
            flash_segment,
            app_size,
            part_size,
            partition_table_offset,
        })
    }
}

impl<'a> ImageFormat<'a> for IdfBootloaderFormat<'a> {
    fn flash_segments<'b>(&'b self) -> Box<dyn Iterator<Item = RomSegment<'b>> + 'b>
    where
        'a: 'b,
    {
        let bootloader_segment = RomSegment {
            addr: self.params.boot_addr,
            data: Cow::Borrowed(&self.bootloader),
        };

        let partition_table_segment = RomSegment {
            addr: self.partition_table_offset,
            data: Cow::Owned(self.partition_table.to_bin().unwrap()),
        };

        let app_partition = self
            .partition_table
            .find("factory")
            .or_else(|| self.partition_table.find_by_type(Type::App))
            .expect("no application partition found");

        if self.flash_segment.data.len() > app_partition.size() as usize {
            panic!(
                "image size ({} bytes) is larger partition size ({} bytes)",
                self.flash_segment.data.len(),
                app_partition.size()
            );
        }

        let app_segment = RomSegment {
            addr: app_partition.offset(),
            data: Cow::Borrowed(&self.flash_segment.data),
        };

        Box::new(
            once(bootloader_segment)
                .chain(once(partition_table_segment))
                .chain(once(app_segment)),
        )
    }

    fn ota_segments<'b>(&'b self) -> Box<dyn Iterator<Item = RomSegment<'b>> + 'b>
    where
        'a: 'b,
    {
        Box::new(once(self.flash_segment.borrow()))
    }

    fn app_size(&self) -> u32 {
        self.app_size
    }

    fn part_size(&self) -> Option<u32> {
        Some(self.part_size)
    }
}

/// Actual alignment (in data bytes) required for a segment header: positioned
/// so that after we write the next 8 byte header, file_offset % IROM_ALIGN ==
/// segment.addr % IROM_ALIGN
///
/// (this is because the segment's vaddr may not be IROM_ALIGNed, more likely is
/// aligned IROM_ALIGN+0x18 to account for the binary file header)
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

/// Merge adjacent segments into one.
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

/// Save a segment to the data buffer.
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

/// Stores a segment header and the segment data in the data buffer.
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

#[cfg(test)]
pub mod tests {
    use std::fs;

    use super::*;
    use crate::{elf::ElfFirmwareImage, image_format::FlashFrequency};

    // Copied from: src/targets/esp32.rs
    const PARAMS: Esp32Params = Esp32Params::new(
        0x1000,
        0x1_0000,
        0x3f_0000,
        0,
        FlashFrequency::_40Mhz,
        include_bytes!("../../resources/bootloaders/esp32-bootloader.bin"),
    );

    #[test]
    fn test_idf_bootloader_format() {
        let input_bytes = fs::read("tests/resources/esp32_hal_blinky").unwrap();
        let expected_bin = fs::read("tests/resources/esp32_hal_blinky.bin").unwrap();

        let image = ElfFirmwareImage::try_from(input_bytes.as_slice()).unwrap();
        let flash_image = IdfBootloaderFormat::new(
            &image,
            Chip::Esp32,
            0,
            PARAMS,
            None,
            None,
            None,
            None,
            FlashSettings::default(),
        )
        .unwrap();

        let segments = flash_image.flash_segments().collect::<Vec<_>>();
        assert_eq!(segments.len(), 3);

        let buf = segments[2].data.as_ref();
        assert_eq!(expected_bin.len(), buf.len());
        assert_eq!(expected_bin.as_slice(), buf);
    }
}

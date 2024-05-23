//! ESP-IDF application binary image format

use std::{borrow::Cow, io::Write, iter::once, mem::size_of};

use bytemuck::{bytes_of, from_bytes, Pod, Zeroable};
use esp_idf_part::{Partition, PartitionTable, Type};
use sha2::{Digest, Sha256};

use crate::{
    elf::{CodeSegment, FirmwareImage, RomSegment},
    error::Error,
    flasher::{FlashFrequency, FlashMode, FlashSettings, FlashSize},
    targets::{Chip, Esp32Params},
};

const ESP_CHECKSUM_MAGIC: u8 = 0xef;
const ESP_MAGIC: u8 = 0xE9;
const IROM_ALIGN: u32 = 0x10000;
const SEG_HEADER_LEN: u32 = 8;
const WP_PIN_DISABLED: u8 = 0xEE;

/// Firmware header used by the ESP-IDF bootloader.
///
/// ## Header documentation:
/// * [Header](https://docs.espressif.com/projects/esptool/en/latest/esp32c3/advanced-topics/firmware-image-format.html#file-header)
/// * [Extended header](https://docs.espressif.com/projects/esptool/en/latest/esp32c3/advanced-topics/firmware-image-format.html#extended-file-header)
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
#[repr(C, packed)]
#[doc(alias = "esp_image_header_t")]
struct ImageHeader {
    magic: u8,
    segment_count: u8,
    /// Flash read mode (esp_image_spi_mode_t)
    flash_mode: u8,
    /// ..4 bits are flash chip size (esp_image_flash_size_t)
    /// 4.. bits are flash frequency (esp_image_spi_freq_t)
    #[doc(alias = "spi_size")]
    #[doc(alias = "spi_speed")]
    flash_config: u8,
    entry: u32,

    // extended header part
    wp_pin: u8,
    clk_q_drv: u8,
    d_cs_drv: u8,
    gd_wp_drv: u8,
    chip_id: u16,
    min_rev: u8,
    /// Minimum chip revision supported by image, in format: major * 100 + minor
    min_chip_rev_full: u16,
    /// Maximal chip revision supported by image, in format: major * 100 + minor
    max_chip_rev_full: u16,
    reserved: [u8; 4],
    append_digest: u8,
}

impl Default for ImageHeader {
    fn default() -> Self {
        Self {
            magic: ESP_MAGIC,
            segment_count: 3,
            flash_mode: FlashMode::default() as _,
            flash_config: ((FlashSize::default() as u8) << 4) | FlashFrequency::default() as u8,
            entry: 0,
            wp_pin: WP_PIN_DISABLED,
            clk_q_drv: 0,
            d_cs_drv: 0,
            gd_wp_drv: 0,
            chip_id: Default::default(),
            min_rev: 0,
            min_chip_rev_full: 0,
            max_chip_rev_full: u16::MAX,
            reserved: Default::default(),
            append_digest: 1,
        }
    }
}

impl ImageHeader {
    /// Updates flash size and speed filed.
    pub fn write_flash_config(
        &mut self,
        size: FlashSize,
        freq: FlashFrequency,
        chip: Chip,
    ) -> Result<(), Error> {
        let flash_size = size.encode_flash_size()?;
        let flash_speed = freq.encode_flash_frequency(chip)?;

        // bit field
        self.flash_config = (flash_size << 4) | flash_speed;
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, Pod, Zeroable)]
#[repr(C, packed)]
struct SegmentHeader {
    addr: u32,
    length: u32,
}

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

        // re-calculate hash of the bootloader - needed since we modified the header
        let bootloader_len = bootloader.len();
        let mut hasher = Sha256::new();
        hasher.update(&bootloader[..bootloader_len - 32]);
        let hash = hasher.finalize();
        bootloader.to_mut()[bootloader_len - 32..].copy_from_slice(&hash);

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

        // The size of the application must not exceed the size of the target app
        // partition.
        if app_size as f32 / part_size as f32 > 1.0 {
            return Err(Error::ElfTooBig(app_size, part_size));
        }

        let flash_segment = RomSegment {
            addr: target_app_partition.offset(),
            data: Cow::Owned(data),
        };

        // If the user did not specify a partition offset, we need to assume that the
        // partition offset is (first partition offset) - 0x1000, since this is
        // the most common case.
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

    pub fn flash_segments<'b>(&'b self) -> Box<dyn Iterator<Item = RomSegment<'b>> + 'b>
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

        let app_segment = RomSegment {
            addr: self.flash_segment.addr,
            data: Cow::Borrowed(&self.flash_segment.data),
        };

        Box::new(
            once(bootloader_segment)
                .chain(once(partition_table_segment))
                .chain(once(app_segment)),
        )
    }

    pub fn ota_segments<'b>(&'b self) -> Box<dyn Iterator<Item = RomSegment<'b>> + 'b>
    where
        'a: 'b,
    {
        Box::new(once(self.flash_segment.borrow()))
    }

    pub fn app_size(&self) -> u32 {
        self.app_size
    }

    pub fn part_size(&self) -> Option<u32> {
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

/// Update the checksum with the given data
fn update_checksum(data: &[u8], mut checksum: u8) -> u8 {
    for byte in data {
        checksum ^= *byte;
    }

    checksum
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_flash_config_write() {
        let mut header = ImageHeader::default();
        header
            .write_flash_config(FlashSize::_4Mb, FlashFrequency::_40Mhz, Chip::Esp32c3)
            .unwrap();
        assert_eq!(header.flash_config, 0x20);

        header
            .write_flash_config(FlashSize::_32Mb, FlashFrequency::_80Mhz, Chip::Esp32s3)
            .unwrap();
        assert_eq!(header.flash_config, 0x5F);
    }
}

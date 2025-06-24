//! ESP-IDF application binary image format

use std::{
    borrow::Cow,
    collections::HashMap,
    ffi::c_char,
    fs,
    io::Write,
    iter::once,
    mem::size_of,
    path::Path,
};

use bytemuck::{Pod, Zeroable, bytes_of, from_bytes, pod_read_unaligned};
use esp_idf_part::{AppType, DataType, Flags, Partition, PartitionTable, SubType, Type};
use log::warn;
use miette::{IntoDiagnostic, Result};
use object::{
    Endianness,
    File,
    Object,
    ObjectSection,
    ObjectSymbol,
    read::elf::ElfFile32 as ElfFile,
};
use sha2::{Digest, Sha256};

use super::{Segment, ram_segments, rom_segments};
use crate::{
    Error,
    error::AppDescriptorError,
    flasher::{FlashData, FlashFrequency, FlashMode, FlashSize},
    target::{Chip, XtalFrequency},
};

const ESP_CHECKSUM_MAGIC: u8 = 0xEF;
const ESP_MAGIC: u8 = 0xE9;
const IROM_ALIGN: u32 = 0x10000;
const SEG_HEADER_LEN: u32 = 8;
const WP_PIN_DISABLED: u8 = 0xEE;

/// Max partition size is 16 MB
const MAX_PARTITION_SIZE: u32 = 16 * 1000 * 1024;

const BOOTLOADER_ESP32_26MHZ: &[u8] =
    include_bytes!("../../resources/bootloaders/esp32_26-bootloader.bin");
const BOOTLOADER_ESP32_40MHZ: &[u8] =
    include_bytes!("../../resources/bootloaders/esp32-bootloader.bin");

const BOOTLOADER_ESP32C2_26MHZ: &[u8] =
    include_bytes!("../../resources/bootloaders/esp32c2_26-bootloader.bin");
const BOOTLOADER_ESP32C2_40MHZ: &[u8] =
    include_bytes!("../../resources/bootloaders/esp32c2-bootloader.bin");

const BOOTLOADER_ESP32C3: &[u8] =
    include_bytes!("../../resources/bootloaders/esp32c3-bootloader.bin");
const BOOTLOADER_ESP32C5: &[u8] =
    include_bytes!("../../resources/bootloaders/esp32c5-bootloader.bin");
const BOOTLOADER_ESP32C6: &[u8] =
    include_bytes!("../../resources/bootloaders/esp32c6-bootloader.bin");
const BOOTLOADER_ESP32H2: &[u8] =
    include_bytes!("../../resources/bootloaders/esp32h2-bootloader.bin");
const BOOTLOADER_ESP32P4: &[u8] =
    include_bytes!("../../resources/bootloaders/esp32p4-bootloader.bin");
const BOOTLOADER_ESP32S2: &[u8] =
    include_bytes!("../../resources/bootloaders/esp32s2-bootloader.bin");
const BOOTLOADER_ESP32S3: &[u8] =
    include_bytes!("../../resources/bootloaders/esp32s3-bootloader.bin");

/// Get the default bootloader for the given chip and crystal frequency
pub(crate) fn default_bootloader(
    chip: Chip,
    xtal_freq: XtalFrequency,
) -> Result<&'static [u8], Error> {
    let error = Error::UnsupportedFeature {
        chip,
        feature: "the selected crystal frequency".into(),
    };

    match chip {
        Chip::Esp32 => match xtal_freq {
            XtalFrequency::_26Mhz => Ok(BOOTLOADER_ESP32_26MHZ),
            XtalFrequency::_40Mhz => Ok(BOOTLOADER_ESP32_40MHZ),
            _ => Err(error),
        },
        Chip::Esp32c2 => match xtal_freq {
            XtalFrequency::_26Mhz => Ok(BOOTLOADER_ESP32C2_26MHZ),
            XtalFrequency::_40Mhz => Ok(BOOTLOADER_ESP32C2_40MHZ),
            _ => Err(error),
        },
        Chip::Esp32c3 => match xtal_freq {
            XtalFrequency::_40Mhz => Ok(BOOTLOADER_ESP32C3),
            _ => Err(error),
        },
        Chip::Esp32c5 => match xtal_freq {
            XtalFrequency::_40Mhz | XtalFrequency::_48Mhz => Ok(BOOTLOADER_ESP32C5),
            _ => Err(error),
        },
        Chip::Esp32c6 => match xtal_freq {
            XtalFrequency::_40Mhz => Ok(BOOTLOADER_ESP32C6),
            _ => Err(error),
        },
        Chip::Esp32h2 => match xtal_freq {
            XtalFrequency::_32Mhz => Ok(BOOTLOADER_ESP32H2),
            _ => Err(error),
        },
        Chip::Esp32p4 => match xtal_freq {
            XtalFrequency::_40Mhz => Ok(BOOTLOADER_ESP32P4),
            _ => Err(error),
        },
        Chip::Esp32s2 => match xtal_freq {
            XtalFrequency::_40Mhz => Ok(BOOTLOADER_ESP32S2),
            _ => Err(error),
        },
        Chip::Esp32s3 => match xtal_freq {
            XtalFrequency::_40Mhz => Ok(BOOTLOADER_ESP32S3),
            _ => Err(error),
        },
    }
}

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
    /// Updates flash size and baud filed.
    pub fn write_flash_config(
        &mut self,
        size: FlashSize,
        freq: FlashFrequency,
        chip: Chip,
    ) -> Result<(), Error> {
        let flash_size = size.encode_flash_size()?;
        let flash_baud = freq.encode_flash_frequency(chip)?;

        // bit field
        self.flash_config = (flash_size << 4) | flash_baud;
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, Pod, Zeroable)]
#[repr(C, packed)]
struct SegmentHeader {
    addr: u32,
    length: u32,
}

/// Application descriptor used by the ESP-IDF bootloader.
///
/// [Documentation](https://docs.espressif.com/projects/esp-idf/en/stable/esp32/api-reference/system/app_image_format.html#application-description)
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
#[repr(C, packed)]
#[doc(alias = "esp_app_desc_t")]
struct AppDescriptor {
    /// Magic word ESP_APP_DESC_MAGIC_WORD
    magic_word: u32,
    /// Secure version
    secure_version: u32,
    reserv1: [u32; 2],
    /// Application version
    version: [c_char; 32],
    /// Project name
    project_name: [c_char; 32],
    /// Compile time
    time: [c_char; 16],
    /// Compile date
    date: [c_char; 16],
    /// Version IDF
    idf_ver: [c_char; 32],
    /// sha256 of elf file
    app_elf_sha256: [u8; 32],
    /// Minimal eFuse block revision supported by image,
    /// in format: major * 100 + minor
    min_efuse_blk_rev_full: u16,
    /// Maximal eFuse block revision supported by image,
    /// in format: major * 100 + minor
    max_efuse_blk_rev_full: u16,
    /// MMU page size in log base 2 format
    mmu_page_size: u8,
    reserv3: [u8; 3],
    reserv2: [u32; 18],
}

impl AppDescriptor {
    const ESP_APP_DESC_MAGIC_WORD: u32 = 0xABCD5432;
}

/// Image format for ESP32 family chips using the second-stage bootloader from
/// ESP-IDF
#[derive(Debug)]
pub struct IdfBootloaderFormat<'a> {
    boot_addr: u32,
    bootloader: Cow<'a, [u8]>,
    partition_table: PartitionTable,
    flash_segment: Segment<'a>,
    app_size: u32,
    partition_table_size: u32,
    partition_table_offset: u32,
}

impl<'a> IdfBootloaderFormat<'a> {
    /// Create a new [`IdfBootloaderFormat`].
    pub fn new(
        elf_data: &'a [u8],
        flash_data: &FlashData,
        partition_table_path: Option<&Path>,
        bootloader_path: Option<&Path>,
        partition_table_offset: Option<u32>,
        target_app_partition: Option<&str>,
    ) -> Result<Self, Error> {
        let elf = ElfFile::parse(elf_data)?;

        let partition_table = if let Some(partition_table_path) = partition_table_path {
            parse_partition_table(partition_table_path)?
        } else {
            default_partition_table(
                flash_data.chip,
                flash_data.flash_settings.size.map(|v| v.size()),
            )
        };

        if partition_table
            .partitions()
            .iter()
            .map(|p| p.size())
            .sum::<u32>()
            > flash_data.flash_settings.size.unwrap_or_default().size()
        {
            return Err(Error::PartitionTableDoesNotFit(
                flash_data.flash_settings.size.unwrap_or_default(),
            ));
        }

        let mut bootloader = if let Some(bootloader_path) = bootloader_path {
            let bootloader = fs::read(bootloader_path)?;
            Cow::Owned(bootloader)
        } else {
            let default_bootloader = default_bootloader(flash_data.chip, flash_data.xtal_freq)?;
            Cow::Borrowed(default_bootloader)
        };

        // fetch the generated header from the bootloader
        let mut calc_bootloader_size = 0;
        let bootloader_header_size = size_of::<ImageHeader>();
        calc_bootloader_size += bootloader_header_size;
        let mut header: ImageHeader = *from_bytes(&bootloader[0..bootloader_header_size]);
        if header.magic != ESP_MAGIC {
            return Err(Error::InvalidBootloader);
        }

        for _ in 0..header.segment_count {
            let segment: SegmentHeader = *from_bytes(
                &bootloader
                    [calc_bootloader_size..calc_bootloader_size + size_of::<SegmentHeader>()],
            );
            calc_bootloader_size += segment.length as usize + size_of::<SegmentHeader>();
        }

        // update the header if a user has specified any custom arguments
        if let Some(mode) = flash_data.flash_settings.mode {
            header.flash_mode = mode as u8;
        }

        header.write_flash_config(
            flash_data.flash_settings.size.unwrap_or_default(),
            flash_data
                .flash_settings
                .freq
                .unwrap_or(flash_data.chip.default_flash_frequency()),
            flash_data.chip,
        )?;

        bootloader.to_mut().splice(
            0..size_of::<ImageHeader>(),
            bytes_of(&header).iter().copied(),
        );

        // The header was modified so we need to recalculate the hash of the
        // bootloader. The hash is at the end of the bootloader segments and
        // 1-byte checksum at the end of a 16-byte padded boundary.
        //
        // Source: Point 3 of <https://docs.espressif.com/projects/esp-idf/en/v5.4/esp32c3/api-reference/system/app_image_format.html>
        calc_bootloader_size += 1; // add checksum size
        calc_bootloader_size = calc_bootloader_size + ((16 - (calc_bootloader_size % 16)) % 16);
        let bootloader_sha_start = calc_bootloader_size;
        calc_bootloader_size += 32; // add sha256 size
        let bootloader_sha_end = calc_bootloader_size;

        let mut hasher = Sha256::new();
        hasher.update(&bootloader[..bootloader_sha_start]);
        let hash = hasher.finalize();
        log::debug!(
            "Updating bootloader SHA256 from {} to {}",
            encode_hex(&bootloader[bootloader_sha_start..bootloader_sha_end]),
            encode_hex(hash)
        );
        bootloader.to_mut()[bootloader_sha_start..bootloader_sha_end].copy_from_slice(&hash);

        // write the header of the app
        // use the same settings as the bootloader
        // just update the entry point
        header.entry = elf.elf_header().e_entry.get(Endianness::Little);
        header.wp_pin = WP_PIN_DISABLED;
        header.chip_id = flash_data.chip.id();
        header.min_chip_rev_full = flash_data.min_chip_rev;
        header.append_digest = 1;

        let mut data = bytes_of(&header).to_vec();

        // The bootloader needs segments to be 4-byte aligned, but ensuring that
        // alignment by padding segments might result in overlapping segments. We
        // need to merge adjacent segments first to avoid the possibility of them
        // overlapping, and then do the padding.
        let mut flash_segments: Vec<_> = pad_align_segments(merge_adjacent_segments(
            rom_segments(flash_data.chip, &elf).collect(),
        ));
        let mut ram_segments: Vec<_> = pad_align_segments(merge_adjacent_segments(
            ram_segments(flash_data.chip, &elf).collect(),
        ));

        let mut checksum = ESP_CHECKSUM_MAGIC;
        let mut segment_count = 0;

        // Find and bubble the app descriptor segment to the first position. We do this
        // after merging/padding the segments, so it should be okay to reorder them now.
        let app_desc_addr = if let Some(appdesc) = elf.section_by_name(".flash.appdesc") {
            let address = appdesc.address() as u32;
            let Some(segment_position) = flash_segments
                .iter_mut()
                .position(|s| s.addr <= address && s.addr + s.size() > address)
            else {
                unreachable!("appdesc segment not found");
            };

            // We need to place the segment to the first position
            flash_segments[0..=segment_position].rotate_right(1);
            Some(address)
        } else {
            None
        };

        let valid_page_sizes = flash_data
            .chip
            .valid_mmu_page_sizes()
            .unwrap_or(&[IROM_ALIGN]);
        let valid_page_sizes_string = valid_page_sizes
            .iter()
            .map(|size| format!("{:#x}", size))
            .collect::<Vec<_>>()
            .join(", ");
        let app_desc_mmu_page_size = if let Some(address) = app_desc_addr {
            let segment = &flash_segments[0];

            let offset = (address - segment.addr) as usize;
            let app_descriptor_size = size_of::<AppDescriptor>();

            let segment_data = segment.data();
            let app_descriptor_bytes = &segment_data[offset..][..app_descriptor_size];
            let app_descriptor: AppDescriptor = pod_read_unaligned(app_descriptor_bytes);

            if app_descriptor.magic_word != AppDescriptor::ESP_APP_DESC_MAGIC_WORD {
                return Err(
                    AppDescriptorError::MagicWordMismatch(app_descriptor.magic_word).into(),
                );
            }

            if app_descriptor.mmu_page_size != 0 {
                // Read page size from the app descriptor
                Some(1 << app_descriptor.mmu_page_size)
            } else {
                // Infer from the app descriptor alignment

                // Subtract image + extended header (24 bytes) and segment header (8 bytes)
                let address = address - 32;

                // Page sizes are defined in ascenting order
                let mut page_size = None;
                for size in valid_page_sizes.iter().rev().copied() {
                    if address % size == 0 {
                        page_size = Some(size);
                        break;
                    }
                }

                if page_size.is_none() {
                    warn!(
                        "The app descriptor is placed at {:#x} which is not aligned to any of the \
                        supported page sizes: {}",
                        address, valid_page_sizes_string
                    );
                    return Err(AppDescriptorError::IncorrectDescriptorAlignment.into());
                }

                page_size
            }
        } else {
            None
        };

        // Precedence is:
        // - user input (unimplemented)
        // - app descriptor
        // - value based on app descriptor alignment
        // - default value
        let mmu_page_size = flash_data
            .mmu_page_size
            .or(app_desc_mmu_page_size)
            .unwrap_or(IROM_ALIGN);

        if !valid_page_sizes.contains(&mmu_page_size) {
            warn!(
                "MMU page size {:#x} is not supported. Supported page sizes are: {}",
                mmu_page_size, valid_page_sizes_string
            );
            return Err(AppDescriptorError::IncorrectDescriptorAlignment.into());
        };

        for segment in flash_segments {
            loop {
                let pad_len = segment_padding(data.len(), &segment, mmu_page_size);
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

            checksum = save_flash_segment(&mut data, segment, checksum, mmu_page_size)?;
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

        let target_app_partition: Partition =
        // Use the target app partition if provided
        if let Some(target_partition) = target_app_partition {
            partition_table
                .find(target_partition)
                .ok_or(Error::AppPartitionNotFound)?
                .clone()
        } else {
            // The default partition table contains the "factory" partition, and if a user
            // provides a partition table via command-line then the validation step confirms
            // that at least one "app" partition is present. We prefer the "factory"
            // partition, and use any available "app" partitions if not present.
            partition_table
                .find("factory")
                .or_else(|| partition_table.find_by_type(Type::App))
                .ok_or(Error::AppPartitionNotFound)?
                .clone()
        };

        let app_size = data.len() as u32;
        let partition_table_size = target_app_partition.size();

        // The size of the application must not exceed the size of the target app
        // partition.
        if app_size as f32 / partition_table_size as f32 > 1.0 {
            return Err(Error::ElfTooBig(app_size, partition_table_size));
        }

        let flash_segment = Segment {
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

        let boot_addr = flash_data.chip.boot_address();

        Ok(Self {
            boot_addr,
            bootloader,
            partition_table,
            flash_segment,
            app_size,
            partition_table_size,
            partition_table_offset,
        })
    }

    /// Returns an iterator over the [Segment]'s that should be placed in flash.
    pub fn flash_segments<'b>(self) -> impl Iterator<Item = Segment<'b>>
    where
        'a: 'b,
    {
        let bootloader_segment = Segment {
            addr: self.boot_addr,
            data: self.bootloader,
        };

        let partition_table_segment = Segment {
            addr: self.partition_table_offset,
            data: Cow::Owned(self.partition_table.to_bin().unwrap()),
        };

        let app_segment = Segment {
            addr: self.flash_segment.addr,
            data: self.flash_segment.data,
        };

        once(bootloader_segment)
            .chain(once(partition_table_segment))
            .chain(once(app_segment))
    }

    /// Returns an iterator over the OTA segment.
    pub fn ota_segments<'b>(self) -> impl Iterator<Item = Segment<'b>>
    where
        'a: 'b,
    {
        once(self.flash_segment)
    }

    /// Returns a map of metadata about the application image.
    pub fn metadata(&self) -> HashMap<&str, String> {
        HashMap::from([
            ("app_size", self.app_size.to_string()),
            ("part_size", self.partition_table_size.to_string()),
        ])
    }

    /// Returns the partition table.
    pub fn partition_table(&self) -> PartitionTable {
        self.partition_table.clone()
    }
}

/// Generates a default partition table.
///
/// `flash_size` is used to scale app partition when present, otherwise the
/// parameter defaults are used.
fn default_partition_table(chip: Chip, flash_size: Option<u32>) -> PartitionTable {
    const NVS_ADDR: u32 = 0x9000;
    const NVS_SIZE: u32 = 0x6000;
    const PHY_INIT_DATA_ADDR: u32 = 0xf000;
    const PHY_INIT_DATA_SIZE: u32 = 0x1000;

    let (app_addr, app_size) = match chip {
        Chip::Esp32 => (0x1_0000, 0x3f_0000),
        Chip::Esp32c2 => (0x1_0000, 0x1f_0000),
        Chip::Esp32c3 => (0x1_0000, 0x3f_0000),
        Chip::Esp32c5 => (0x1_0000, 0x3f_0000),
        Chip::Esp32c6 => (0x1_0000, 0x3f_0000),
        Chip::Esp32h2 => (0x1_0000, 0x3f_0000),
        Chip::Esp32p4 => (0x1_0000, 0x3f_0000),
        Chip::Esp32s2 => (0x1_0000, 0x10_0000),
        Chip::Esp32s3 => (0x1_0000, 0x10_0000),
    };

    PartitionTable::new(vec![
        Partition::new(
            String::from("nvs"),
            Type::Data,
            SubType::Data(DataType::Nvs),
            NVS_ADDR,
            NVS_SIZE,
            Flags::empty(),
        ),
        Partition::new(
            String::from("phy_init"),
            Type::Data,
            SubType::Data(DataType::Phy),
            PHY_INIT_DATA_ADDR,
            PHY_INIT_DATA_SIZE,
            Flags::empty(),
        ),
        Partition::new(
            String::from("factory"),
            Type::App,
            SubType::App(AppType::Factory),
            app_addr,
            core::cmp::min(
                flash_size.map_or(app_size, |size| size - app_addr),
                MAX_PARTITION_SIZE,
            ),
            Flags::empty(),
        ),
    ])
}

/// Actual alignment (in data bytes) required for a segment header: positioned
/// so that after we write the next 8 byte header, file_offset % IROM_ALIGN ==
/// segment.addr % IROM_ALIGN
///
/// (this is because the segment's vaddr may not be IROM_ALIGNed, more likely is
/// aligned IROM_ALIGN+0x18 to account for the binary file header)
fn segment_padding(offset: usize, segment: &Segment<'_>, align_to: u32) -> u32 {
    let align_past = (segment.addr - SEG_HEADER_LEN) % align_to;
    let pad_len = ((align_to - ((offset as u32) % align_to)) + align_past) % align_to;

    if pad_len % align_to == 0 {
        0
    } else if pad_len > SEG_HEADER_LEN {
        pad_len - SEG_HEADER_LEN
    } else {
        pad_len + align_to - SEG_HEADER_LEN
    }
}

/// Merge adjacent segments into one.
fn merge_adjacent_segments(mut segments: Vec<Segment<'_>>) -> Vec<Segment<'_>> {
    segments.sort();

    let mut merged: Vec<Segment<'_>> = Vec::with_capacity(segments.len());
    for segment in segments {
        if let Some(last) = merged.last_mut() {
            let last_end = last.addr + last.size();
            if last_end == segment.addr {
                *last += segment.data();
                continue;
            }

            // There is some space between the segments. We can merge them if they would
            // either be contiguous, or overlap, if the first segment was 4-byte
            // aligned.
            let max_padding = (4 - last_end % 4) % 4;
            if last_end + max_padding >= segment.addr {
                *last += &[0u8; 4][..(segment.addr - last_end) as usize];
                *last += segment.data();
                continue;
            }
        }

        merged.push(segment)
    }

    merged
}

fn pad_align_segments(mut segments: Vec<Segment<'_>>) -> Vec<Segment<'_>> {
    segments.iter_mut().for_each(|segment| segment.pad_align(4));
    segments
}

/// Save a segment to the data buffer.
fn save_flash_segment(
    data: &mut Vec<u8>,
    mut segment: Segment<'_>,
    checksum: u8,
    mmu_page_size: u32,
) -> Result<u8, Error> {
    let end_pos = (data.len() + segment.data().len()) as u32 + SEG_HEADER_LEN;
    let segment_remainder = end_pos % mmu_page_size;

    if segment_remainder < 0x24 {
        // Work around a bug in ESP-IDF 2nd stage bootloader, that it didn't map the
        // last MMU page, if an IROM/DROM segment was < 0x24 bytes over the page
        // boundary.
        static PADDING: [u8; 0x24] = [0; 0x24];

        segment += &PADDING[0..(0x24 - segment_remainder as usize)];
    }

    let checksum = save_segment(data, &segment, checksum)?;

    Ok(checksum)
}

/// Stores a segment header and the segment data in the data buffer.
fn save_segment(data: &mut Vec<u8>, segment: &Segment<'_>, checksum: u8) -> Result<u8, Error> {
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

/// Parse a [PartitionTable] from the provided path
pub fn parse_partition_table(path: &Path) -> Result<PartitionTable, Error> {
    let data = fs::read(path).map_err(|e| Error::FileOpenError(path.display().to_string(), e))?;

    Ok(PartitionTable::try_from(data)?)
}

fn encode_hex<T>(data: T) -> String
where
    T: AsRef<[u8]>,
{
    const HEX_CHARS: &[u8] = b"0123456789abcdef";

    let mut s = String::new();
    for byte in data.as_ref() {
        s.push(HEX_CHARS[(byte >> 4) as usize] as char);
        s.push(HEX_CHARS[(byte & 0x0F) as usize] as char);
    }

    s
}

/// Check if the provided ELF contains the app descriptor required by [the IDF bootloader](https://docs.espressif.com/projects/esp-idf/en/stable/esp32/api-guides/bootloader.html).
pub fn check_idf_bootloader(elf_data: &Vec<u8>) -> Result<()> {
    let object = File::parse(elf_data.as_slice()).into_diagnostic()?;

    // Check for section and symbol presence in esp-hal based projects
    let section = object.section_by_name(".rodata_desc").is_some();
    let is_esp_hal = object.section_by_name(".espressif.metadata").is_some();
    let esp_hal_symbol = object.symbols().any(|sym| sym.name() == Ok("esp_app_desc"));

    // Check for esp-idf (.flash.rodata) without .flash.appdesc (app desc)
    let is_esp_idf_without_app_desc = object.section_by_name(".flash.rodata").is_some()
        && !object.section_by_name(".flash.appdesc").is_some();

    if is_esp_hal && (!section || !esp_hal_symbol) {
        // esp-hal specific, because it searches for the hard-coded esp_app_desc symbol
        // in an ELF
        return Err(Error::AppDescriptorNotPresent {
        error: "The app descriptor is not present in the `esp-hal` based project.\n\
                You need to add the https://github.com/esp-rs/esp-hal/tree/main/esp-bootloader-esp-idf \
                to your project."
            .to_string(),
    })
    .into_diagnostic();
    } else if is_esp_idf_without_app_desc {
        // esp-idf specific, because it searches for the .flash.rodata and
        // .flash.appdesc sections
        return Err(Error::AppDescriptorNotPresent {
            error: "The app descriptor is not present in the `esp-idf` based project.".to_string(),
        })
        .into_diagnostic();
    }

    Ok(())
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

    #[test]
    fn test_encode_hex() {
        assert_eq!(encode_hex(&[0u8]), "00");
        assert_eq!(encode_hex(&[10u8]), "0a");
        assert_eq!(encode_hex(&[255u8]), "ff");

        assert_eq!(encode_hex(&[222u8, 202, 251, 173]), "decafbad");
    }

    #[test]
    fn merge_adjacent_segments_pads() {
        let segments = vec![
            Segment::new(0x1000, &[0u8; 0x100]),
            Segment::new(0x1100, &[0u8; 0xFF]),
            Segment::new(0x1200, &[0u8; 0x100]),
        ];

        let merged = merge_adjacent_segments(segments);
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].addr, 0x1000);
        assert_eq!(merged[0].size(), 0x300);
    }
}

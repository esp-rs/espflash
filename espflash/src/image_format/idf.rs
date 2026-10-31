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
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::{Metadata, Segment, ram_segments, rom_segments};
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

/// The partition table offset of the bundled bootloaders, which is the ESP-IDF
/// default of `CONFIG_PARTITION_TABLE_OFFSET`.
const BUNDLED_PARTITION_TABLE_OFFSET: u32 = 0x8000;

/// The flash space of the partition table, from its offset
const PARTITION_TABLE_SIZE: u32 = 0x1000;

/// The bundled bootloaders for one chip and crystal frequency.
#[derive(Clone, Copy)]
pub(crate) struct Bootloaders {
    /// The default bootloader, which leaves the flash in dual mode.
    pub(crate) dio: &'static [u8],
    /// A bootloader built for QIO. It enables quad mode on the flash chip
    /// itself, so it is used only when QIO is asked for.
    pub(crate) qio: &'static [u8],
}

macro_rules! bootloaders {
    ($name:literal) => {
        Bootloaders {
            dio: include_bytes!(concat!(
                "../../resources/bootloaders/",
                $name,
                "-bootloader.bin"
            )),
            qio: include_bytes!(concat!(
                "../../resources/bootloaders/",
                $name,
                "-qio-bootloader.bin"
            )),
        }
    };
}

const BOOTLOADER_ESP32_26MHZ: Bootloaders = bootloaders!("esp32_26");
const BOOTLOADER_ESP32_40MHZ: Bootloaders = bootloaders!("esp32");

const BOOTLOADER_ESP32C2_26MHZ: Bootloaders = bootloaders!("esp32c2_26");
const BOOTLOADER_ESP32C2_40MHZ: Bootloaders = bootloaders!("esp32c2");

const BOOTLOADER_ESP32C3: Bootloaders = bootloaders!("esp32c3");
const BOOTLOADER_ESP32C5: Bootloaders = bootloaders!("esp32c5");
const BOOTLOADER_ESP32C6: Bootloaders = bootloaders!("esp32c6");
const BOOTLOADER_ESP32C61: Bootloaders = bootloaders!("esp32c61");
const BOOTLOADER_ESP32H2: Bootloaders = bootloaders!("esp32h2");
const BOOTLOADER_ESP32H4_V0_0: Bootloaders = bootloaders!("esp32h4-v0.0");
const BOOTLOADER_ESP32H4_V0_2: Bootloaders = bootloaders!("esp32h4-v0.2");
const BOOTLOADER_ESP32P4_V0: Bootloaders = bootloaders!("esp32p4-v0");
const BOOTLOADER_ESP32P4_V3: Bootloaders = bootloaders!("esp32p4-v3");
const BOOTLOADER_ESP32S2: Bootloaders = bootloaders!("esp32s2");
const BOOTLOADER_ESP32S3: Bootloaders = bootloaders!("esp32s3");
const BOOTLOADER_ESP32S31: Bootloaders = bootloaders!("esp32s31");
/// Get the default bootloaders for the given chip, crystal frequency, and
/// chip revision.
pub(crate) fn default_bootloaders(
    chip: Chip,
    xtal_freq: XtalFrequency,
    chip_revision: Option<u16>,
    min_chip_rev: u16,
) -> Result<Bootloaders, Error> {
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
        Chip::Esp32c61 => match xtal_freq {
            XtalFrequency::_40Mhz => Ok(BOOTLOADER_ESP32C61),
            _ => Err(error),
        },
        Chip::Esp32h2 => match xtal_freq {
            XtalFrequency::_32Mhz => Ok(BOOTLOADER_ESP32H2),
            _ => Err(error),
        },
        Chip::Esp32h4 => match xtal_freq {
            XtalFrequency::_32Mhz => {
                let revision = chip_revision.unwrap_or(min_chip_rev);

                if revision >= 2 {
                    Ok(BOOTLOADER_ESP32H4_V0_2)
                } else {
                    Ok(BOOTLOADER_ESP32H4_V0_0)
                }
            }
            _ => Err(error),
        },
        Chip::Esp32p4 => match xtal_freq {
            XtalFrequency::_40Mhz => {
                let revision = chip_revision.unwrap_or(min_chip_rev);

                if revision >= 300 {
                    Ok(BOOTLOADER_ESP32P4_V3)
                } else {
                    Ok(BOOTLOADER_ESP32P4_V0)
                }
            }
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
        Chip::Esp32s31 => match xtal_freq {
            XtalFrequency::_40Mhz => Ok(BOOTLOADER_ESP32S31),
            _ => Err(error),
        },
    }
}

/// The mode to write into the 2nd stage bootloader's own header.
///
/// The ROM cannot boot a bootloader marked QIO or QOUT. ESP-IDF marks it DIO
/// for both ([`Kconfig.projbuild`]), and a bootloader built for quad mode
/// enables quad mode itself.
///
/// [`Kconfig.projbuild`]: https://github.com/espressif/esp-idf/blob/9a97f6c54ec638111ce55cd36581b3c192f15207/components/esptool_py/Kconfig.projbuild#L76-L84
fn bootloader_flash_mode(mode: FlashMode) -> FlashMode {
    match mode {
        FlashMode::Qio | FlashMode::Qout => FlashMode::Dio,
        mode => mode,
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
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
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
        Self::new_with_chip_revision(
            elf_data,
            flash_data,
            partition_table_path,
            bootloader_path,
            partition_table_offset,
            target_app_partition,
            None,
        )
    }

    pub(crate) fn new_with_chip_revision(
        elf_data: &'a [u8],
        flash_data: &FlashData,
        partition_table_path: Option<&Path>,
        bootloader_path: Option<&Path>,
        partition_table_offset: Option<u32>,
        target_app_partition: Option<&str>,
        chip_revision: Option<u16>,
    ) -> Result<Self, Error> {
        let elf = ElfFile::parse(elf_data)?;

        // Use the minimum chip revision declared in the ELF metadata as a
        // floor, allowing the CLI value to raise it further but not
        // lower it.
        let metadata_min_rev = Metadata::from_bytes(Some(elf_data))
            .min_chip_revision()
            .unwrap_or(0);
        let min_chip_rev = flash_data.min_chip_rev.max(metadata_min_rev);

        let partition_table = if let Some(partition_table_path) = partition_table_path {
            let data = fs::read(partition_table_path)
                .map_err(|e| Error::FileOpenError(partition_table_path.display().to_string(), e))?;
            PartitionTable::try_from(data)?
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
            let bootloaders = default_bootloaders(
                flash_data.chip,
                flash_data.xtal_freq,
                chip_revision,
                min_chip_rev,
            )?;
            match flash_data.flash_settings.mode {
                Some(FlashMode::Qio) => Cow::Borrowed(bootloaders.qio),
                Some(FlashMode::Qout) => {
                    warn!(
                        "There is no bundled {} bootloader for QOUT: the flash will be read in dual mode. Use `--bootloader` with an ESP-IDF bootloader built for QOUT to change that.",
                        flash_data.chip
                    );
                    Cow::Borrowed(bootloaders.dio)
                }
                _ => Cow::Borrowed(bootloaders.dio),
            }
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

        // The app header keeps the requested mode, the bootloader's own header
        // must not be quad
        let mut bootloader_header = header;
        if let Some(mode) = flash_data.flash_settings.mode {
            bootloader_header.flash_mode = bootloader_flash_mode(mode) as u8;
        }
        bootloader.to_mut().splice(
            0..size_of::<ImageHeader>(),
            bytes_of(&bootloader_header).iter().copied(),
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
        header.min_chip_rev_full = min_chip_rev;
        header.append_digest = 1;

        let mut data = bytes_of(&header).to_vec();

        // The bootloader needs segments to be 4-byte aligned, but ensuring that
        // alignment by padding segments might result in overlapping segments.
        // We need to merge adjacent segments first to avoid the
        // possibility of them overlapping, and then do the padding.
        let mut flash_segments: Vec<_> = pad_align_segments(merge_adjacent_segments(
            rom_segments(flash_data.chip, &elf).collect(),
        ));
        let mut ram_segments: Vec<_> = pad_align_segments(merge_adjacent_segments(
            ram_segments(flash_data.chip, &elf).collect(),
        ));

        let mut checksum = ESP_CHECKSUM_MAGIC;
        let mut segment_count = 0;

        // Find and bubble the app descriptor segment to the first position. We
        // do this after merging/padding the segments, so it should be
        // okay to reorder them now.
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
            .map(|size| format!("{size:#x}"))
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

                // Subtract image + extended header (24 bytes) and segment
                // header (8 bytes)
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
                        "The app descriptor is placed at {address:#x} which is not aligned to any of the \
                        supported page sizes: {valid_page_sizes_string}"
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
                "MMU page size {mmu_page_size:#x} is not supported. Supported page sizes are: {valid_page_sizes_string}"
            );
            return Err(AppDescriptorError::IncorrectDescriptorAlignment.into());
        };

        // Calculate SHA256 of the entire ELF file and update the app descriptor
        if let Some(address) = app_desc_addr {
            let segment = &mut flash_segments[0];
            let offset = (address - segment.addr) as usize;
            let app_descriptor_size = size_of::<AppDescriptor>();

            let mut hasher = Sha256::new();
            hasher.update(elf_data);
            let elf_sha256 = hasher.finalize();

            let segment_data = segment.data_mut();
            let app_descriptor_bytes = &mut segment_data[offset..][..app_descriptor_size];
            let app_descriptor: &mut AppDescriptor = bytemuck::from_bytes_mut(app_descriptor_bytes);
            app_descriptor.app_elf_sha256.copy_from_slice(&elf_sha256);
        }

        for segment in flash_segments {
            loop {
                let pad_len = segment_padding(data.len(), &segment, mmu_page_size);

                // Optimisation: if flash segments need padding, we fill it up
                // with RAM segment data if we can, to save
                // space in the final image.
                if pad_len > SEG_HEADER_LEN
                    && let Some(ram_segment) = ram_segments.first_mut()
                {
                    // save up to `pad_len` from the ram segment, any remaining
                    // bits in the ram segments will be
                    // saved later
                    let pad_segment = ram_segment.split_off(pad_len as usize);
                    checksum = save_segment(&mut data, &pad_segment, checksum)?;
                    if ram_segment.data().is_empty() {
                        ram_segments.remove(0);
                    }
                    segment_count += 1;
                } else if pad_len > 0 {
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

        // since we added some dummy segments, we need to patch the segment
        // count
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

        // The size of the application must not exceed the size of the target
        // app partition.
        if app_size as f32 / partition_table_size as f32 > 1.0 {
            return Err(Error::ElfTooBig(app_size, partition_table_size));
        }

        let flash_segment = Segment {
            addr: target_app_partition.offset(),
            data: Cow::Owned(data),
        };

        // If the user did not specify a partition offset, use the offset of
        // the bundled bootloader. For another bootloader, we need to assume
        // that the partition offset is (first partition offset) - 0x1000,
        // since this is the most common case.
        let partition_table_offset = partition_table_offset.unwrap_or_else(|| {
            if bootloader_path.is_none() {
                return BUNDLED_PARTITION_TABLE_OFFSET;
            }
            let partitions = partition_table.partitions();
            let first_partition = partitions
                .iter()
                .min_by(|a, b| a.offset().cmp(&b.offset()))
                .unwrap();
            first_partition.offset() - PARTITION_TABLE_SIZE
        });

        if bootloader_path.is_none() && partition_table_offset != BUNDLED_PARTITION_TABLE_OFFSET {
            warn!(
                "The bundled bootloader reads the partition table at {BUNDLED_PARTITION_TABLE_OFFSET:#x}, not at {partition_table_offset:#x}. Use `--bootloader` with an ESP-IDF bootloader built for this partition table offset."
            );
        }

        let table_end = partition_table_offset.saturating_add(PARTITION_TABLE_SIZE);
        if let Some(partition) = partition_table.partitions().iter().find(|p| {
            p.offset() < table_end && partition_table_offset < p.offset().saturating_add(p.size())
        }) {
            return Err(Error::PartitionTableOverlap {
                offset: partition_table_offset,
                name: partition.name(),
                partition_offset: partition.offset(),
            });
        }

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
        Chip::Esp32c61 => (0x1_0000, 0x3f_0000),
        Chip::Esp32h2 => (0x1_0000, 0x3f_0000),
        Chip::Esp32h4 => (0x1_0000, 0x3f_0000),
        Chip::Esp32p4 => (0x1_0000, 0x3f_0000),
        Chip::Esp32s2 => (0x1_0000, 0x10_0000),
        Chip::Esp32s3 => (0x1_0000, 0x10_0000),
        Chip::Esp32s31 => (0x1_0000, 0x3f_0000),
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

    if pad_len.is_multiple_of(align_to) {
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

            // There is some space between the segments. We can merge them if
            // they would either be contiguous, or overlap, if the
            // first segment was 4-byte aligned.
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
        // Work around a bug in ESP-IDF 2nd stage bootloader, that it didn't map
        // the last MMU page, if an IROM/DROM segment was < 0x24 bytes
        // over the page boundary.
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

    // A project with `strip = true` will discard the
    // symbol we are looking for but the section is kept
    let has_app_desc = object.symbols().any(|sym| sym.name() == Ok("esp_app_desc"))
        || object.section_by_name(".flash.appdesc").is_some()
        || object.section_by_name(".rodata_desc").is_some();

    if !has_app_desc {
        // when using `strip = true` we will (maybe wrongly) assume ESP-IDF
        // but at least it should still guide the user into the right direction
        let is_esp_hal = object.section_by_name(".espressif.metadata").is_some();

        if is_esp_hal {
            return Err(Error::AppDescriptorNotPresent(
                "ESP-IDF App Descriptor (https://docs.espressif.com/projects/esp-idf/en/stable/esp32/api-reference/system/app_image_format.html#application-description) missing in your`esp-hal` application.\n
                You may need to add the `esp_bootloader_esp_idf::esp_app_desc!()` macro to your application, see https://docs.espressif.com/projects/rust/esp-bootloader-esp-idf/latest for more information."
                    .to_string(),
            ))
            .into_diagnostic();
        } else {
            return Err(Error::AppDescriptorNotPresent(
                "ESP-IDF App Descriptor (https://docs.espressif.com/projects/esp-idf/en/stable/esp32/api-reference/system/app_image_format.html#application-description) missing in your`esp-idf` application.".to_string(),
            ))
            .into_diagnostic();
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::sync::{Mutex, MutexGuard, Once};

    use super::*;
    use crate::flasher::FlashSettings;

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

    /// The bootloader, partition table and app images, as flashed
    fn image(chip: Chip, mode: Option<FlashMode>) -> Vec<Vec<u8>> {
        image_with_bootloader(chip, mode, None)
    }

    fn image_with_bootloader(
        chip: Chip,
        mode: Option<FlashMode>,
        bootloader: Option<&Path>,
    ) -> Vec<Vec<u8>> {
        let elf = std::fs::read(format!("tests/data/{chip}")).unwrap();
        let flash_data = FlashData::new(
            FlashSettings::new(mode, None, None),
            0,
            None,
            chip,
            XtalFrequency::_40Mhz,
        );
        let image =
            IdfBootloaderFormat::new(&elf, &flash_data, None, bootloader, None, None).unwrap();
        image
            .flash_segments()
            .map(|s| s.data.into_owned())
            .collect()
    }

    fn sha256_valid(bootloader: &[u8]) -> bool {
        let digest = bootloader.len() - 32;
        Sha256::digest(&bootloader[..digest]).as_slice() == &bootloader[digest..]
    }

    #[test]
    fn bootloader_header_is_never_quad() {
        assert_eq!(bootloader_flash_mode(FlashMode::Qio), FlashMode::Dio);
        assert_eq!(bootloader_flash_mode(FlashMode::Qout), FlashMode::Dio);
        assert_eq!(bootloader_flash_mode(FlashMode::Dio), FlashMode::Dio);
        assert_eq!(bootloader_flash_mode(FlashMode::Dout), FlashMode::Dout);
    }

    /// Whether `bootloader` is `bundled` apart from the header and the hash
    fn is_bundled(bootloader: &[u8], bundled: &[u8]) -> bool {
        let body = size_of::<ImageHeader>()..bundled.len() - 32;
        bootloader.len() == bundled.len() && bootloader[body.clone()] == bundled[body]
    }

    /// The chips with test ELFs for a 40 MHz crystal
    const CHIPS_40MHZ: [Chip; 10] = [
        Chip::Esp32,
        Chip::Esp32c2,
        Chip::Esp32c3,
        Chip::Esp32c5,
        Chip::Esp32c6,
        Chip::Esp32c61,
        Chip::Esp32p4,
        Chip::Esp32s2,
        Chip::Esp32s3,
        Chip::Esp32s31,
    ];

    #[test]
    fn qio_uses_the_quad_bootloader_marked_dio() {
        for chip in CHIPS_40MHZ {
            let bundled = default_bootloaders(chip, XtalFrequency::_40Mhz, None, 0).unwrap();
            assert_ne!(bundled.qio, bundled.dio, "{chip}");

            let segments = image(chip, Some(FlashMode::Qio));
            let (bootloader, app) = (&segments[0], &segments[2]);

            assert!(is_bundled(bootloader, bundled.qio), "{chip}");
            assert_eq!(bootloader[2], FlashMode::Dio as u8, "{chip}");
            assert!(sha256_valid(bootloader), "{chip}");
            assert_eq!(app[2], FlashMode::Qio as u8, "{chip}");
        }
    }

    #[test]
    fn dio_keeps_the_default_bootloader() {
        let bundled = default_bootloaders(Chip::Esp32s3, XtalFrequency::_40Mhz, None, 0).unwrap();
        let segments = image(Chip::Esp32s3, Some(FlashMode::Dio));
        let (bootloader, app) = (&segments[0], &segments[2]);

        assert!(is_bundled(bootloader, bundled.dio));
        assert_eq!(bootloader[2], FlashMode::Dio as u8);
        assert!(sha256_valid(bootloader));
        assert_eq!(app[2], FlashMode::Dio as u8);
    }

    #[test]
    fn qout_falls_back_to_the_dio_bootloader() {
        let bundled = default_bootloaders(Chip::Esp32c3, XtalFrequency::_40Mhz, None, 0).unwrap();
        let segments = image(Chip::Esp32c3, Some(FlashMode::Qout));
        let (bootloader, app) = (&segments[0], &segments[2]);

        assert!(is_bundled(bootloader, bundled.dio));
        assert_eq!(bootloader[2], FlashMode::Dio as u8);
        assert!(sha256_valid(bootloader));
        assert_eq!(app[2], FlashMode::Qout as u8);
    }

    #[test]
    fn no_mode_keeps_the_dio_bootloader_and_its_mode() {
        let bundled = default_bootloaders(Chip::Esp32c3, XtalFrequency::_40Mhz, None, 0).unwrap();
        let segments = image(Chip::Esp32c3, None);
        let bootloader = &segments[0];

        assert!(is_bundled(bootloader, bundled.dio));
        assert_eq!(bootloader[2], bundled.dio[2]);
        assert!(sha256_valid(bootloader));
    }

    #[test]
    fn qio_with_a_given_bootloader_keeps_it_marked_dio() {
        let path = Path::new("resources/bootloaders/esp32s3-bootloader.bin");
        let given = std::fs::read(path).unwrap();
        let segments = image_with_bootloader(Chip::Esp32s3, Some(FlashMode::Qio), Some(path));
        let (bootloader, app) = (&segments[0], &segments[2]);

        assert!(is_bundled(bootloader, &given));
        assert_eq!(bootloader[2], FlashMode::Dio as u8);
        assert!(sha256_valid(bootloader));
        assert_eq!(app[2], FlashMode::Qio as u8);
    }

    /// The address of the partition table `tests/data/{table}`, as flashed
    fn partition_table_address(
        table: &str,
        bootloader: Option<&Path>,
        offset: Option<u32>,
    ) -> Result<u32, Error> {
        let elf = std::fs::read("tests/data/esp32").unwrap();
        let flash_data = FlashData::new(
            FlashSettings::new(None, Some(FlashSize::_16Mb), None),
            0,
            None,
            Chip::Esp32,
            XtalFrequency::_40Mhz,
        );
        let partition_table = Path::new("tests/data").join(table);
        let image = IdfBootloaderFormat::new(
            &elf,
            &flash_data,
            Some(&partition_table),
            bootloader,
            offset,
            None,
        )?;
        Ok(image.flash_segments().nth(1).unwrap().addr)
    }

    const GIVEN_BOOTLOADER: &str = "resources/bootloaders/esp32-bootloader.bin";

    /// The messages of the warnings that the tests log
    static WARNINGS: Mutex<Vec<String>> = Mutex::new(Vec::new());

    struct WarningLogger;

    impl log::Log for WarningLogger {
        fn enabled(&self, metadata: &log::Metadata<'_>) -> bool {
            metadata.level() <= log::Level::Warn
        }

        fn log(&self, record: &log::Record<'_>) {
            if self.enabled(record.metadata()) {
                WARNINGS.lock().unwrap().push(record.args().to_string());
            }
        }

        fn flush(&self) {}
    }

    /// The warnings logged so far, from the first call on
    fn warnings() -> MutexGuard<'static, Vec<String>> {
        static LOGGER: Once = Once::new();
        LOGGER.call_once(|| {
            log::set_logger(&WarningLogger).unwrap();
            log::set_max_level(log::LevelFilter::Warn);
        });
        WARNINGS.lock().unwrap()
    }

    #[test]
    fn bundled_bootloader_gets_the_partition_table_at_its_offset() {
        let address = partition_table_address("partitions_app_at_0x40000.csv", None, None);
        assert_eq!(address.unwrap(), 0x8000);
    }

    #[test]
    fn given_bootloader_gets_the_partition_table_below_the_first_partition() {
        let given = Some(Path::new(GIVEN_BOOTLOADER));
        let address = partition_table_address("partitions_app_at_0x40000.csv", given, None);
        assert_eq!(address.unwrap(), 0x3F000);
    }

    #[test]
    fn partition_table_must_not_overlap_a_partition() {
        let given = Some(Path::new(GIVEN_BOOTLOADER));
        let cases = [
            ("partitions_nvs_at_0x6000.csv", None, None),
            ("partitions.csv", None, Some(0x9000)),
            ("partitions_app_at_0x40000.csv", given, Some(0x13F000)),
        ];

        for (table, bootloader, offset) in cases {
            assert!(
                matches!(
                    partition_table_address(table, bootloader, offset),
                    Err(Error::PartitionTableOverlap { .. })
                ),
                "{table} {offset:x?}"
            );
        }
    }

    #[test]
    fn partition_table_may_touch_a_partition() {
        let given = Some(Path::new(GIVEN_BOOTLOADER));
        let cases = [
            ("partitions.csv", None, None, 0x8000),
            (
                "partitions_app_at_0x40000.csv",
                given,
                Some(0x140000),
                0x140000,
            ),
        ];

        for (table, bootloader, offset, expected) in cases {
            let address = partition_table_address(table, bootloader, offset);
            assert_eq!(address.unwrap(), expected, "{table}");
        }
    }

    #[test]
    fn bundled_bootloader_warns_about_another_partition_table_offset() {
        drop(warnings());
        let given = Some(Path::new(GIVEN_BOOTLOADER));

        let bundled = partition_table_address("partitions_app_at_0x40000.csv", None, Some(0xA000));
        let other = partition_table_address("partitions_app_at_0x40000.csv", given, Some(0xB000));

        assert_eq!(bundled.unwrap(), 0xA000);
        assert_eq!(other.unwrap(), 0xB000);
        let warnings = warnings();
        assert!(warnings.iter().any(|w| w.contains("not at 0xa000")));
        assert!(!warnings.iter().any(|w| w.contains("0xb000")));
    }

    #[test]
    fn each_chip_crystal_and_revision_gets_its_own_bootloaders() {
        let arms = [
            (Chip::Esp32, XtalFrequency::_26Mhz, 0, "esp32_26"),
            (Chip::Esp32, XtalFrequency::_40Mhz, 0, "esp32"),
            (Chip::Esp32c2, XtalFrequency::_26Mhz, 0, "esp32c2_26"),
            (Chip::Esp32c2, XtalFrequency::_40Mhz, 0, "esp32c2"),
            (Chip::Esp32c3, XtalFrequency::_40Mhz, 0, "esp32c3"),
            (Chip::Esp32c5, XtalFrequency::_40Mhz, 0, "esp32c5"),
            (Chip::Esp32c5, XtalFrequency::_48Mhz, 0, "esp32c5"),
            (Chip::Esp32c6, XtalFrequency::_40Mhz, 0, "esp32c6"),
            (Chip::Esp32c61, XtalFrequency::_40Mhz, 0, "esp32c61"),
            (Chip::Esp32h2, XtalFrequency::_32Mhz, 0, "esp32h2"),
            (Chip::Esp32h4, XtalFrequency::_32Mhz, 1, "esp32h4-v0.0"),
            (Chip::Esp32h4, XtalFrequency::_32Mhz, 2, "esp32h4-v0.2"),
            (Chip::Esp32p4, XtalFrequency::_40Mhz, 0, "esp32p4-v0"),
            (Chip::Esp32p4, XtalFrequency::_40Mhz, 300, "esp32p4-v3"),
            (Chip::Esp32s2, XtalFrequency::_40Mhz, 0, "esp32s2"),
            (Chip::Esp32s3, XtalFrequency::_40Mhz, 0, "esp32s3"),
            (Chip::Esp32s31, XtalFrequency::_40Mhz, 0, "esp32s31"),
        ];

        for (chip, xtal, revision, name) in arms {
            let bundled = default_bootloaders(chip, xtal, Some(revision), 0).unwrap();
            let file =
                |suffix| std::fs::read(format!("resources/bootloaders/{name}{suffix}")).unwrap();

            assert_eq!(bundled.dio, file("-bootloader.bin"), "{name}");
            assert_eq!(bundled.qio, file("-qio-bootloader.bin"), "{name}");
        }
    }

    #[test]
    fn test_encode_hex() {
        assert_eq!(encode_hex([0u8]), "00");
        assert_eq!(encode_hex([10u8]), "0a");
        assert_eq!(encode_hex([255u8]), "ff");

        assert_eq!(encode_hex([222u8, 202, 251, 173]), "decafbad");
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

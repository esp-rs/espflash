//! Direct boot image format

use std::{borrow::Cow, collections::HashMap};

use object::{
    Endianness,
    elf::{PT_LOAD, SHF_ALLOC, SHT_NOBITS},
    read::elf::{ElfFile32 as ElfFile, ProgramHeader as _, SectionHeader as _},
};
use serde::{Deserialize, Serialize};

use super::{ImageFormatKind, Segment};
use crate::{Error, target::Chip};

/// The ROM starts the application if it finds this word twice in a row at
/// [`magic_offset`].
const DIRECT_BOOT_MAGIC: [u8; 8] = [0x1d, 0x04, 0xdb, 0xae, 0x1d, 0x04, 0xdb, 0xae];

/// The largest flash the direct boot linker scripts map (64 MB). Load
/// addresses at or above it are memory addresses, not flash offsets.
const MAX_FLASH_OFFSET: u32 = 0x0400_0000;

/// Returns the flash offset at which the ROM looks for [`DIRECT_BOOT_MAGIC`],
/// or `None` if the chip does not support direct boot.
fn magic_offset(chip: Chip) -> Option<u32> {
    match chip {
        Chip::Esp32 | Chip::Esp32s2 => None,
        Chip::Esp32s3 => Some(0x400),
        Chip::Esp32c2
        | Chip::Esp32c3
        | Chip::Esp32c5
        | Chip::Esp32c6
        | Chip::Esp32c61
        | Chip::Esp32h2
        | Chip::Esp32h4
        | Chip::Esp32p4
        | Chip::Esp32s31 => Some(0x0),
    }
}

/// Returns the lowest revision, in `major * 100 + minor` format, that
/// direct boot works on, or `None` if it works on every revision.
fn min_revision(chip: Chip) -> Option<u16> {
    match chip {
        // Earlier ROMs do not support direct boot.
        Chip::Esp32c3 => Some(3),
        // The entry stub of `esp-bootloader-direct-boot` hangs on earlier
        // revisions, before `esp_hal::init` can check the revision.
        Chip::Esp32p4 => Some(300),
        _ => None,
    }
}

/// Image format for applications that the ROM boots directly, without a
/// second-stage bootloader, partition table or app image header.
///
/// The image consists of the ELF file's loadable segments placed at their
/// load addresses, which are flash offsets.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct DirectBootFormat<'a> {
    segment: Segment<'a>,
}

impl<'a> DirectBootFormat<'a> {
    /// Create a new [`DirectBootFormat`].
    ///
    /// `chip_revision` is the revision of the target, in `major * 100 +
    /// minor` format, if known.
    pub fn new(elf_data: &'a [u8], chip: Chip, chip_revision: Option<u16>) -> Result<Self, Error> {
        let Some(magic_offset) = magic_offset(chip) else {
            return Err(Error::UnsupportedImageFormat {
                format: ImageFormatKind::DirectBoot,
                chip,
                context: None,
            });
        };

        if let (Some(min), Some(revision)) = (min_revision(chip), chip_revision)
            && revision < min
        {
            return Err(Error::UnsupportedImageFormat {
                format: ImageFormatKind::DirectBoot,
                chip,
                context: Some(format!(
                    "direct boot requires revision v{}.{} or later, found v{}.{}",
                    min / 100,
                    min % 100,
                    revision / 100,
                    revision % 100
                )),
            });
        }

        let elf = ElfFile::parse(elf_data)?;
        let endian = Endianness::Little;

        // Program segments can also contain the ELF and program headers, so the
        // image is made of sections, placed at the load address of the
        // segment that contains them.
        let mut segments = Vec::new();
        for section in elf.elf_section_table().iter() {
            if section.sh_flags(endian).0 & SHF_ALLOC.0 == 0
                || section.sh_type(endian) == SHT_NOBITS
                || section.sh_size(endian) == 0
            {
                continue;
            }

            let offset = section.sh_offset(endian);
            let Some(addr) = elf
                .elf_program_headers()
                .iter()
                .filter(|phdr| phdr.p_type(endian) == PT_LOAD)
                .find(|phdr| {
                    let start = phdr.p_offset(endian);
                    (start..start + phdr.p_filesz(endian)).contains(&offset)
                })
                .map(|phdr| phdr.p_paddr(endian) + (offset - phdr.p_offset(endian)))
            else {
                continue;
            };

            let data = section
                .data(endian, elf_data)
                .map_err(|_| Error::InvalidElf("Invalid section header".into()))?;

            if addr.saturating_add(data.len() as u32) > MAX_FLASH_OFFSET {
                return Err(Error::InvalidDirectBootBinary);
            }

            segments.push((addr, data));
        }
        segments.sort_by_key(|&(addr, _)| addr);

        if segments.is_empty() {
            return Err(Error::InvalidDirectBootBinary);
        }

        // The image always starts at 0x0, so the sector holding the boot
        // header is fully rewritten, even when the ROM looks for the magic
        // at a non-zero offset (ESP32-S3).
        let mut data = Vec::new();
        for (addr, segment) in segments {
            let offset = addr as usize;
            if offset < data.len() {
                return Err(Error::InvalidDirectBootBinary);
            }
            data.resize(offset, 0xff);
            data.extend_from_slice(segment);
        }

        let has_magic = data
            .get(magic_offset as usize..)
            .and_then(|data| data.get(..DIRECT_BOOT_MAGIC.len()))
            == Some(DIRECT_BOOT_MAGIC.as_slice());
        if !has_magic {
            return Err(Error::InvalidDirectBootBinary);
        }

        data.resize(data.len().next_multiple_of(4), 0xff);

        Ok(Self {
            segment: Segment {
                addr: 0,
                data: Cow::Owned(data),
            },
        })
    }

    /// Returns an iterator over the [Segment]'s that should be placed in flash.
    pub fn flash_segments<'b>(self) -> impl Iterator<Item = Segment<'b>>
    where
        'a: 'b,
    {
        std::iter::once(self.segment)
    }

    /// Returns an iterator over the OTA segment.
    pub fn ota_segments<'b>(self) -> impl Iterator<Item = Segment<'b>>
    where
        'a: 'b,
    {
        std::iter::once(self.segment)
    }

    /// Returns a map of metadata about the application image.
    pub fn metadata(&self) -> HashMap<&str, String> {
        HashMap::from([("app_size", self.segment.size().to_string())])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `expected_bin` is the image as produced by `objcopy`, which starts at
    /// the first section, `bin_offset`.
    fn check_image(elf: &str, expected_bin: &str, chip: Chip, bin_offset: usize) {
        let elf_data = std::fs::read(elf).unwrap();
        let expected = std::fs::read(expected_bin).unwrap();

        let segments = DirectBootFormat::new(&elf_data, chip, None)
            .unwrap()
            .flash_segments()
            .collect::<Vec<_>>();
        assert_eq!(segments.len(), 1);

        let segment = &segments[0];
        let data = segment.data();
        let end = bin_offset + expected.len();
        assert_eq!(segment.addr, 0);
        assert_eq!(data.len(), end.next_multiple_of(4));
        assert!(data[..bin_offset].iter().all(|&b| b == 0xff));
        assert_eq!(&data[bin_offset..end], expected.as_slice());
        assert!(data[end..].iter().all(|&b| b == 0xff));
    }

    #[test]
    fn test_direct_boot_image_c6() {
        check_image(
            "tests/data/esp_hal_direct_boot_c6.elf",
            "tests/data/esp_hal_direct_boot_c6.img",
            Chip::Esp32c6,
            0x0,
        );
    }

    #[test]
    fn test_direct_boot_image_s3() {
        check_image(
            "tests/data/esp_hal_direct_boot_s3.elf",
            "tests/data/esp_hal_direct_boot_s3.img",
            Chip::Esp32s3,
            0x400,
        );
    }

    #[test]
    fn test_direct_boot_unsupported_chips() {
        let elf_data = std::fs::read("tests/data/esp_hal_direct_boot_c6.elf").unwrap();

        for chip in [Chip::Esp32, Chip::Esp32s2] {
            assert!(matches!(
                DirectBootFormat::new(&elf_data, chip, None),
                Err(Error::UnsupportedImageFormat { .. })
            ));
        }
    }

    #[test]
    fn test_direct_boot_min_revision() {
        let elf_data = std::fs::read("tests/data/esp_hal_direct_boot_c6.elf").unwrap();

        for (chip, revision, supported) in [
            (Chip::Esp32c3, Some(2), false),
            (Chip::Esp32c3, Some(3), true),
            (Chip::Esp32p4, Some(101), false),
            (Chip::Esp32p4, Some(300), true),
            // Unknown revisions, e.g. in `save-image`, are not rejected
            (Chip::Esp32p4, None, true),
        ] {
            let result = DirectBootFormat::new(&elf_data, chip, revision);
            if supported {
                assert!(result.is_ok(), "{chip} {revision:?}");
            } else {
                assert!(
                    matches!(result, Err(Error::UnsupportedImageFormat { .. })),
                    "{chip} {revision:?}"
                );
            }
        }
    }

    #[test]
    fn test_direct_boot_magic_at_wrong_offset() {
        let elf_data = std::fs::read("tests/data/esp_hal_direct_boot_c6.elf").unwrap();

        assert!(matches!(
            DirectBootFormat::new(&elf_data, Chip::Esp32s3, None),
            Err(Error::InvalidDirectBootBinary)
        ));
    }

    #[test]
    fn test_esp_idf_elf_is_rejected() {
        let elf_data = std::fs::read("tests/data/esp_hal_backtrace_c6.elf").unwrap();

        assert!(matches!(
            DirectBootFormat::new(&elf_data, Chip::Esp32c6, None),
            Err(Error::InvalidDirectBootBinary)
        ));
    }
}

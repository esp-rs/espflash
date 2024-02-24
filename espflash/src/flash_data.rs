use std::{fs, path::Path, str::FromStr};

use esp_idf_part::PartitionTable;
use serde::{Deserialize, Serialize};
use strum::{Display, EnumIter, IntoEnumIterator, VariantNames};

use crate::{
    error::Error,
    targets::{Chip, XtalFrequency},
};

pub(crate) const CHECKSUM_INIT: u8 = 0xEF;
pub(crate) const FLASH_SECTOR_SIZE: usize = 0x1000;
pub(crate) const FLASH_WRITE_SIZE: usize = 0x400;

/// Parameters of the attached SPI flash chip (sizes, etc).
///
/// See https://github.com/espressif/esptool/blob/da31d9d7a1bb496995f8e30a6be259689948e43e/esptool.py#L655
#[derive(Copy, Clone, Debug)]
#[repr(C)]
pub struct SpiSetParams {
    /// Flash chip ID
    fl_id: u32,
    /// Total size in bytes
    total_size: u32,
    /// Block size
    block_size: u32,
    /// Sector size
    sector_size: u32,
    /// Page size
    page_size: u32,
    /// Status mask
    status_mask: u32,
}

impl SpiSetParams {
    pub const fn default(size: u32) -> Self {
        SpiSetParams {
            fl_id: 0,
            total_size: size,
            block_size: 64 * 1024,
            sector_size: 4 * 1024,
            page_size: 256,
            status_mask: 0xFFFF,
        }
    }

    /// Encode the parameters into a byte array
    pub fn encode(&self) -> Vec<u8> {
        let mut encoded: Vec<u8> = Vec::new();
        encoded.extend_from_slice(&self.fl_id.to_le_bytes());
        encoded.extend_from_slice(&self.total_size.to_le_bytes());
        encoded.extend_from_slice(&self.block_size.to_le_bytes());
        encoded.extend_from_slice(&self.sector_size.to_le_bytes());
        encoded.extend_from_slice(&self.page_size.to_le_bytes());
        encoded.extend_from_slice(&self.status_mask.to_le_bytes());
        encoded
    }
}

/// Supported flash frequencies
///
/// Note that not all frequencies are supported by each target device.
#[cfg_attr(feature = "cli", derive(clap::ValueEnum))]
#[derive(
    Debug, Default, Clone, Copy, Hash, PartialEq, Eq, Display, VariantNames, Serialize, Deserialize,
)]
#[non_exhaustive]
#[repr(u8)]
pub enum FlashFrequency {
    /// 12 MHz
    _12Mhz,
    /// 15 MHz
    _15Mhz,
    /// 16 MHz
    _16Mhz,
    /// 20 MHz
    _20Mhz,
    /// 24 MHz
    _24Mhz,
    /// 26 MHz
    _26Mhz,
    /// 30 MHz
    _30Mhz,
    /// 40 MHz
    #[default]
    _40Mhz,
    /// 48 MHz
    _48Mhz,
    /// 60 MHz
    _60Mhz,
    /// 80 MHz
    _80Mhz,
}

impl FlashFrequency {
    /// Encodes flash frequency into the format used by the bootloader.
    pub fn encode_flash_frequency(self: FlashFrequency, chip: Chip) -> Result<u8, Error> {
        let encodings = chip.into_target().flash_frequency_encodings();
        if let Some(&f) = encodings.get(&self) {
            Ok(f)
        } else {
            Err(Error::UnsupportedFlashFrequency {
                chip,
                frequency: self,
            })
        }
    }
}

/// Supported flash modes
#[cfg_attr(feature = "cli", derive(clap::ValueEnum))]
#[derive(Copy, Clone, Debug, Default, VariantNames, Serialize, Deserialize)]
#[non_exhaustive]
#[strum(serialize_all = "lowercase")]
pub enum FlashMode {
    /// Quad I/O (4 pins used for address & data)
    Qio,
    /// Quad Output (4 pins used for data)
    Qout,
    /// Dual I/O (2 pins used for address & data)
    #[default]
    Dio,
    /// Dual Output (2 pins used for data)
    Dout,
}

/// Supported flash sizes
///
/// Note that not all sizes are supported by each target device.
#[cfg_attr(feature = "cli", derive(clap::ValueEnum))]
#[derive(
    Clone,
    Copy,
    Debug,
    Default,
    Eq,
    PartialEq,
    Display,
    VariantNames,
    EnumIter,
    Serialize,
    Deserialize,
)]
#[non_exhaustive]
#[repr(u8)]
#[strum(serialize_all = "SCREAMING_SNAKE_CASE")]
#[doc(alias("esp_image_flash_size_t"))]
pub enum FlashSize {
    /// 256 KB
    _256Kb,
    /// 512 KB
    _512Kb,
    /// 1 MB
    _1Mb,
    /// 2 MB
    _2Mb,
    /// 4 MB
    #[default]
    _4Mb,
    /// 8 MB
    _8Mb,
    /// 16 MB
    _16Mb,
    /// 32 MB
    _32Mb,
    /// 64 MB
    _64Mb,
    /// 128 MB
    _128Mb,
    /// 256 MB
    _256Mb,
}

impl FlashSize {
    /// Encodes flash size into the format used by the bootloader.
    ///
    /// ## Values:
    ///
    /// * https://docs.espressif.com/projects/esptool/en/latest/esp32s3/advanced-topics/firmware-image-format.html#file-header
    pub const fn encode_flash_size(self: FlashSize) -> Result<u8, Error> {
        use FlashSize::*;

        let encoded = match self {
            _1Mb => 0,
            _2Mb => 1,
            _4Mb => 2,
            _8Mb => 3,
            _16Mb => 4,
            _32Mb => 5,
            _64Mb => 6,
            _128Mb => 7,
            _256Mb => 8,
            _ => return Err(Error::UnsupportedFlash(self as u8)),
        };

        Ok(encoded)
    }

    /// Create a [FlashSize] from an [u8]
    ///
    /// [source](https://github.com/espressif/esptool/blob/f4d2510e2c897621884f433ef3f191e8fc5ff184/esptool/cmds.py#L42)
    pub const fn from_detected(value: u8) -> Result<FlashSize, Error> {
        match value {
            0x12 | 0x32 => Ok(FlashSize::_256Kb),
            0x13 | 0x33 => Ok(FlashSize::_512Kb),
            0x14 | 0x34 => Ok(FlashSize::_1Mb),
            0x15 | 0x35 => Ok(FlashSize::_2Mb),
            0x16 | 0x36 => Ok(FlashSize::_4Mb),
            0x17 | 0x37 => Ok(FlashSize::_8Mb),
            0x18 | 0x38 => Ok(FlashSize::_16Mb),
            0x19 | 0x39 => Ok(FlashSize::_32Mb),
            0x20 | 0x1A | 0x3A => Ok(FlashSize::_64Mb),
            0x21 | 0x1B => Ok(FlashSize::_128Mb),
            0x22 | 0x1C => Ok(FlashSize::_256Mb),
            _ => Err(Error::UnsupportedFlash(value)),
        }
    }

    /// Returns the flash size in bytes
    pub const fn size(self) -> u32 {
        match self {
            FlashSize::_256Kb => 0x0040000,
            FlashSize::_512Kb => 0x0080000,
            FlashSize::_1Mb => 0x0100000,
            FlashSize::_2Mb => 0x0200000,
            FlashSize::_4Mb => 0x0400000,
            FlashSize::_8Mb => 0x0800000,
            FlashSize::_16Mb => 0x1000000,
            FlashSize::_32Mb => 0x2000000,
            FlashSize::_64Mb => 0x4000000,
            FlashSize::_128Mb => 0x8000000,
            FlashSize::_256Mb => 0x10000000,
        }
    }
}

impl FromStr for FlashSize {
    type Err = Error;
    /// Create a [FlashSize] from a string
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let upper = s.to_uppercase();
        FlashSize::VARIANTS
            .iter()
            .copied()
            .zip(FlashSize::iter())
            .find(|(name, _)| *name == upper)
            .map(|(_, variant)| variant)
            .ok_or_else(|| Error::InvalidFlashSize(s.to_string()))
    }
}

/// Information about the connected device
#[derive(Debug, Clone)]
pub struct DeviceInfo {
    /// The chip being used
    pub chip: Chip,
    /// The revision of the chip
    pub revision: Option<(u32, u32)>,
    /// The crystal frequency of the chip
    pub crystal_frequency: XtalFrequency,
    /// The total available flash size
    pub flash_size: FlashSize,
    /// Device features
    pub features: Vec<String>,
    /// MAC address
    pub mac_address: String,
}

/// Flash settings to use when flashing a device
#[derive(Copy, Clone, Debug)]
#[non_exhaustive]
pub struct FlashSettings {
    pub mode: Option<FlashMode>,
    pub size: Option<FlashSize>,
    pub freq: Option<FlashFrequency>,
}

impl FlashSettings {
    pub const fn default() -> Self {
        FlashSettings {
            mode: None,
            size: None,
            freq: None,
        }
    }

    pub fn new(
        mode: Option<FlashMode>,
        size: Option<FlashSize>,
        freq: Option<FlashFrequency>,
    ) -> Self {
        FlashSettings { mode, size, freq }
    }
}

/// Builder interface to create [`FlashData`] objects.
pub struct FlashDataBuilder<'a> {
    bootloader_path: Option<&'a Path>,
    partition_table_path: Option<&'a Path>,
    partition_table_offset: Option<u32>,
    target_app_partition: Option<String>,
    flash_settings: FlashSettings,
    min_chip_rev: u16,
}

impl<'a> Default for FlashDataBuilder<'a> {
    fn default() -> Self {
        Self {
            bootloader_path: Default::default(),
            partition_table_path: Default::default(),
            partition_table_offset: Default::default(),
            target_app_partition: Default::default(),
            flash_settings: FlashSettings::default(),
            min_chip_rev: Default::default(),
        }
    }
}

impl<'a> FlashDataBuilder<'a> {
    /// Creates a new [`FlashDataBuilder`] object.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the bootloader path.
    pub fn with_bootloader(mut self, bootloader_path: &'a Path) -> Self {
        self.bootloader_path = Some(bootloader_path);
        self
    }

    /// Sets the partition table path.
    pub fn with_partition_table(mut self, partition_table_path: &'a Path) -> Self {
        self.partition_table_path = Some(partition_table_path);
        self
    }

    /// Sets the partition table offset.
    pub fn with_partition_table_offset(mut self, partition_table_offset: u32) -> Self {
        self.partition_table_offset = Some(partition_table_offset);
        self
    }

    /// Sets the label of the target app partition.
    pub fn with_target_app_partition(mut self, target_app_partition: String) -> Self {
        self.target_app_partition = Some(target_app_partition);
        self
    }

    /// Sets the flash settings.
    pub fn with_flash_settings(mut self, flash_settings: FlashSettings) -> Self {
        self.flash_settings = flash_settings;
        self
    }

    /// Sets the minimum chip revision.
    pub fn with_min_chip_rev(mut self, min_chip_rev: u16) -> Self {
        self.min_chip_rev = min_chip_rev;
        self
    }

    /// Builds a [`FlashData`] object.
    pub fn build(self) -> Result<FlashData, Error> {
        FlashData::new(
            self.bootloader_path,
            self.partition_table_path,
            self.partition_table_offset,
            self.target_app_partition,
            self.flash_settings,
            self.min_chip_rev,
        )
    }
}

/// Flash data and configuration
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct FlashData {
    pub bootloader: Option<Vec<u8>>,
    pub partition_table: Option<PartitionTable>,
    pub partition_table_offset: Option<u32>,
    pub target_app_partition: Option<String>,
    pub flash_settings: FlashSettings,
    pub min_chip_rev: u16,
}

impl FlashData {
    pub fn new(
        bootloader: Option<&Path>,
        partition_table: Option<&Path>,
        partition_table_offset: Option<u32>,
        target_app_partition: Option<String>,
        flash_settings: FlashSettings,
        min_chip_rev: u16,
    ) -> Result<Self, Error> {
        // If the '--bootloader' option is provided, load the binary file at the
        // specified path.
        let bootloader = if let Some(path) = bootloader {
            let data = fs::canonicalize(path)
                .and_then(fs::read)
                .map_err(|e| Error::FileOpenError(path.display().to_string(), e))?;

            Some(data)
        } else {
            None
        };

        // If the '-T' option is provided, load the partition table from
        // the CSV or binary file at the specified path.
        let partition_table = match partition_table {
            Some(path) => Some(parse_partition_table(path)?),
            None => None,
        };

        Ok(FlashData {
            bootloader,
            partition_table,
            partition_table_offset,
            target_app_partition,
            flash_settings,
            min_chip_rev,
        })
    }
}

/// Parameters for attaching to a target devices SPI flash
#[derive(Copy, Clone, Debug)]
#[repr(C)]
pub struct SpiAttachParams {
    clk: u8,
    q: u8,
    d: u8,
    hd: u8,
    cs: u8,
}

impl SpiAttachParams {
    pub const fn default() -> Self {
        SpiAttachParams {
            clk: 0,
            q: 0,
            d: 0,
            hd: 0,
            cs: 0,
        }
    }

    // Default SPI parameters for ESP32-PICO-D4
    pub const fn esp32_pico_d4() -> Self {
        SpiAttachParams {
            clk: 6,
            q: 17,
            d: 8,
            hd: 11,
            cs: 16,
        }
    }

    /// Encode the parameters into a byte array
    pub fn encode(self, stub: bool) -> Vec<u8> {
        let packed = ((self.hd as u32) << 24)
            | ((self.cs as u32) << 18)
            | ((self.d as u32) << 12)
            | ((self.q as u32) << 6)
            | (self.clk as u32);

        let mut encoded: Vec<u8> = packed.to_le_bytes().to_vec();

        if !stub {
            encoded.append(&mut vec![0u8; 4]);
        }

        encoded
    }
}

/// Parse a [PartitionTable] from the provided path
pub fn parse_partition_table(path: &Path) -> Result<PartitionTable, Error> {
    let data = fs::read(path).map_err(|e| Error::FileOpenError(path.display().to_string(), e))?;

    Ok(PartitionTable::try_from(data)?)
}

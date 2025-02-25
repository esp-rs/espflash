//! Flashable target devices
//!
//! All ESP32 devices support booting via the ESP-IDF bootloader. It's also
//! possible to write an application to and boot from RAM, where a bootloader is
//! obviously not required either.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use strum::{Display, EnumIter, EnumString, VariantNames};
use xmas_elf::ElfFile;

#[cfg(feature = "serialport")]
pub use self::flash_target::{Esp32Target, RamTarget};
#[cfg(feature = "serialport")]
use crate::{
    connection::Connection,
    flasher::{SpiAttachParams, FLASH_WRITE_SIZE},
    targets::flash_target::{FlashTarget, MAX_RAM_BLOCK_SIZE},
};
use crate::{
    flasher::{FlashData, FlashFrequency},
    image_format::IdfBootloaderFormat,
    targets::{
        esp32::Esp32,
        esp32c2::Esp32c2,
        esp32c3::Esp32c3,
        esp32c6::Esp32c6,
        esp32h2::Esp32h2,
        esp32p4::Esp32p4,
        esp32s2::Esp32s2,
        esp32s3::Esp32s3,
    },
    Error,
};

mod esp32;
mod esp32c2;
pub(crate) mod esp32c3;
mod esp32c6;
mod esp32h2;
pub(crate) mod esp32p4;
pub(crate) mod esp32s2;
pub(crate) mod esp32s3;

#[cfg(feature = "serialport")]
pub(crate) mod flash_target;

/// Supported crystal frequencies
///
/// Note that not all frequencies are supported by each target device.
#[cfg_attr(feature = "cli", derive(clap::ValueEnum))]
#[derive(
    Debug, Default, Clone, Copy, Hash, PartialEq, Eq, Display, VariantNames, Serialize, Deserialize,
)]
#[non_exhaustive]
#[repr(u32)]
pub enum XtalFrequency {
    #[strum(serialize = "26 MHz")]
    /// 26 MHz
    _26Mhz,
    #[strum(serialize = "32 MHz")]
    /// 32 MHz
    _32Mhz,
    #[strum(serialize = "40 MHz")]
    /// 40 MHz
    #[default]
    _40Mhz,
}

impl XtalFrequency {
    pub fn default(chip: Chip) -> Self {
        match chip {
            Chip::Esp32 => Self::_40Mhz,
            Chip::Esp32c2 => Self::_40Mhz,
            Chip::Esp32c3 => Self::_40Mhz,
            Chip::Esp32c6 => Self::_40Mhz,
            Chip::Esp32h2 => Self::_32Mhz,
            Chip::Esp32p4 => Self::_40Mhz,
            Chip::Esp32s2 => Self::_40Mhz,
            Chip::Esp32s3 => Self::_40Mhz,
        }
    }
}

/// All supported devices
#[cfg_attr(feature = "cli", derive(clap::ValueEnum))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Display, EnumIter, EnumString, VariantNames)]
#[non_exhaustive]
#[strum(serialize_all = "lowercase")]
pub enum Chip {
    /// ESP32
    Esp32,
    /// ESP32-C2, ESP8684
    Esp32c2,
    /// ESP32-C3, ESP8685
    Esp32c3,
    /// ESP32-C6
    Esp32c6,
    /// ESP32-H2
    Esp32h2,
    /// ESP32-P4
    Esp32p4,
    /// ESP32-S2
    Esp32s2,
    /// ESP32-S3
    Esp32s3,
}

impl Chip {
    pub fn from_magic(magic: u32) -> Result<Self, Error> {
        if Esp32::has_magic_value(magic) {
            Ok(Chip::Esp32)
        } else if Esp32c2::has_magic_value(magic) {
            Ok(Chip::Esp32c2)
        } else if Esp32c3::has_magic_value(magic) {
            Ok(Chip::Esp32c3)
        } else if Esp32c6::has_magic_value(magic) {
            Ok(Chip::Esp32c6)
        } else if Esp32h2::has_magic_value(magic) {
            Ok(Chip::Esp32h2)
        } else if Esp32p4::has_magic_value(magic) {
            Ok(Chip::Esp32p4)
        } else if Esp32s2::has_magic_value(magic) {
            Ok(Chip::Esp32s2)
        } else if Esp32s3::has_magic_value(magic) {
            Ok(Chip::Esp32s3)
        } else {
            Err(Error::ChipDetectError(magic))
        }
    }

    pub fn into_target(&self) -> Box<dyn Target> {
        match self {
            Chip::Esp32 => Box::new(Esp32),
            Chip::Esp32c2 => Box::new(Esp32c2),
            Chip::Esp32c3 => Box::new(Esp32c3),
            Chip::Esp32c6 => Box::new(Esp32c6),
            Chip::Esp32h2 => Box::new(Esp32h2),
            Chip::Esp32p4 => Box::new(Esp32p4),
            Chip::Esp32s2 => Box::new(Esp32s2),
            Chip::Esp32s3 => Box::new(Esp32s3),
        }
    }

    #[cfg(feature = "serialport")]
    pub fn flash_target(
        &self,
        spi_params: SpiAttachParams,
        use_stub: bool,
        verify: bool,
        skip: bool,
    ) -> Box<dyn FlashTarget> {
        Box::new(Esp32Target::new(*self, spi_params, use_stub, verify, skip))
    }

    #[cfg(feature = "serialport")]
    pub fn ram_target(
        &self,
        entry: Option<u32>,
        max_ram_block_size: usize,
    ) -> Box<dyn FlashTarget> {
        Box::new(RamTarget::new(entry, max_ram_block_size))
    }
}

/// Device-specific parameters
#[derive(Debug, Clone, Copy)]
pub struct Esp32Params {
    pub boot_addr: u32,
    pub partition_addr: u32,
    pub nvs_addr: u32,
    pub nvs_size: u32,
    pub phy_init_data_addr: u32,
    pub phy_init_data_size: u32,
    pub app_addr: u32,
    pub app_size: u32,
    pub chip_id: u16,
    pub flash_freq: FlashFrequency,
    pub default_bootloader: &'static [u8],
}

impl Esp32Params {
    pub const fn new(
        boot_addr: u32,
        app_addr: u32,
        app_size: u32,
        chip_id: u16,
        flash_freq: FlashFrequency,
        bootloader: &'static [u8],
    ) -> Self {
        Self {
            boot_addr,
            partition_addr: 0x8000,
            nvs_addr: 0x9000,
            nvs_size: 0x6000,
            phy_init_data_addr: 0xf000,
            phy_init_data_size: 0x1000,
            app_addr,
            app_size,
            chip_id,
            flash_freq,
            default_bootloader: bootloader,
        }
    }
}

/// SPI register addresses
#[derive(Debug)]
pub struct SpiRegisters {
    base: u32,
    usr_offset: u32,
    usr1_offset: u32,
    usr2_offset: u32,
    w0_offset: u32,
    mosi_length_offset: Option<u32>,
    miso_length_offset: Option<u32>,
}

impl SpiRegisters {
    pub fn cmd(&self) -> u32 {
        self.base
    }

    pub fn usr(&self) -> u32 {
        self.base + self.usr_offset
    }

    pub fn usr1(&self) -> u32 {
        self.base + self.usr1_offset
    }

    pub fn usr2(&self) -> u32 {
        self.base + self.usr2_offset
    }

    pub fn w0(&self) -> u32 {
        self.base + self.w0_offset
    }

    pub fn mosi_length(&self) -> Option<u32> {
        self.mosi_length_offset.map(|offset| self.base + offset)
    }

    pub fn miso_length(&self) -> Option<u32> {
        self.miso_length_offset.map(|offset| self.base + offset)
    }
}

/// Enable the reading of eFuses for a target
pub trait ReadEFuse {
    /// Returns the base address of the eFuse register
    fn efuse_reg(&self) -> u32;

    #[cfg(feature = "serialport")]
    /// Given an active connection, read the nth word of the eFuse region
    fn read_efuse(&self, connection: &mut Connection, n: u32) -> Result<u32, Error> {
        let reg = self.efuse_reg() + (n * 0x4);
        connection.read_reg(reg)
    }
}

/// Operations for interacting with supported target devices
pub trait Target: ReadEFuse {
    /// Is the provided address `addr` in flash?
    fn addr_is_flash(&self, addr: u32) -> bool;

    #[cfg(feature = "serialport")]
    /// Enumerate the chip's features, read from eFuse
    fn chip_features(&self, connection: &mut Connection) -> Result<Vec<&str>, Error>;

    #[cfg(feature = "serialport")]
    /// Determine the chip's revision number
    fn chip_revision(&self, connection: &mut Connection) -> Result<(u32, u32), Error> {
        let major = self.major_chip_version(connection)?;
        let minor = self.minor_chip_version(connection)?;

        Ok((major, minor))
    }

    #[cfg(feature = "serialport")]
    fn major_chip_version(&self, connection: &mut Connection) -> Result<u32, Error>;

    #[cfg(feature = "serialport")]
    fn minor_chip_version(&self, connection: &mut Connection) -> Result<u32, Error>;

    #[cfg(feature = "serialport")]
    /// What is the crystal frequency?
    fn crystal_freq(&self, connection: &mut Connection) -> Result<XtalFrequency, Error>;

    /// Numeric encodings for the flash frequencies supported by a chip
    fn flash_frequency_encodings(&self) -> HashMap<FlashFrequency, u8> {
        use FlashFrequency::*;

        let encodings = [(_20Mhz, 0x2), (_26Mhz, 0x1), (_40Mhz, 0x0), (_80Mhz, 0xf)];

        HashMap::from(encodings)
    }

    #[cfg(feature = "serialport")]
    /// Write size for flashing operations
    fn flash_write_size(&self, _connection: &mut Connection) -> Result<usize, Error> {
        Ok(FLASH_WRITE_SIZE)
    }

    /// Build an image from the provided data for flashing
    fn get_flash_image<'a>(
        &self,
        elf: ElfFile<'a>,
        flash_data: FlashData,
        chip_revision: Option<(u32, u32)>,
        xtal_freq: XtalFrequency,
    ) -> Result<IdfBootloaderFormat<'a>, Error>;

    #[cfg(feature = "serialport")]
    /// What is the MAC address?
    fn mac_address(&self, connection: &mut Connection) -> Result<String, Error> {
        let word5 = self.read_efuse(connection, 17)?;
        let word6 = self.read_efuse(connection, 18)?;

        let bytes = ((word6 as u64) << 32) | word5 as u64;
        let bytes = bytes.to_be_bytes();
        let bytes = &bytes[2..];

        Ok(bytes_to_mac_addr(bytes))
    }

    #[cfg(feature = "serialport")]
    /// Maximum RAM block size for writing
    fn max_ram_block_size(&self, _connection: &mut Connection) -> Result<usize, Error> {
        Ok(MAX_RAM_BLOCK_SIZE)
    }

    /// SPI register addresses for a chip
    fn spi_registers(&self) -> SpiRegisters;

    /// Build targets supported by a chip
    fn supported_build_targets(&self) -> &[&str];

    /// Is the build target `target` supported by the chip?
    fn supports_build_target(&self, target: &str) -> bool {
        self.supported_build_targets().contains(&target)
    }
}

#[cfg(feature = "serialport")]
fn bytes_to_mac_addr(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|b| format!("{:02x}", b))
        .collect::<Vec<_>>()
        .join(":")
}

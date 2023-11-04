//! Flashable target devices
//!
//! Different devices support different boot options; the ESP8266 does not use a
//! second-stage bootloader, nor does direct boot (only supported by certain
//! devices). All ESP32* devices support booting via the ESP-IDF bootloader.
//! It's also possible to write an application to and boot from RAM, where a
//! bootloader is obviously not required either.

use std::collections::HashMap;

use esp_idf_part::{AppType, DataType, Partition, PartitionTable, SubType, Type};
use strum::{Display, EnumIter, EnumString, EnumVariantNames};

use self::flash_target::MAX_RAM_BLOCK_SIZE;
pub use self::{
    esp32::Esp32,
    esp32c2::Esp32c2,
    esp32c3::Esp32c3,
    esp32c6::Esp32c6,
    esp32h2::Esp32h2,
    esp32s2::Esp32s2,
    esp32s3::Esp32s3,
    esp8266::Esp8266,
    flash_target::{Esp32Target, Esp8266Target, FlashTarget, RamTarget},
};
use crate::{
    connection::Connection,
    elf::FirmwareImage,
    error::Error,
    flasher::{FlashFrequency, FlashMode, FlashSize, SpiAttachParams, FLASH_WRITE_SIZE},
    image_format::{ImageFormat, ImageFormatKind},
};

/// Max partition size is 16 MB
const MAX_PARTITION_SIZE: u32 = 16 * 1000 * 1024;

mod esp32;
mod esp32c2;
mod esp32c3;
mod esp32c6;
mod esp32h2;
mod esp32s2;
mod esp32s3;
mod esp8266;
mod flash_target;

/// All supported devices
#[cfg_attr(feature = "cli", derive(clap::ValueEnum))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Display, EnumIter, EnumString, EnumVariantNames)]
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
    /// ESP32-S2
    Esp32s2,
    /// ESP32-S3
    Esp32s3,
    /// ESP32-H2
    Esp32h2,
    /// ESP8266
    Esp8266,
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
        } else if Esp32s2::has_magic_value(magic) {
            Ok(Chip::Esp32s2)
        } else if Esp32s3::has_magic_value(magic) {
            Ok(Chip::Esp32s3)
        } else if Esp32h2::has_magic_value(magic) {
            Ok(Chip::Esp32h2)
        } else if Esp8266::has_magic_value(magic) {
            Ok(Chip::Esp8266)
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
            Chip::Esp32s2 => Box::new(Esp32s2),
            Chip::Esp32s3 => Box::new(Esp32s3),
            Chip::Esp32h2 => Box::new(Esp32h2),
            Chip::Esp8266 => Box::new(Esp8266),
        }
    }

    pub fn flash_target(
        &self,
        spi_params: SpiAttachParams,
        use_stub: bool,
    ) -> Box<dyn FlashTarget> {
        match self {
            Chip::Esp8266 => Box::new(Esp8266Target::new()),
            _ => Box::new(Esp32Target::new(*self, spi_params, use_stub)),
        }
    }

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

    /// Generates a default partition table.
    ///
    /// `flash_size` is used to scale app partition when present, otherwise the
    /// paramameter defaults are used.
    pub fn default_partition_table(&self, flash_size: Option<u32>) -> PartitionTable {
        PartitionTable::new(vec![
            Partition::new(
                String::from("nvs"),
                Type::Data,
                SubType::Data(DataType::Nvs),
                self.nvs_addr,
                self.nvs_size,
                false,
            ),
            Partition::new(
                String::from("phy_init"),
                Type::Data,
                SubType::Data(DataType::Phy),
                self.phy_init_data_addr,
                self.phy_init_data_size,
                false,
            ),
            Partition::new(
                String::from("factory"),
                Type::App,
                SubType::App(AppType::Factory),
                self.app_addr,
                core::cmp::min(
                    flash_size.map_or(self.app_size, |size| size - self.app_addr),
                    MAX_PARTITION_SIZE,
                ),
                false,
            ),
        ])
    }
}

/// SPI register addresses
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

    /// Enumerate the chip's features, read from eFuse
    fn chip_features(&self, connection: &mut Connection) -> Result<Vec<&str>, Error>;

    /// Determine the chip's revision number
    fn chip_revision(&self, connection: &mut Connection) -> Result<(u32, u32), Error> {
        let major = self.major_chip_version(connection)?;
        let minor = self.minor_chip_version(connection)?;

        Ok((major, minor))
    }

    fn major_chip_version(&self, connection: &mut Connection) -> Result<u32, Error>;

    fn minor_chip_version(&self, connection: &mut Connection) -> Result<u32, Error>;

    /// What is the crystal frequency?
    fn crystal_freq(&self, connection: &mut Connection) -> Result<u32, Error>;

    /// Numeric encodings for the flash frequencies supported by a chip
    fn flash_frequency_encodings(&self) -> HashMap<FlashFrequency, u8> {
        use FlashFrequency::*;

        let encodings = [(_20Mhz, 0x2), (_26Mhz, 0x1), (_40Mhz, 0x0), (_80Mhz, 0xf)];

        HashMap::from(encodings)
    }

    /// Write size for flashing operations
    fn flash_write_size(&self, _connection: &mut Connection) -> Result<usize, Error> {
        Ok(FLASH_WRITE_SIZE)
    }

    /// Build an image from the provided data for flashing
    fn get_flash_image<'a>(
        &self,
        image: &'a dyn FirmwareImage<'a>,
        bootloader: Option<Vec<u8>>,
        partition_table: Option<PartitionTable>,
        target_app_partition: Option<String>,
        image_format: Option<ImageFormatKind>,
        chip_revision: Option<(u32, u32)>,
        flash_mode: Option<FlashMode>,
        flash_size: Option<FlashSize>,
        flash_freq: Option<FlashFrequency>,
    ) -> Result<Box<dyn ImageFormat<'a> + 'a>, Error>;

    /// What is the MAC address?
    fn mac_address(&self, connection: &mut Connection) -> Result<String, Error> {
        let word5 = self.read_efuse(connection, 17)?;
        let word6 = self.read_efuse(connection, 18)?;

        let bytes = ((word6 as u64) << 32) | word5 as u64;
        let bytes = bytes.to_be_bytes();
        let bytes = &bytes[2..];

        Ok(bytes_to_mac_addr(bytes))
    }

    /// Maximum RAM block size for writing
    fn max_ram_block_size(&self, _connection: &mut Connection) -> Result<usize, Error> {
        Ok(MAX_RAM_BLOCK_SIZE)
    }

    /// SPI register addresses for a chip
    fn spi_registers(&self) -> SpiRegisters;

    /// Image formats supported by a chip
    fn supported_image_formats(&self) -> &[ImageFormatKind] {
        &[ImageFormatKind::EspBootloader]
    }

    /// Build targets supported by a chip
    fn supported_build_targets(&self) -> &[&str];

    /// Is the build target `target` supported by the chip?
    fn supports_build_target(&self, target: &str) -> bool {
        self.supported_build_targets().contains(&target)
    }
}

fn bytes_to_mac_addr(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|b| format!("{:02x}", b))
        .collect::<Vec<_>>()
        .join(":")
}

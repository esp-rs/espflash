use std::{collections::HashMap, str::FromStr};

use esp_idf_part::{AppType, DataType, Partition, PartitionTable, SubType, Type};
use strum::{Display, EnumIter, EnumVariantNames};

use self::flash_target::MAX_RAM_BLOCK_SIZE;
pub use self::{
    esp32::Esp32,
    esp32c2::Esp32c2,
    esp32c3::Esp32c3,
    esp32s2::Esp32s2,
    esp32s3::Esp32s3,
    esp8266::Esp8266,
    flash_target::{Esp32Target, Esp8266Target, FlashTarget, RamTarget},
};
use crate::{
    connection::Connection,
    elf::FirmwareImage,
    error::{ChipDetectError, Error},
    flasher::{FlashFrequency, FlashMode, FlashSize, SpiAttachParams, FLASH_WRITE_SIZE},
    image_format::{ImageFormat, ImageFormatId},
};

mod esp32;
mod esp32c2;
mod esp32c3;
mod esp32s2;
mod esp32s3;
mod esp8266;
mod flash_target;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Display, EnumIter, EnumVariantNames)]
#[strum(serialize_all = "lowercase")]
pub enum Chip {
    Esp32,
    Esp32c2,
    Esp32c3,
    Esp32s2,
    Esp32s3,
    Esp8266,
}

impl FromStr for Chip {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        use Chip::*;

        match s.to_lowercase().replace('-', "").as_str() {
            "esp32" => Ok(Esp32),
            "esp32c2" => Ok(Esp32c2),
            "esp32c3" => Ok(Esp32c3),
            "esp32s2" => Ok(Esp32s2),
            "esp32s3" => Ok(Esp32s3),
            "esp8266" => Ok(Esp8266),
            _ => Err(Error::UnrecognizedChipName),
        }
    }
}

impl Chip {
    pub fn from_magic(magic: u32) -> Result<Self, ChipDetectError> {
        if Esp32::has_magic_value(magic) {
            Ok(Chip::Esp32)
        } else if Esp32c2::has_magic_value(magic) {
            Ok(Chip::Esp32c2)
        } else if Esp32c3::has_magic_value(magic) {
            Ok(Chip::Esp32c3)
        } else if Esp32s2::has_magic_value(magic) {
            Ok(Chip::Esp32s2)
        } else if Esp32s3::has_magic_value(magic) {
            Ok(Chip::Esp32s3)
        } else if Esp8266::has_magic_value(magic) {
            Ok(Chip::Esp8266)
        } else {
            Err(ChipDetectError::from(magic))
        }
    }

    pub fn into_target(&self) -> Box<dyn Target> {
        match self {
            Chip::Esp32 => Box::new(Esp32),
            Chip::Esp32c2 => Box::new(Esp32c2),
            Chip::Esp32c3 => Box::new(Esp32c3),
            Chip::Esp32s2 => Box::new(Esp32s2),
            Chip::Esp32s3 => Box::new(Esp32s3),
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
    pub default_bootloader: &'static [u8],
}

impl Esp32Params {
    pub const fn new(
        boot_addr: u32,
        app_addr: u32,
        app_size: u32,
        chip_id: u16,
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
            default_bootloader: bootloader,
        }
    }

    /// Generates a default partition table.
    /// `flash_size` is used to scale app partition when present, otherwise the
    /// param defaults are used.
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
                flash_size.map_or(self.app_size, |size| size - self.app_addr),
                false,
            ),
        ])
    }
}

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

pub trait Target: ReadEFuse {
    fn addr_is_flash(&self, addr: u32) -> bool;

    fn chip_features(&self, connection: &mut Connection) -> Result<Vec<&str>, Error>;

    fn chip_revision(&self, _connection: &mut Connection) -> Result<Option<u32>, Error> {
        Ok(None)
    }

    fn crystal_freq(&self, connection: &mut Connection) -> Result<u32, Error>;

    fn flash_frequency_encodings(&self) -> HashMap<FlashFrequency, u8> {
        use FlashFrequency::*;

        let encodings = [
            (Flash20M, 0x2),
            (Flash26M, 0x1),
            (Flash40M, 0x0),
            (Flash80M, 0xf),
        ];

        HashMap::from(encodings)
    }

    fn flash_write_size(&self, _connection: &mut Connection) -> Result<usize, Error> {
        Ok(FLASH_WRITE_SIZE)
    }

    fn get_flash_image<'a>(
        &self,
        image: &'a dyn FirmwareImage<'a>,
        bootloader: Option<Vec<u8>>,
        partition_table: Option<PartitionTable>,
        image_format: Option<ImageFormatId>,
        chip_revision: Option<u32>,
        flash_mode: Option<FlashMode>,
        flash_size: Option<FlashSize>,
        flash_freq: Option<FlashFrequency>,
    ) -> Result<Box<dyn ImageFormat<'a> + 'a>, Error>;

    fn mac_address(&self, connection: &mut Connection) -> Result<String, Error> {
        let word5 = self.read_efuse(connection, 5)?;
        let word6 = self.read_efuse(connection, 6)?;

        let bytes = ((word6 as u64) << 32) | word5 as u64;
        let bytes = bytes.to_be_bytes();
        let bytes = &bytes[2..];

        Ok(bytes_to_mac_addr(bytes))
    }

    fn max_ram_block_size(&self, _connection: &mut Connection) -> Result<usize, Error> {
        Ok(MAX_RAM_BLOCK_SIZE)
    }

    fn spi_registers(&self) -> SpiRegisters;

    fn supported_image_formats(&self) -> &[ImageFormatId] {
        &[ImageFormatId::Bootloader]
    }

    fn supported_build_targets(&self) -> &[&str];

    fn supports_build_target(&self, target: &str) -> bool {
        self.supported_build_targets().contains(&target)
    }
}

pub(crate) fn bytes_to_mac_addr(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|b| format!("{:02x}", b))
        .collect::<Vec<_>>()
        .join(":")
}

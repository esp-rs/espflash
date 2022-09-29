use std::ops::Range;

use esp_idf_part::PartitionTable;

use super::{bytes_to_mac_addr, Chip, Esp32Params, ReadEFuse, SpiRegisters, Target};
use crate::{
    connection::Connection,
    elf::FirmwareImage,
    error::Error,
    flasher::{FlashFrequency, FlashMode, FlashSize},
    image_format::{DirectBootFormat, IdfBootloaderFormat, ImageFormat, ImageFormatId},
};

pub(crate) const CHIP_DETECT_MAGIC_VALUES: &[u32] = &[0x9];

pub(crate) const FLASH_RANGES: &[Range<u32>] = &[
    0x4200_0000..0x4400_0000, // IROM
    0x3c00_0000..0x3e00_0000, // DROM
];

pub(crate) const PARAMS: Esp32Params = Esp32Params::new(
    0x0,
    0x1_0000,
    0x10_0000,
    9,
    include_bytes!("../../resources/bootloaders/esp32s3-bootloader.bin"),
);

pub struct Esp32s3;

impl Esp32s3 {
    pub fn has_magic_value(value: u32) -> bool {
        CHIP_DETECT_MAGIC_VALUES.contains(&value)
    }
}

impl ReadEFuse for Esp32s3 {
    fn efuse_reg(&self) -> u32 {
        0x6000_7030
    }
}

impl Target for Esp32s3 {
    fn addr_is_flash(&self, addr: u32) -> bool {
        FLASH_RANGES.iter().any(|range| range.contains(&addr))
    }

    fn chip_features(&self, _connection: &mut Connection) -> Result<Vec<&str>, Error> {
        Ok(vec!["WiFi", "BLE"])
    }

    fn crystal_freq(&self, _connection: &mut Connection) -> Result<u32, Error> {
        // The ESP32-S3's XTAL has a fixed frequency of 40MHz.
        Ok(40)
    }

    fn get_flash_image<'a>(
        &self,
        image: &'a dyn FirmwareImage<'a>,
        bootloader: Option<Vec<u8>>,
        partition_table: Option<PartitionTable>,
        image_format: Option<ImageFormatId>,
        _chip_revision: Option<u32>,
        flash_mode: Option<FlashMode>,
        flash_size: Option<FlashSize>,
        flash_freq: Option<FlashFrequency>,
    ) -> Result<Box<dyn ImageFormat<'a> + 'a>, Error> {
        let image_format = image_format.unwrap_or(ImageFormatId::Bootloader);

        match image_format {
            ImageFormatId::Bootloader => Ok(Box::new(IdfBootloaderFormat::new(
                image,
                Chip::Esp32s3,
                PARAMS,
                partition_table,
                bootloader,
                flash_mode,
                flash_size,
                flash_freq,
            )?)),
            ImageFormatId::DirectBoot => Ok(Box::new(DirectBootFormat::new(image, 0x400)?)),
        }
    }

    fn mac_address(&self, connection: &mut Connection) -> Result<String, Error> {
        let word5 = self.read_efuse(connection, 5)?;
        let word6 = self.read_efuse(connection, 6)?;

        let bytes = ((word6 as u64) << 32) | word5 as u64;
        let bytes = bytes.to_be_bytes();
        let bytes = &bytes[2..];

        Ok(bytes_to_mac_addr(bytes))
    }

    fn spi_registers(&self) -> SpiRegisters {
        SpiRegisters {
            base: 0x6000_2000,
            usr_offset: 0x18,
            usr1_offset: 0x1C,
            usr2_offset: 0x20,
            w0_offset: 0x58,
            mosi_length_offset: Some(0x24),
            miso_length_offset: Some(0x28),
        }
    }

    fn supported_image_formats(&self) -> &[ImageFormatId] {
        &[ImageFormatId::Bootloader, ImageFormatId::DirectBoot]
    }

    fn supported_build_targets(&self) -> &[&str] {
        &["xtensa-esp32s3-none-elf", "xtensa-esp32s3-espidf"]
    }
}

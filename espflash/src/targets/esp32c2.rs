use std::{collections::HashMap, ops::Range};

use esp_idf_part::PartitionTable;

use super::{bytes_to_mac_addr, Chip, Esp32Params, ReadEFuse, SpiRegisters, Target};
use crate::{
    connection::Connection,
    elf::FirmwareImage,
    error::Error,
    flasher::{FlashFrequency, FlashMode, FlashSize},
    image_format::{DirectBootFormat, IdfBootloaderFormat, ImageFormat, ImageFormatKind},
};

const CHIP_DETECT_MAGIC_VALUES: &[u32] = &[
    0x6f51_306f, // ECO0
    0x7c41_a06f, // ECO1
];

const FLASH_RANGES: &[Range<u32>] = &[
    0x4200_0000..0x4240_0000, // IROM
    0x3c00_0000..0x3c40_0000, // DROM
];

const PARAMS: Esp32Params = Esp32Params::new(
    0x0,
    0x1_0000,
    0x1f_0000,
    12,
    include_bytes!("../../resources/bootloaders/esp32c2-bootloader.bin"),
);

/// ESP32-C2 Target
pub struct Esp32c2;

impl Esp32c2 {
    pub fn has_magic_value(value: u32) -> bool {
        CHIP_DETECT_MAGIC_VALUES.contains(&value)
    }
}

impl ReadEFuse for Esp32c2 {
    fn efuse_reg(&self) -> u32 {
        0x6000_8800
    }
}

impl Target for Esp32c2 {
    fn addr_is_flash(&self, addr: u32) -> bool {
        FLASH_RANGES.iter().any(|range| range.contains(&addr))
    }

    fn chip_features(&self, _connection: &mut Connection) -> Result<Vec<&str>, Error> {
        Ok(vec!["WiFi", "BLE"])
    }

    fn major_chip_version(&self, connection: &mut Connection) -> Result<u32, Error> {
        Ok(self.read_efuse(connection, 17)? >> 20 & 0x3)
    }

    fn minor_chip_version(&self, connection: &mut Connection) -> Result<u32, Error> {
        Ok(self.read_efuse(connection, 17)? >> 16 & 0xf)
    }

    fn crystal_freq(&self, _connection: &mut Connection) -> Result<u32, Error> {
        // The ESP32-C2's XTAL has a fixed frequency of 40MHz.
        Ok(40)
    }

    fn flash_frequency_encodings(&self) -> HashMap<FlashFrequency, u8> {
        use FlashFrequency::*;

        let encodings = [
            (Flash15M, 0x2),
            (Flash20M, 0x1),
            (Flash30M, 0x0),
            (Flash60M, 0xF),
        ];

        HashMap::from(encodings)
    }

    fn get_flash_image<'a>(
        &self,
        image: &'a dyn FirmwareImage<'a>,
        bootloader: Option<Vec<u8>>,
        partition_table: Option<PartitionTable>,
        image_format: Option<ImageFormatKind>,
        _chip_revision: Option<(u32, u32)>,
        flash_mode: Option<FlashMode>,
        flash_size: Option<FlashSize>,
        flash_freq: Option<FlashFrequency>,
    ) -> Result<Box<dyn ImageFormat<'a> + 'a>, Error> {
        let image_format = image_format.unwrap_or(ImageFormatKind::EspBootloader);

        match image_format {
            ImageFormatKind::EspBootloader => Ok(Box::new(IdfBootloaderFormat::new(
                image,
                Chip::Esp32c2,
                PARAMS,
                partition_table,
                bootloader,
                flash_mode,
                flash_size,
                flash_freq,
            )?)),
            ImageFormatKind::DirectBoot => Ok(Box::new(DirectBootFormat::new(image, 0)?)),
        }
    }

    /// What is the MAC address?
    fn mac_address(&self, connection: &mut Connection) -> Result<String, Error> {
        let word5 = self.read_efuse(connection, 16)?;
        let word6 = self.read_efuse(connection, 17)?;

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

    fn supported_image_formats(&self) -> &[ImageFormatKind] {
        &[ImageFormatKind::EspBootloader, ImageFormatKind::DirectBoot]
    }

    fn supported_build_targets(&self) -> &[&str] {
        &[
            "riscv32imac-unknown-none-elf",
            "riscv32imc-esp-espidf",
            "riscv32imc-unknown-none-elf",
        ]
    }
}

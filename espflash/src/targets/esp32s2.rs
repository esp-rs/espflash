use std::ops::Range;

use esp_idf_part::PartitionTable;

use crate::{
    connection::Connection,
    elf::FirmwareImage,
    error::{Error, UnsupportedImageFormatError},
    flasher::{FlashFrequency, FlashMode, FlashSize, FLASH_WRITE_SIZE},
    image_format::{IdfBootloaderFormat, ImageFormat, ImageFormatKind},
    targets::{Chip, Esp32Params, ReadEFuse, SpiRegisters, Target, MAX_RAM_BLOCK_SIZE},
};

const CHIP_DETECT_MAGIC_VALUES: &[u32] = &[0x0000_07c6];

const FLASH_RANGES: &[Range<u32>] = &[
    0x4008_0000..0x40b8_0000, // IROM
    0x3f00_0000..0x3f3f_0000, // DROM
];

const MAX_USB_BLOCK_SIZE: usize = 0x800;

const PARAMS: Esp32Params = Esp32Params::new(
    0x1000,
    0x1_0000,
    0x10_0000,
    2,
    FlashFrequency::_40Mhz,
    include_bytes!("../../resources/bootloaders/esp32s2-bootloader.bin"),
);

/// ESP32-S2 Target
pub struct Esp32s2;

impl Esp32s2 {
    /// Return if the connection is USB OTG
    fn connection_is_usb_otg(&self, connection: &mut Connection) -> Result<bool, Error> {
        const UARTDEV_BUF_NO: u32 = 0x3fff_fd14; // Address which indicates OTG in use
        const UARTDEV_BUF_NO_USB_OTG: u32 = 2; // Value of UARTDEV_BUF_NO when OTG is in use

        Ok(connection.read_reg(UARTDEV_BUF_NO)? == UARTDEV_BUF_NO_USB_OTG)
    }

    /// Return the block2 version based on eFuses
    fn get_block2_version(&self, connection: &mut Connection) -> Result<u32, Error> {
        let blk2_word4 = self.read_efuse(connection, 27)?;
        let block2_version = (blk2_word4 >> 4) & 0x7;

        Ok(block2_version)
    }

    /// Return the flash version based on eFuses
    fn get_flash_version(&self, connection: &mut Connection) -> Result<u32, Error> {
        let blk1_word3 = self.read_efuse(connection, 20)?;
        let flash_version = (blk1_word3 >> 21) & 0xf;

        Ok(flash_version)
    }

    /// Return the PSRAM version based on eFuses
    fn get_psram_version(&self, connection: &mut Connection) -> Result<u32, Error> {
        let blk1_word3 = self.read_efuse(connection, 20)?;
        let psram_version = (blk1_word3 >> 28) & 0xf;

        Ok(psram_version)
    }

    /// Check if the magic value contains the specified value
    pub fn has_magic_value(value: u32) -> bool {
        CHIP_DETECT_MAGIC_VALUES.contains(&value)
    }
}

impl ReadEFuse for Esp32s2 {
    fn efuse_reg(&self) -> u32 {
        0x3f41_a000
    }
}

impl Target for Esp32s2 {
    fn addr_is_flash(&self, addr: u32) -> bool {
        FLASH_RANGES.iter().any(|range| range.contains(&addr))
    }

    fn chip_features(&self, connection: &mut Connection) -> Result<Vec<&str>, Error> {
        let mut features = vec!["WiFi"];

        let flash_version = match self.get_flash_version(connection)? {
            0 => "No Embedded Flash",
            1 => "Embedded Flash 2MB",
            2 => "Embedded Flash 4MB",
            _ => "Unknown Embedded Flash",
        };
        features.push(flash_version);

        let psram_version = match self.get_psram_version(connection)? {
            0 => "No Embedded PSRAM",
            1 => "Embedded PSRAM 2MB",
            2 => "Embedded PSRAM 4MB",
            _ => "Unknown Embedded PSRAM",
        };
        features.push(psram_version);

        let block2_version = match self.get_block2_version(connection)? {
            0 => "No calibration in BLK2 of efuse",
            1 => "ADC and temperature sensor calibration in BLK2 of efuse V1",
            2 => "ADC and temperature sensor calibration in BLK2 of efuse V2",
            _ => "Unknown Calibration in BLK2",
        };
        features.push(block2_version);

        Ok(features)
    }

    fn major_chip_version(&self, connection: &mut Connection) -> Result<u32, Error> {
        Ok(self.read_efuse(connection, 20)? >> 18 & 0x3)
    }

    fn minor_chip_version(&self, connection: &mut Connection) -> Result<u32, Error> {
        let hi = self.read_efuse(connection, 20)? >> 20 & 0x1;
        let lo = self.read_efuse(connection, 21)? >> 4 & 0x7;

        Ok((hi << 3) + lo)
    }

    fn crystal_freq(&self, _connection: &mut Connection) -> Result<u32, Error> {
        // The ESP32-S2's XTAL has a fixed frequency of 40MHz.
        Ok(40)
    }

    fn flash_write_size(&self, connection: &mut Connection) -> Result<usize, Error> {
        Ok(if self.connection_is_usb_otg(connection)? {
            MAX_USB_BLOCK_SIZE
        } else {
            FLASH_WRITE_SIZE
        })
    }

    fn get_flash_image<'a>(
        &self,
        image: &'a dyn FirmwareImage<'a>,
        bootloader: Option<Vec<u8>>,
        partition_table: Option<PartitionTable>,
        target_app_partition: Option<String>,
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
                Chip::Esp32s2,
                PARAMS,
                partition_table,
                target_app_partition,
                bootloader,
                flash_mode,
                flash_size,
                flash_freq,
            )?)),
            _ => Err(UnsupportedImageFormatError::new(image_format, Chip::Esp32s2, None).into()),
        }
    }

    fn max_ram_block_size(&self, connection: &mut Connection) -> Result<usize, Error> {
        Ok(if self.connection_is_usb_otg(connection)? {
            MAX_USB_BLOCK_SIZE
        } else {
            MAX_RAM_BLOCK_SIZE
        })
    }

    fn spi_registers(&self) -> SpiRegisters {
        SpiRegisters {
            base: 0x3f40_2000,
            usr_offset: 0x18,
            usr1_offset: 0x1C,
            usr2_offset: 0x20,
            w0_offset: 0x58,
            mosi_length_offset: Some(0x24),
            miso_length_offset: Some(0x28),
        }
    }

    fn supported_build_targets(&self) -> &[&str] {
        &["xtensa-esp32s2-none-elf", "xtensa-esp32s2-espidf"]
    }
}

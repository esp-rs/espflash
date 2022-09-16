use std::ops::Range;

use super::Esp32Params;
use crate::{
    chip::{ChipType, ReadEFuse, SpiRegisters},
    connection::Connection,
    elf::{FirmwareImage, FlashFrequency, FlashMode},
    error::UnsupportedImageFormatError,
    flash_target::MAX_RAM_BLOCK_SIZE,
    flasher::{FlashSize, FLASH_WRITE_SIZE},
    image_format::{Esp32BootloaderFormat, ImageFormat, ImageFormatId},
    Chip, Error, PartitionTable,
};

const MAX_USB_BLOCK_SIZE: usize = 0x800;

pub struct Esp32s2;

pub const PARAMS: Esp32Params = Esp32Params::new(
    0x1000,
    0x10000,
    0x100000,
    2,
    include_bytes!("../../../resources/bootloaders/esp32s2-bootloader.bin"),
);

impl ChipType for Esp32s2 {
    const CHIP_DETECT_MAGIC_VALUES: &'static [u32] = &[0x000007c6];

    const UART_CLKDIV_REG: u32 = 0x3f400014;

    const SPI_REGISTERS: SpiRegisters = SpiRegisters {
        base: 0x3f402000,
        usr_offset: 0x18,
        usr1_offset: 0x1C,
        usr2_offset: 0x20,
        w0_offset: 0x58,
        mosi_length_offset: Some(0x24),
        miso_length_offset: Some(0x28),
    };

    const FLASH_RANGES: &'static [Range<u32>] = &[
        0x40080000..0x40b80000, // IROM
        0x3F000000..0x3F3F0000, // DROM
    ];

    const SUPPORTED_TARGETS: &'static [&'static str] =
        &["xtensa-esp32s2-none-elf", "xtensa-esp32s2-espidf"];

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

    fn max_ram_block_size(&self, connection: &mut Connection) -> Result<usize, Error> {
        Ok(if self.connection_is_usb_otg(connection)? {
            MAX_USB_BLOCK_SIZE
        } else {
            MAX_RAM_BLOCK_SIZE
        })
    }

    fn get_flash_segments<'a>(
        image: &'a dyn FirmwareImage<'a>,
        bootloader: Option<Vec<u8>>,
        partition_table: Option<PartitionTable>,
        image_format: ImageFormatId,
        _chip_revision: Option<u32>,
        flash_mode: Option<FlashMode>,
        flash_size: Option<FlashSize>,
        flash_freq: Option<FlashFrequency>,
    ) -> Result<Box<dyn ImageFormat<'a> + 'a>, Error> {
        match image_format {
            ImageFormatId::Bootloader => Ok(Box::new(Esp32BootloaderFormat::new(
                image,
                Chip::Esp32s2,
                PARAMS,
                partition_table,
                bootloader,
                flash_mode,
                flash_size,
                flash_freq,
            )?)),
            _ => Err(UnsupportedImageFormatError::new(image_format, Chip::Esp32s2, None).into()),
        }
    }
}

impl ReadEFuse for Esp32s2 {
    const EFUSE_REG_BASE: u32 = 0x3F41A030;
}

impl Esp32s2 {
    fn connection_is_usb_otg(&self, connection: &mut Connection) -> Result<bool, Error> {
        const UARTDEV_BUF_NO: u32 = 0x3FFFFD14; // Address which indicates OTG in use
        const UARTDEV_BUF_NO_USB_OTG: u32 = 2; // Value of UARTDEV_BUF_NO when OTG is in use

        Ok(connection.read_reg(UARTDEV_BUF_NO)? == UARTDEV_BUF_NO_USB_OTG)
    }

    fn get_flash_version(&self, connection: &mut Connection) -> Result<u32, Error> {
        let blk1_word3 = self.read_efuse(connection, 8)?;
        let flash_version = (blk1_word3 >> 21) & 0xf;

        Ok(flash_version)
    }

    fn get_psram_version(&self, connection: &mut Connection) -> Result<u32, Error> {
        let blk1_word3 = self.read_efuse(connection, 8)?;
        let psram_version = (blk1_word3 >> 28) & 0xf;

        Ok(psram_version)
    }

    fn get_block2_version(&self, connection: &mut Connection) -> Result<u32, Error> {
        let blk2_word4 = self.read_efuse(connection, 15)?;
        let block2_version = (blk2_word4 >> 4) & 0x7;

        Ok(block2_version)
    }
}

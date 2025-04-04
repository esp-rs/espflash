#[cfg(feature = "serialport")]
use std::collections::HashMap;
use std::ops::Range;

#[cfg(feature = "serialport")]
use super::flash_target::MAX_RAM_BLOCK_SIZE;
#[cfg(feature = "serialport")]
use crate::{connection::Connection, flasher::FLASH_WRITE_SIZE};
use crate::{
    flasher::{FlashData, FlashFrequency},
    image_format::IdfBootloaderFormat,
    targets::{Chip, EfuseField, Esp32Params, ReadEFuse, SpiRegisters, Target, XtalFrequency},
    Error,
};

pub(crate) const CHIP_ID: u16 = 2;

const CHIP_DETECT_MAGIC_VALUES: &[u32] = &[0x0000_07c6];

const FLASH_RANGES: &[Range<u32>] = &[
    0x4008_0000..0x40b8_0000, // IROM
    0x3f00_0000..0x3f3f_0000, // DROM
];

#[cfg(feature = "serialport")]
const MAX_USB_BLOCK_SIZE: usize = 0x800;

const PARAMS: Esp32Params = Esp32Params::new(
    0x1000,
    0x1_0000,
    0x10_0000,
    CHIP_ID,
    FlashFrequency::_40Mhz,
    include_bytes!("../../resources/bootloaders/esp32s2-bootloader.bin"),
);

/// ESP32-S2 Target
pub struct Esp32s2;

impl Esp32s2 {
    /// Return the block2 version based on eFuses
    #[cfg(feature = "serialport")]
    fn block2_version(&self, connection: &mut Connection) -> Result<u32, Error> {
        let fields = self.common_fields();
        self.read_field(connection, fields["BLOCK2_VERSION"])
    }

    /// Return the flash version based on eFuses
    #[cfg(feature = "serialport")]
    fn flash_version(&self, connection: &mut Connection) -> Result<u32, Error> {
        let fields = self.common_fields();
        self.read_field(connection, fields["FLASH_VERSION"])
    }

    /// Return the PSRAM version based on eFuses
    #[cfg(feature = "serialport")]
    fn psram_version(&self, connection: &mut Connection) -> Result<u32, Error> {
        let fields = self.common_fields();
        self.read_field(connection, fields["PSRAM_VERSION"])
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

    #[cfg(feature = "serialport")]
    fn common_fields(&self) -> HashMap<&'static str, EfuseField> {
        let mut fields = HashMap::new();

        // MAC address fields
        fields.insert(
            "MAC_FACTORY_0",
            EfuseField {
                word_offset: 17,
                bit_offset: 0,
                bit_count: 32,
            },
        );
        fields.insert(
            "MAC_FACTORY_1",
            EfuseField {
                word_offset: 18,
                bit_offset: 0,
                bit_count: 16,
            },
        );

        // Chip version fields
        fields.insert(
            "MAJOR_VERSION",
            EfuseField {
                word_offset: 20,
                bit_offset: 18,
                bit_count: 2,
            },
        );
        fields.insert(
            "MINOR_VERSION_HI",
            EfuseField {
                word_offset: 20,
                bit_offset: 20,
                bit_count: 1,
            },
        );
        fields.insert(
            "MINOR_VERSION_LO",
            EfuseField {
                word_offset: 21,
                bit_offset: 4,
                bit_count: 3,
            },
        );

        // Block version fields
        fields.insert(
            "BLOCK2_VERSION",
            EfuseField {
                word_offset: 20,
                bit_offset: 30,
                bit_count: 2,
            },
        );
        fields.insert(
            "PSRAM_VERSION",
            EfuseField {
                word_offset: 20,
                bit_offset: 28,
                bit_count: 2,
            },
        );
        fields.insert(
            "FLASH_VERSION",
            EfuseField {
                word_offset: 20,
                bit_offset: 26,
                bit_count: 2,
            },
        );

        fields
    }
}

impl Target for Esp32s2 {
    fn addr_is_flash(&self, addr: u32) -> bool {
        FLASH_RANGES.iter().any(|range| range.contains(&addr))
    }

    #[cfg(feature = "serialport")]
    fn chip_features(&self, connection: &mut Connection) -> Result<Vec<&str>, Error> {
        let mut features = vec!["WiFi"];

        let flash_version = match self.flash_version(connection)? {
            0 => "No Embedded Flash",
            1 => "Embedded Flash 2MB",
            2 => "Embedded Flash 4MB",
            _ => "Unknown Embedded Flash",
        };
        features.push(flash_version);

        let psram_version = match self.psram_version(connection)? {
            0 => "No Embedded PSRAM",
            1 => "Embedded PSRAM 2MB",
            2 => "Embedded PSRAM 4MB",
            _ => "Unknown Embedded PSRAM",
        };
        features.push(psram_version);

        let block2_version = match self.block2_version(connection)? {
            0 => "No calibration in BLK2 of efuse",
            1 => "ADC and temperature sensor calibration in BLK2 of efuse V1",
            2 => "ADC and temperature sensor calibration in BLK2 of efuse V2",
            _ => "Unknown Calibration in BLK2",
        };
        features.push(block2_version);

        Ok(features)
    }

    #[cfg(feature = "serialport")]
    fn major_chip_version(&self, connection: &mut Connection) -> Result<u32, Error> {
        let fields = self.common_fields();
        self.read_field(connection, fields["MAJOR_VERSION"])
    }

    #[cfg(feature = "serialport")]
    fn minor_chip_version(&self, connection: &mut Connection) -> Result<u32, Error> {
        let fields = self.common_fields();
        let hi = self.read_field(connection, fields["MINOR_VERSION_HI"])?;
        let lo = self.read_field(connection, fields["MINOR_VERSION_LO"])?;

        Ok((hi << 3) + lo)
    }

    #[cfg(feature = "serialport")]
    fn crystal_freq(&self, _connection: &mut Connection) -> Result<XtalFrequency, Error> {
        // The ESP32-S2's XTAL has a fixed frequency of 40MHz.
        Ok(XtalFrequency::_40Mhz)
    }

    #[cfg(feature = "serialport")]
    fn flash_write_size(&self, connection: &mut Connection) -> Result<usize, Error> {
        use super::UsbOtg;

        Ok(if self.is_using_usb_otg(connection)? {
            MAX_USB_BLOCK_SIZE
        } else {
            FLASH_WRITE_SIZE
        })
    }

    fn flash_image<'a>(
        &self,
        elf_data: &'a [u8],
        flash_data: FlashData,
        _chip_revision: Option<(u32, u32)>,
        xtal_freq: XtalFrequency,
    ) -> Result<IdfBootloaderFormat<'a>, Error> {
        if xtal_freq != XtalFrequency::_40Mhz {
            return Err(Error::UnsupportedFeature {
                chip: Chip::Esp32s2,
                feature: "the selected crystal frequency".into(),
            });
        }

        IdfBootloaderFormat::new(elf_data, Chip::Esp32s2, flash_data, PARAMS)
    }

    #[cfg(feature = "serialport")]
    fn max_ram_block_size(&self, connection: &mut Connection) -> Result<usize, Error> {
        use super::UsbOtg;

        Ok(if self.is_using_usb_otg(connection)? {
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

    #[cfg(feature = "serialport")]
    fn mac_address(&self, connection: &mut Connection) -> Result<String, Error> {
        let fields = self.common_fields();
        self.read_mac_address_from_words(
            connection,
            fields["MAC_FACTORY_0"],
            fields["MAC_FACTORY_1"],
        )
    }
}

#[cfg(feature = "serialport")]
impl super::RtcWdtReset for Esp32s2 {
    fn wdt_wprotect(&self) -> u32 {
        0x3F40_80AC
    }

    fn wdt_config0(&self) -> u32 {
        0x3F40_8094
    }

    fn wdt_config1(&self) -> u32 {
        0x3F40_8098
    }

    fn can_rtc_wdt_reset(&self, connection: &mut Connection) -> Result<bool, Error> {
        const GPIO_STRAP: u32 = 0x3F40_4038;
        const OPTION1: u32 = 0x3F40_8128;
        const GPIO_STRAP_SPI_BOOT_MASK: u32 = 1 << 3;
        const FORCE_DOWNLOAD_BOOT_MASK: u32 = 0x1;

        Ok(
            connection.read_reg(GPIO_STRAP)? & GPIO_STRAP_SPI_BOOT_MASK == 0 // GPIO0 low
                && connection.read_reg(OPTION1)? & FORCE_DOWNLOAD_BOOT_MASK == 0,
        )
    }
}

#[cfg(feature = "serialport")]
impl super::UsbOtg for Esp32s2 {
    fn uartdev_buf_no(&self) -> u32 {
        0x3FFF_FD14
    }

    fn uartdev_buf_no_usb_otg(&self) -> u32 {
        2
    }
}

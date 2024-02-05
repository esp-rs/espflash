use std::ops::Range;

#[cfg(feature = "serialport")]
use crate::connection::Connection;
use crate::{
    elf::FirmwareImage,
    error::Error,
    flasher::{FlashData, FlashFrequency},
    image_format::{IdfBootloaderFormat, ImageFormat},
    targets::{Chip, Esp32Params, ReadEFuse, SpiRegisters, Target, XtalFrequency},
};

const CHIP_DETECT_MAGIC_VALUES: &[u32] = &[
    0x6921_506f, // ECO1 + ECO2
    0x1b31_506f, // ECO3
    0x4881_606F, // ECO6
    0x4361_606f, // ECO7
];

const FLASH_RANGES: &[Range<u32>] = &[
    0x4200_0000..0x4280_0000, // IROM
    0x3c00_0000..0x3c80_0000, // DROM
];

const PARAMS: Esp32Params = Esp32Params::new(
    0x0,
    0x1_0000,
    0x3f_0000,
    5,
    FlashFrequency::_40Mhz,
    include_bytes!("../../resources/bootloaders/esp32c3-bootloader.bin"),
);

/// ESP32-C3 Target
pub struct Esp32c3;

impl Esp32c3 {
    /// Check if the magic value contains the specified value
    pub fn has_magic_value(value: u32) -> bool {
        CHIP_DETECT_MAGIC_VALUES.contains(&value)
    }
}

impl ReadEFuse for Esp32c3 {
    fn efuse_reg(&self) -> u32 {
        0x6000_8800
    }
}

impl Target for Esp32c3 {
    fn addr_is_flash(&self, addr: u32) -> bool {
        FLASH_RANGES.iter().any(|range| range.contains(&addr))
    }

    #[cfg(feature = "serialport")]
    fn chip_features(&self, _connection: &mut Connection) -> Result<Vec<&str>, Error> {
        Ok(vec!["WiFi", "BLE"])
    }

    #[cfg(feature = "serialport")]
    fn major_chip_version(&self, connection: &mut Connection) -> Result<u32, Error> {
        Ok(self.read_efuse(connection, 22)? >> 24 & 0x3)
    }

    #[cfg(feature = "serialport")]
    fn minor_chip_version(&self, connection: &mut Connection) -> Result<u32, Error> {
        let hi = self.read_efuse(connection, 22)? >> 23 & 0x1;
        let lo = self.read_efuse(connection, 20)? >> 18 & 0x7;

        Ok((hi << 3) + lo)
    }

    #[cfg(feature = "serialport")]
    fn crystal_freq(&self, _connection: &mut Connection) -> Result<XtalFrequency, Error> {
        // The ESP32-C3's XTAL has a fixed frequency of 40MHz.
        Ok(XtalFrequency::_40Mhz)
    }

    fn get_flash_image<'a>(
        &self,
        image: &'a dyn FirmwareImage<'a>,
        flash_data: FlashData,
        _chip_revision: Option<(u32, u32)>,
        xtal_freq: XtalFrequency,
    ) -> Result<Box<dyn ImageFormat<'a> + 'a>, Error> {
        if xtal_freq != XtalFrequency::_40Mhz {
            return Err(Error::UnsupportedFeature {
                chip: Chip::Esp32c3,
                feature: "the selected crystal frequency".into(),
            });
        }

        Ok(Box::new(IdfBootloaderFormat::new(
            image,
            Chip::Esp32c3,
            flash_data.min_chip_rev,
            PARAMS,
            flash_data.partition_table,
            flash_data.partition_table_offset,
            flash_data.target_app_partition,
            flash_data.bootloader,
            flash_data.flash_settings,
        )?))
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

    fn supported_build_targets(&self) -> &[&str] {
        &[
            "riscv32imac-unknown-none-elf",
            "riscv32imc-esp-espidf",
            "riscv32imc-unknown-none-elf",
        ]
    }
}

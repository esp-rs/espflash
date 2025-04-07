use std::{collections::HashMap, ops::Range};

#[cfg(feature = "serialport")]
use crate::connection::Connection;
use crate::{
    flasher::{FlashData, FlashFrequency},
    image_format::IdfBootloaderFormat,
    targets::{Chip, Esp32Params, ReadEFuse, SpiRegisters, Target, XtalFrequency},
    Error,
};

pub(crate) const CHIP_ID: u16 = 16;

const CHIP_DETECT_MAGIC_VALUES: &[u32] = &[0xD7B7_3E80];

const FLASH_RANGES: &[Range<u32>] = &[
    0x4200_0000..0x4280_0000, // IROM
    0x4280_0000..0x4300_0000, // DROM
];

const PARAMS: Esp32Params = Esp32Params::new(
    0x0,
    0x1_0000,
    0x3f_0000,
    CHIP_ID,
    FlashFrequency::_24Mhz,
    include_bytes!("../../resources/bootloaders/esp32h2-bootloader.bin"),
    Some(&[8 * 1024, 16 * 1024, 32 * 1024, 64 * 1024]),
);

/// ESP32-H2 Target
pub struct Esp32h2;

impl Esp32h2 {
    pub fn has_magic_value(value: u32) -> bool {
        CHIP_DETECT_MAGIC_VALUES.contains(&value)
    }
}

impl ReadEFuse for Esp32h2 {
    fn efuse_reg(&self) -> u32 {
        0x600B_0800
    }
}

impl Target for Esp32h2 {
    fn addr_is_flash(&self, addr: u32) -> bool {
        FLASH_RANGES.iter().any(|range| range.contains(&addr))
    }

    #[cfg(feature = "serialport")]
    fn chip_features(&self, _connection: &mut Connection) -> Result<Vec<&str>, Error> {
        Ok(vec!["BLE"])
    }

    #[cfg(feature = "serialport")]
    fn major_chip_version(&self, connection: &mut Connection) -> Result<u32, Error> {
        Ok((self.read_efuse(connection, 22)? >> 24) & 0x3)
    }

    #[cfg(feature = "serialport")]
    fn minor_chip_version(&self, connection: &mut Connection) -> Result<u32, Error> {
        let hi = (self.read_efuse(connection, 22)? >> 23) & 0x1;
        let lo = (self.read_efuse(connection, 20)? >> 18) & 0x7;

        Ok((hi << 3) + lo)
    }

    #[cfg(feature = "serialport")]
    fn crystal_freq(&self, _connection: &mut Connection) -> Result<XtalFrequency, Error> {
        // The ESP32-H2's XTAL has a fixed frequency of 32MHz.
        Ok(XtalFrequency::_32Mhz)
    }

    fn flash_frequency_encodings(&self) -> HashMap<FlashFrequency, u8> {
        use FlashFrequency::*;

        let encodings = [(_12Mhz, 0x2), (_16Mhz, 0x1), (_24Mhz, 0x0), (_48Mhz, 0xF)];

        HashMap::from(encodings)
    }

    fn flash_image<'a>(
        &self,
        elf_data: &'a [u8],
        flash_data: FlashData,
        _chip_revision: Option<(u32, u32)>,
        xtal_freq: XtalFrequency,
    ) -> Result<IdfBootloaderFormat<'a>, Error> {
        if xtal_freq != XtalFrequency::_32Mhz {
            return Err(Error::UnsupportedFeature {
                chip: Chip::Esp32h2,
                feature: "the selected crystal frequency".into(),
            });
        }

        IdfBootloaderFormat::new(elf_data, Chip::Esp32h2, flash_data, PARAMS)
    }

    fn spi_registers(&self) -> SpiRegisters {
        SpiRegisters {
            base: 0x6000_3000,
            usr_offset: 0x18,
            usr1_offset: 0x1c,
            usr2_offset: 0x20,
            w0_offset: 0x58,
            mosi_length_offset: Some(0x24),
            miso_length_offset: Some(0x28),
        }
    }

    fn supported_build_targets(&self) -> &[&str] {
        &["riscv32imac-esp-espidf", "riscv32imac-unknown-none-elf"]
    }
}

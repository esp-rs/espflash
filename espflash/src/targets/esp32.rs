use std::ops::Range;

use esp_idf_part::PartitionTable;

use crate::{
    connection::Connection,
    elf::FirmwareImage,
    error::{Error, UnsupportedImageFormatError},
    flasher::{FlashFrequency, FlashMode, FlashSize},
    image_format::{IdfBootloaderFormat, ImageFormat, ImageFormatKind},
    targets::{bytes_to_mac_addr, Chip, Esp32Params, ReadEFuse, SpiRegisters, Target},
};

const CHIP_DETECT_MAGIC_VALUES: &[u32] = &[0x00f0_1d83];

const FLASH_RANGES: &[Range<u32>] = &[
    0x400d_0000..0x4040_0000, // IROM
    0x3f40_0000..0x3f80_0000, // DROM
];

const PARAMS: Esp32Params = Esp32Params::new(
    0x1000,
    0x1_0000,
    0x3f_0000,
    0,
    FlashFrequency::_40Mhz,
    include_bytes!("../../resources/bootloaders/esp32-bootloader.bin"),
);

const UART_CLKDIV_REG: u32 = 0x3ff4_0014;
const UART_CLKDIV_MASK: u32 = 0xfffff;

const XTAL_CLK_DIVIDER: u32 = 1;

/// ESP32 Target
pub struct Esp32;

impl Esp32 {
    /// Check if the magic value contains the specified value
    pub fn has_magic_value(value: u32) -> bool {
        CHIP_DETECT_MAGIC_VALUES.contains(&value)
    }

    /// Return the package version based on the eFuses
    fn package_version(&self, connection: &mut Connection) -> Result<u32, Error> {
        let word3 = self.read_efuse(connection, 3)?;

        let pkg_version = (word3 >> 9) & 0x7;
        let pkg_version = pkg_version + (((word3 >> 2) & 0x1) << 3);

        Ok(pkg_version)
    }
}

impl ReadEFuse for Esp32 {
    fn efuse_reg(&self) -> u32 {
        0x3ff5_a000
    }
}

impl Target for Esp32 {
    fn addr_is_flash(&self, addr: u32) -> bool {
        FLASH_RANGES.iter().any(|range| range.contains(&addr))
    }

    fn chip_features(&self, connection: &mut Connection) -> Result<Vec<&str>, Error> {
        let word3 = self.read_efuse(connection, 3)?;
        let word4 = self.read_efuse(connection, 4)?;
        let word6 = self.read_efuse(connection, 6)?;

        let mut features = vec!["WiFi"];

        let chip_ver_dis_bt = word3 & 0x2;
        if chip_ver_dis_bt == 0 {
            features.push("BT");
        }

        let chip_ver_dis_app_cpu = word3 & 0x1;
        if chip_ver_dis_app_cpu == 0 {
            features.push("Dual Core");
        } else {
            features.push("Single Core");
        }

        let chip_cpu_freq_rated = word3 & (1 << 13);
        if chip_cpu_freq_rated != 0 {
            let chip_cpu_freq_low = word3 & (1 << 12);
            if chip_cpu_freq_low != 0 {
                features.push("160MHz");
            } else {
                features.push("240MHz");
            }
        }

        let pkg_version = self.package_version(connection)?;
        if [2, 4, 5, 6].contains(&pkg_version) {
            features.push("Embedded Flash");
        }
        if pkg_version == 6 {
            features.push("Embedded PSRAM");
        }

        let adc_vref = (word4 >> 8) & 0x1;
        if adc_vref != 0 {
            features.push("VRef calibration in efuse");
        }

        let blk3_part_res = (word3 >> 14) & 0x1;
        if blk3_part_res != 0 {
            features.push("BLK3 partially reserved");
        }

        let coding_scheme = word6 & 0x3;
        features.push(match coding_scheme {
            0 => "Coding Scheme None",
            1 => "Coding Scheme 3/4",
            2 => "Coding Scheme Repeat (UNSUPPORTED)",
            _ => "Coding Scheme Invalid",
        });

        Ok(features)
    }

    fn major_chip_version(&self, connection: &mut Connection) -> Result<u32, Error> {
        let apb_ctl_date = connection.read_reg(0x3FF6_607C)?;

        let rev_bit0 = (self.read_efuse(connection, 3)? >> 15) & 0x1;
        let rev_bit1 = (self.read_efuse(connection, 5)? >> 20) & 0x1;
        let rev_bit2 = (apb_ctl_date >> 31) & 0x1;

        let combine_value = (rev_bit2 << 2) | (rev_bit1 << 1) | rev_bit0;

        match combine_value {
            1 => Ok(1),
            3 => Ok(2),
            7 => Ok(3),
            _ => Ok(0),
        }
    }

    fn minor_chip_version(&self, connection: &mut Connection) -> Result<u32, Error> {
        Ok((self.read_efuse(connection, 5)? >> 24) & 0x3)
    }

    fn crystal_freq(&self, connection: &mut Connection) -> Result<u32, Error> {
        let uart_div = connection.read_reg(UART_CLKDIV_REG)? & UART_CLKDIV_MASK;
        let est_xtal = (connection.get_baud()? * uart_div) / 1_000_000 / XTAL_CLK_DIVIDER;
        let norm_xtal = if est_xtal > 33 { 40 } else { 26 };

        Ok(norm_xtal)
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
                Chip::Esp32,
                PARAMS,
                partition_table,
                target_app_partition,
                bootloader,
                flash_mode,
                flash_size,
                flash_freq,
            )?)),
            _ => Err(UnsupportedImageFormatError::new(image_format, Chip::Esp32, None).into()),
        }
    }

    fn mac_address(&self, connection: &mut Connection) -> Result<String, Error> {
        let word1 = self.read_efuse(connection, 1)?;
        let word2 = self.read_efuse(connection, 2)?;

        let words = ((word2 as u64) << 32) | word1 as u64;
        let bytes = words.to_be_bytes();
        let bytes = &bytes[2..8];

        Ok(bytes_to_mac_addr(bytes))
    }

    fn spi_registers(&self) -> SpiRegisters {
        SpiRegisters {
            base: 0x3ff4_2000,
            usr_offset: 0x1c,
            usr1_offset: 0x20,
            usr2_offset: 0x24,
            w0_offset: 0x80,
            mosi_length_offset: Some(0x28),
            miso_length_offset: Some(0x2c),
        }
    }

    fn supported_build_targets(&self) -> &[&str] {
        &["xtensa-esp32-none-elf", "xtensa-esp32-espidf"]
    }
}

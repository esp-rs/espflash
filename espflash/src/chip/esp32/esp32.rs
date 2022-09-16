use std::ops::Range;

use super::Esp32Params;
use crate::{
    chip::{bytes_to_mac_addr, Chip, ChipType, ReadEFuse, SpiRegisters},
    connection::Connection,
    elf::{FirmwareImage, FlashFrequency, FlashMode},
    error::UnsupportedImageFormatError,
    flasher::FlashSize,
    image_format::{Esp32BootloaderFormat, ImageFormat, ImageFormatId},
    Error, PartitionTable,
};

pub struct Esp32;

pub const PARAMS: Esp32Params = Esp32Params::new(
    0x1000,
    0x10000,
    0x3f0000,
    0,
    include_bytes!("../../../resources/bootloaders/esp32-bootloader.bin"),
);

impl ChipType for Esp32 {
    const CHIP_DETECT_MAGIC_VALUES: &'static [u32] = &[0x00f01d83];

    const UART_CLKDIV_REG: u32 = 0x3ff40014;

    const SPI_REGISTERS: SpiRegisters = SpiRegisters {
        base: 0x3ff42000,
        usr_offset: 0x1c,
        usr1_offset: 0x20,
        usr2_offset: 0x24,
        w0_offset: 0x80,
        mosi_length_offset: Some(0x28),
        miso_length_offset: Some(0x2c),
    };

    const FLASH_RANGES: &'static [Range<u32>] = &[
        0x400d0000..0x40400000, // IROM
        0x3F400000..0x3F800000, // DROM
    ];

    const SUPPORTED_TARGETS: &'static [&'static str] =
        &["xtensa-esp32-none-elf", "xtensa-esp32-espidf"];

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
                Chip::Esp32,
                PARAMS,
                partition_table,
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
}

impl ReadEFuse for Esp32 {
    const EFUSE_REG_BASE: u32 = 0x3ff5a000;
}

impl Esp32 {
    pub fn chip_revision(&self, connection: &mut Connection) -> Result<u32, Error> {
        let word3 = self.read_efuse(connection, 3)?;
        let word5 = self.read_efuse(connection, 5)?;

        let apb_ctrl_date = connection.read_reg(0x3FF6607C)?;

        let rev_bit0 = (word3 >> 15) & 0x1 != 0;
        let rev_bit1 = (word5 >> 20) & 0x1 != 0;
        let rev_bit2 = (apb_ctrl_date >> 31) & 0x1 != 0;

        let revision = match (rev_bit0, rev_bit1, rev_bit2) {
            (true, true, true) => 3,
            (true, true, false) => 2,
            (true, false, _) => 1,
            (false, _, _) => 0,
        };

        Ok(revision)
    }

    fn package_version(&self, connection: &mut Connection) -> Result<u32, Error> {
        let word3 = self.read_efuse(connection, 3)?;

        let pkg_version = (word3 >> 9) & 0x7;
        let pkg_version = pkg_version + (((word3 >> 2) & 0x1) << 3);

        Ok(pkg_version)
    }
}

#[test]
fn test_esp32_rom() {
    use std::fs::read;

    use crate::elf::ElfFirmwareImage;

    let input_bytes = read("./tests/data/esp32").unwrap();
    let expected_bin = read("./tests/data/esp32.bin").unwrap();

    let image = ElfFirmwareImage::try_from(input_bytes.as_slice()).unwrap();
    let flash_image =
        Esp32BootloaderFormat::new(&image, Chip::Esp32, PARAMS, None, None, None, None, None)
            .unwrap();

    let segments = flash_image.flash_segments().collect::<Vec<_>>();

    assert_eq!(3, segments.len());
    let buff = segments[2].data.as_ref();
    assert_eq!(expected_bin.len(), buff.len());
    assert_eq!(&expected_bin.as_slice(), &buff);
}

//! Flashable target devices
//!
//! All ESP32 devices support booting via the ESP-IDF bootloader. It's also
//! possible to write an application to and boot from RAM, where a bootloader is
//! obviously not required either.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use strum::{Display, EnumIter, EnumString, IntoEnumIterator, VariantNames};

#[cfg(feature = "serialport")]
pub use self::flash_target::{
    DefaultProgressCallback,
    Esp32Target,
    FlashTarget,
    ProgressCallbacks,
    RamTarget,
};
use crate::{
    Error,
    flasher::{FLASH_WRITE_SIZE, FlashFrequency},
};
#[cfg(feature = "serialport")]
use crate::{connection::Connection, flasher::SpiAttachParams, target::efuse::EfuseField};

pub mod efuse;

#[cfg(feature = "serialport")]
pub(crate) mod flash_target;

#[cfg(feature = "serialport")]
pub(crate) const WDT_WKEY: u32 = 0x50D8_3AA1;

/// Maximum block size for RAM flashing.
pub(crate) const MAX_RAM_BLOCK_SIZE: usize = 0x1800;

/// Supported crystal frequencies
///
/// Note that not all frequencies are supported by each target device.
#[cfg_attr(feature = "cli", derive(clap::ValueEnum))]
#[derive(
    Debug, Default, Clone, Copy, Hash, PartialEq, Eq, Display, VariantNames, Serialize, Deserialize,
)]
#[non_exhaustive]
#[repr(u32)]
pub enum XtalFrequency {
    /// 26 MHz
    #[strum(serialize = "26 MHz")]
    _26Mhz,
    /// 32 MHz
    #[strum(serialize = "32 MHz")]
    _32Mhz,
    /// 40 MHz
    #[default]
    #[strum(serialize = "40 MHz")]
    _40Mhz,
    /// 48MHz
    #[strum(serialize = "48 MHz")]
    _48Mhz,
}

/// All supported devices
#[cfg_attr(feature = "cli", derive(clap::ValueEnum))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Display, EnumIter, EnumString, VariantNames)]
#[non_exhaustive]
#[strum(serialize_all = "lowercase")]
pub enum Chip {
    /// ESP32
    Esp32,
    /// ESP32-C2, ESP8684
    Esp32c2,
    /// ESP32-C3, ESP8685
    Esp32c3,
    /// ESP32-C5
    Esp32c5,
    /// ESP32-C6
    Esp32c6,
    /// ESP32-H2
    Esp32h2,
    /// ESP32-P4
    Esp32p4,
    /// ESP32-S2
    Esp32s2,
    /// ESP32-S3
    Esp32s3,
}

impl Chip {
    /// Create a [Chip] from a magic value.
    pub fn from_magic(magic: u32) -> Result<Self, Error> {
        for chip in Chip::iter() {
            if chip.has_magic_value(magic) {
                return Ok(chip);
            }
        }

        Err(Error::ChipDetectError(format!(
            "unrecognized magic value: {magic:#x}"
        )))
    }

    /// Check if the magic value contains the specified value
    pub fn has_magic_value(&self, value: u32) -> bool {
        match self {
            Chip::Esp32 => [0x00f0_1d83].contains(&value),
            Chip::Esp32c2 => [
                0x6f51_306f, // ECO0
                0x7c41_a06f, // ECO1
            ]
            .contains(&value),
            Chip::Esp32c3 => [
                0x6921_506f, // ECO1 + ECO2
                0x1b31_506f, // ECO3
                0x4881_606F, // ECO6
                0x4361_606f, // ECO7
            ]
            .contains(&value),
            Chip::Esp32c5 => [].contains(&value), // Empty array
            Chip::Esp32c6 => [0x2CE0_806F].contains(&value),
            Chip::Esp32h2 => [0xD7B7_3E80].contains(&value),
            Chip::Esp32p4 => [0x0, 0x0ADDBAD0].contains(&value),
            Chip::Esp32s2 => [0x0000_07c6].contains(&value),
            Chip::Esp32s3 => [0x9].contains(&value),
        }
    }

    /// Get the RTC watchdog write protect register address
    #[cfg(feature = "serialport")]
    pub fn wdt_wprotect(&self) -> Option<u32> {
        match self {
            Chip::Esp32c3 => Some(0x6000_80A8),
            Chip::Esp32p4 => Some(0x5011_6018),
            Chip::Esp32s2 => Some(0x3F40_80AC),
            Chip::Esp32s3 => Some(0x6000_80B0),
            _ => None,
        }
    }

    /// Get the RTC watchdog config0 register address
    #[cfg(feature = "serialport")]
    pub fn wdt_config0(&self) -> Option<u32> {
        match self {
            Chip::Esp32c3 => Some(0x6000_8090),
            Chip::Esp32p4 => Some(0x5011_6000),
            Chip::Esp32s2 => Some(0x3F40_8094),
            Chip::Esp32s3 => Some(0x6000_8098),
            _ => None,
        }
    }

    /// Get the RTC watchdog config1 register address
    #[cfg(feature = "serialport")]
    pub fn wdt_config1(&self) -> Option<u32> {
        match self {
            Chip::Esp32c3 => Some(0x6000_8094),
            Chip::Esp32p4 => Some(0x5011_6004),
            Chip::Esp32s2 => Some(0x3F40_8098),
            Chip::Esp32s3 => Some(0x6000_809C),
            _ => None,
        }
    }

    /// Check if RTC WDT reset can be performed
    #[cfg(feature = "serialport")]
    pub fn can_rtc_wdt_reset(&self, connection: &mut Connection) -> Result<bool, Error> {
        match self {
            Chip::Esp32c3 | Chip::Esp32p4 => Ok(true),
            Chip::Esp32s2 => {
                const GPIO_STRAP: u32 = 0x3F40_4038;
                const OPTION1: u32 = 0x3F40_8128;
                const GPIO_STRAP_SPI_BOOT_MASK: u32 = 1 << 3;
                const FORCE_DOWNLOAD_BOOT_MASK: u32 = 0x1;

                Ok(
                    connection.read_reg(GPIO_STRAP)? & GPIO_STRAP_SPI_BOOT_MASK == 0 // GPIO0 low
                        && connection.read_reg(OPTION1)? & FORCE_DOWNLOAD_BOOT_MASK == 0,
                )
            }
            Chip::Esp32s3 => {
                const GPIO_STRAP: u32 = 0x6000_4038;
                const OPTION1: u32 = 0x6000_812C;
                const GPIO_STRAP_SPI_BOOT_MASK: u32 = 1 << 3; // Not download mode
                const FORCE_DOWNLOAD_BOOT_MASK: u32 = 0x1;

                Ok(
                    connection.read_reg(GPIO_STRAP)? & GPIO_STRAP_SPI_BOOT_MASK == 0 // GPIO0 low
                        && connection.read_reg(OPTION1)? & FORCE_DOWNLOAD_BOOT_MASK == 0,
                )
            }
            _ => Err(Error::UnsupportedFeature {
                chip: *self,
                feature: "RTC WDT reset".into(),
            }),
        }
    }

    /// Get the UART device buffer number register address
    #[cfg(feature = "serialport")]
    pub fn uartdev_buf_no(&self) -> Option<u32> {
        match self {
            Chip::Esp32p4 => Some(0x4FF3_FEC8),
            Chip::Esp32s2 => Some(0x3FFF_FD14),
            Chip::Esp32s3 => Some(0x3FCE_F14C),
            _ => None,
        }
    }

    /// Get the UART device buffer number for USB OTG
    #[cfg(feature = "serialport")]
    pub fn uartdev_buf_no_usb_otg(&self) -> Option<u32> {
        match self {
            Chip::Esp32p4 => Some(5),
            Chip::Esp32s2 => Some(2),
            Chip::Esp32s3 => Some(3),
            _ => None,
        }
    }

    /// Check if USB OTG is being used
    #[cfg(feature = "serialport")]
    pub fn is_using_usb_otg(&self, connection: &mut Connection) -> Result<bool, Error> {
        match (self.uartdev_buf_no(), self.uartdev_buf_no_usb_otg()) {
            (Some(buf_no), Some(usb_otg)) => {
                let value = connection.read_reg(buf_no)?;
                Ok(value == usb_otg)
            }
            _ => Err(Error::UnsupportedFeature {
                chip: *self,
                feature: "USB OTG".into(),
            }),
        }
    }

    /// Perform RTC WDT reset
    #[cfg(feature = "serialport")]
    pub fn rtc_wdt_reset(&self, connection: &mut Connection) -> Result<(), Error> {
        match (self.wdt_wprotect(), self.wdt_config0(), self.wdt_config1()) {
            (Some(wdt_wprotect), Some(wdt_config0), Some(wdt_config1)) => {
                use bitflags::bitflags;

                bitflags! {
                    struct WdtConfig0Flags: u32 {
                        const EN               = 1 << 31;
                        const STAGE0           = 5 << 28; // 5 (binary: 101) in bits 28-30
                        const CHIP_RESET_EN    = 1 << 8;  // 8th bit
                        const CHIP_RESET_WIDTH = 1 << 2;  // 1st bit
                    }
                }

                let flags = (WdtConfig0Flags::EN // enable RTC watchdog
                    | WdtConfig0Flags::STAGE0 // enable at the interrupt/system and RTC stage
                    | WdtConfig0Flags::CHIP_RESET_EN // enable chip reset
                    | WdtConfig0Flags::CHIP_RESET_WIDTH) // set chip reset width
                    .bits();

                log::debug!("Resetting with RTC WDT");
                connection.write_reg(wdt_wprotect, WDT_WKEY, None)?;
                connection.write_reg(wdt_config1, 2000, None)?;
                connection.write_reg(wdt_config0, flags, None)?;
                connection.write_reg(wdt_wprotect, 0, None)?;

                std::thread::sleep(std::time::Duration::from_millis(50));

                Ok(())
            }
            _ => Err(Error::UnsupportedFeature {
                chip: *self,
                feature: "RTC WDT reset".into(),
            }),
        }
    }

    /// Returns the chip ID for the [Chip]
    pub fn id(&self) -> u16 {
        match self {
            Chip::Esp32 => 0,
            Chip::Esp32c2 => 12,
            Chip::Esp32c3 => 5,
            Chip::Esp32c5 => 23,
            Chip::Esp32c6 => 13,
            Chip::Esp32h2 => 16,
            Chip::Esp32p4 => 18,
            Chip::Esp32s2 => 2,
            Chip::Esp32s3 => 9,
        }
    }

    /// Returns the valid MMU page sizes for the [Chip]
    pub fn valid_mmu_page_sizes(self) -> Option<&'static [u32]> {
        match self {
            Chip::Esp32c2 => Some(&[16 * 1024, 32 * 1024, 64 * 1024]),
            Chip::Esp32c6 | Chip::Esp32h2 => Some(&[8 * 1024, 16 * 1024, 32 * 1024, 64 * 1024]),
            // TODO: Verify this is correct for Esp32c5
            _ => None,
        }
    }

    /// Returns the boot address for the [Chip]
    pub fn boot_address(&self) -> u32 {
        match self {
            Chip::Esp32c2 | Chip::Esp32c3 | Chip::Esp32c6 | Chip::Esp32h2 | Chip::Esp32s3 => 0x0,
            Chip::Esp32 | Chip::Esp32s2 => 0x1000,
            Chip::Esp32c5 | Chip::Esp32p4 => 0x2000,
        }
    }

    /// Returns the default flash frequency for the [Chip]
    pub fn default_flash_frequency(&self) -> FlashFrequency {
        match self {
            Chip::Esp32
            | Chip::Esp32c3
            | Chip::Esp32c5
            | Chip::Esp32c6
            | Chip::Esp32p4
            | Chip::Esp32s2
            | Chip::Esp32s3 => FlashFrequency::_40Mhz,
            Chip::Esp32c2 => FlashFrequency::_30Mhz,
            Chip::Esp32h2 => FlashFrequency::_24Mhz,
        }
    }

    /// Returns the default crystal frequency for the [Chip]
    pub fn default_xtal_frequency(&self) -> XtalFrequency {
        match self {
            Chip::Esp32c5 => XtalFrequency::_48Mhz,
            Chip::Esp32h2 => XtalFrequency::_32Mhz,
            _ => XtalFrequency::_40Mhz,
        }
    }

    #[cfg(feature = "serialport")]
    /// Creates and returns a new [FlashTarget] for [Esp32Target], using the
    /// provided [SpiAttachParams].
    pub fn flash_target(
        &self,
        spi_params: SpiAttachParams,
        use_stub: bool,
        verify: bool,
        skip: bool,
    ) -> Box<dyn FlashTarget> {
        Box::new(Esp32Target::new(*self, spi_params, use_stub, verify, skip))
    }

    /// Creates and returns a new [FlashTarget] for [RamTarget].
    #[cfg(feature = "serialport")]
    pub fn ram_target(
        &self,
        entry: Option<u32>,
        max_ram_block_size: usize,
    ) -> Box<dyn FlashTarget> {
        Box::new(RamTarget::new(entry, max_ram_block_size))
    }

    /// Returns the base address of the eFuse register
    pub fn efuse_reg(&self) -> u32 {
        match self {
            Chip::Esp32 => 0x3FF5_A000,
            Chip::Esp32c2 => 0x6000_8800,
            Chip::Esp32c3 => 0x6000_8800,
            Chip::Esp32c5 => 0x600B4800,
            Chip::Esp32c6 => 0x600B_0800,
            Chip::Esp32h2 => 0x600B_0800,
            Chip::Esp32p4 => 0x5012_D000,
            Chip::Esp32s2 => 0x3F41_A000,
            Chip::Esp32s3 => 0x6000_7000,
        }
    }

    /// Returns the offset of BLOCK0 relative to the eFuse base register address
    pub fn block0_offset(&self) -> u32 {
        match self {
            Chip::Esp32 => 0x0,
            Chip::Esp32c2 => 0x35,
            Chip::Esp32c3 => 0x2D,
            Chip::Esp32c5 => 0x2C,
            Chip::Esp32c6 => 0x2C,
            Chip::Esp32h2 => 0x2C,
            Chip::Esp32p4 => 0x2C,
            Chip::Esp32s2 => 0x2C,
            Chip::Esp32s3 => 0x2D,
        }
    }

    /// Returns the size of the specified block for the implementing target
    /// device
    pub fn block_size(&self, block: usize) -> u32 {
        match self {
            Chip::Esp32 => efuse::esp32::BLOCK_SIZES[block],
            Chip::Esp32c2 => efuse::esp32c2::BLOCK_SIZES[block],
            Chip::Esp32c3 => efuse::esp32c3::BLOCK_SIZES[block],
            Chip::Esp32c5 => efuse::esp32c5::BLOCK_SIZES[block],
            Chip::Esp32c6 => efuse::esp32c6::BLOCK_SIZES[block],
            Chip::Esp32h2 => efuse::esp32h2::BLOCK_SIZES[block],
            Chip::Esp32p4 => efuse::esp32p4::BLOCK_SIZES[block],
            Chip::Esp32s2 => efuse::esp32s2::BLOCK_SIZES[block],
            Chip::Esp32s3 => efuse::esp32s3::BLOCK_SIZES[block],
        }
    }

    /// Given an active connection, read the specified field of the eFuse region
    #[cfg(feature = "serialport")]
    pub fn read_efuse(&self, connection: &mut Connection, field: EfuseField) -> Result<u32, Error> {
        let mask = if field.bit_count == 32 {
            u32::MAX
        } else {
            (1u32 << field.bit_count) - 1
        };

        let shift = field.bit_start % 32;

        let value = self.read_efuse_raw(connection, field.block, field.word)?;
        let value = (value >> shift) & mask;

        Ok(value)
    }

    /// Read the raw word in the specified eFuse block, without performing any
    /// bit-shifting or masking of the read value
    #[cfg(feature = "serialport")]
    pub fn read_efuse_raw(
        &self,
        connection: &mut Connection,
        block: u32,
        word: u32,
    ) -> Result<u32, Error> {
        let block0_addr = self.efuse_reg() + self.block0_offset();

        let mut block_offset = 0;
        for b in 0..block {
            block_offset += self.block_size(b as usize);
        }

        let addr = block0_addr + block_offset + (word * 0x4);

        connection.read_reg(addr)
    }

    /// Is the provided address `addr` in flash?
    pub fn addr_is_flash(&self, addr: u32) -> bool {
        match self {
            Chip::Esp32 => {
                const FLASH_RANGES: &[std::ops::Range<u32>] = &[
                    0x400d_0000..0x4040_0000, // IROM
                    0x3f40_0000..0x3f80_0000, // DROM
                ];
                FLASH_RANGES.iter().any(|range| range.contains(&addr))
            }
            Chip::Esp32c2 => {
                const FLASH_RANGES: &[std::ops::Range<u32>] = &[
                    0x4200_0000..0x4240_0000, // IROM
                    0x3c00_0000..0x3c40_0000, // DROM
                ];
                FLASH_RANGES.iter().any(|range| range.contains(&addr))
            }
            Chip::Esp32c3 => {
                const FLASH_RANGES: &[std::ops::Range<u32>] = &[
                    0x4200_0000..0x4280_0000, // IROM
                    0x3c00_0000..0x3c80_0000, // DROM
                ];
                FLASH_RANGES.iter().any(|range| range.contains(&addr))
            }
            Chip::Esp32c5 => {
                const FLASH_RANGES: &[std::ops::Range<u32>] = &[
                    0x4200_0000..0x4280_0000, // IROM
                    0x3c00_0000..0x3c80_0000, // DROM
                ];
                FLASH_RANGES.iter().any(|range| range.contains(&addr))
            }
            Chip::Esp32c6 => {
                const FLASH_RANGES: &[std::ops::Range<u32>] = &[
                    0x4200_0000..0x4280_0000, // IROM
                    0x3c00_0000..0x3c80_0000, // DROM
                ];
                FLASH_RANGES.iter().any(|range| range.contains(&addr))
            }
            Chip::Esp32h2 => {
                const FLASH_RANGES: &[std::ops::Range<u32>] = &[
                    0x4200_0000..0x4280_0000, // IROM
                    0x3c00_0000..0x3c80_0000, // DROM
                ];
                FLASH_RANGES.iter().any(|range| range.contains(&addr))
            }
            Chip::Esp32p4 => {
                const FLASH_RANGES: &[std::ops::Range<u32>] = &[
                    0x4800_0000..0x4C00_0000, // IROM
                    0x4000_0000..0x4400_0000, // DROM
                ];
                FLASH_RANGES.iter().any(|range| range.contains(&addr))
            }
            Chip::Esp32s2 => {
                const FLASH_RANGES: &[std::ops::Range<u32>] = &[
                    0x4008_0000..0x4180_0000, // IROM
                    0x3f00_0000..0x3f3f_0000, // DROM
                ];
                FLASH_RANGES.iter().any(|range| range.contains(&addr))
            }
            Chip::Esp32s3 => {
                const FLASH_RANGES: &[std::ops::Range<u32>] = &[
                    0x4200_0000..0x4400_0000, // IROM
                    0x3c00_0000..0x3e00_0000, // DROM
                ];
                FLASH_RANGES.iter().any(|range| range.contains(&addr))
            }
        }
    }

    /// Enumerate the chip's features, read from eFuse
    #[cfg(feature = "serialport")]
    pub fn chip_features(&self, connection: &mut Connection) -> Result<Vec<&str>, Error> {
        match self {
            Chip::Esp32 => {
                let mut features = vec!["WiFi"];

                let disable_bt = self.read_efuse(connection, efuse::esp32::DISABLE_BT)?;
                if disable_bt == 0 {
                    features.push("BT");
                }

                let disable_app_cpu = self.read_efuse(connection, efuse::esp32::DISABLE_APP_CPU)?;
                if disable_app_cpu == 0 {
                    features.push("Dual Core");
                } else {
                    features.push("Single Core");
                }

                let chip_cpu_freq_rated =
                    self.read_efuse(connection, efuse::esp32::CHIP_CPU_FREQ_RATED)?;
                if chip_cpu_freq_rated != 0 {
                    let chip_cpu_freq_low =
                        self.read_efuse(connection, efuse::esp32::CHIP_CPU_FREQ_LOW)?;
                    if chip_cpu_freq_low != 0 {
                        features.push("160MHz");
                    } else {
                        features.push("240MHz");
                    }
                }

                // Get package version using helper method
                let pkg_version = self.esp32_package_version(connection)?;
                if [2, 4, 5, 6].contains(&pkg_version) {
                    features.push("Embedded Flash");
                }
                if pkg_version == 6 {
                    features.push("Embedded PSRAM");
                }

                let adc_vref = self.read_efuse(connection, efuse::esp32::ADC_VREF)?;
                if adc_vref != 0 {
                    features.push("VRef calibration in efuse");
                }

                let blk3_part_reserve =
                    self.read_efuse(connection, efuse::esp32::BLK3_PART_RESERVE)?;
                if blk3_part_reserve != 0 {
                    features.push("BLK3 partially reserved");
                }

                let coding_scheme = self.read_efuse(connection, efuse::esp32::CODING_SCHEME)?;
                features.push(match coding_scheme {
                    0 => "Coding Scheme None",
                    1 => "Coding Scheme 3/4",
                    2 => "Coding Scheme Repeat (UNSUPPORTED)",
                    _ => "Coding Scheme Invalid",
                });

                Ok(features)
            }
            Chip::Esp32c2 => Ok(vec!["WiFi", "BLE"]),
            Chip::Esp32c3 => Ok(vec!["WiFi", "BLE"]),
            Chip::Esp32c5 => Ok(vec!["WiFi", "BLE", "IEEE802.15.4", "240MHz"]),
            Chip::Esp32c6 => Ok(vec!["WiFi 6", "BT 5"]),
            Chip::Esp32h2 => Ok(vec!["BLE"]),
            Chip::Esp32p4 => Ok(vec!["High-Performance MCU"]),
            Chip::Esp32s2 => {
                let mut features = vec!["WiFi"];

                let flash_version = match self.esp32s2_flash_version(connection)? {
                    0 => "No Embedded Flash",
                    1 => "Embedded Flash 2MB",
                    2 => "Embedded Flash 4MB",
                    _ => "Unknown Embedded Flash",
                };
                features.push(flash_version);

                let psram_version = match self.esp32s2_psram_version(connection)? {
                    0 => "No Embedded PSRAM",
                    1 => "Embedded PSRAM 2MB",
                    2 => "Embedded PSRAM 4MB",
                    _ => "Unknown Embedded PSRAM",
                };
                features.push(psram_version);

                let block2_version = match self.esp32s2_block2_version(connection)? {
                    0 => "No calibration in BLK2 of efuse",
                    1 => "ADC and temperature sensor calibration in BLK2 of efuse V1",
                    2 => "ADC and temperature sensor calibration in BLK2 of efuse V2",
                    _ => "Unknown Calibration in BLK2",
                };
                features.push(block2_version);

                Ok(features)
            }
            Chip::Esp32s3 => {
                let mut features = vec!["WiFi", "BLE"];

                // Special handling for chip revision 0
                if self.esp32s3_blk_version_major(connection)? == 1
                    && self.esp32s3_blk_version_minor(connection)? == 1
                {
                    features.push("Embedded PSRAM");
                } else {
                    features.push("Embedded Flash");
                }

                Ok(features)
            }
        }
    }

    /// Determine the chip's revision number
    #[cfg(feature = "serialport")]
    pub fn revision(&self, connection: &mut Connection) -> Result<(u32, u32), Error> {
        let major = self.major_version(connection)?;
        let minor = self.minor_version(connection)?;

        Ok((major, minor))
    }

    /// Returns the chip's major version.
    #[cfg(feature = "serialport")]
    pub fn major_version(&self, connection: &mut Connection) -> Result<u32, Error> {
        match self {
            Chip::Esp32 => {
                let apb_ctl_date = connection.read_reg(0x3FF6_607C)?;

                let word3 = self.read_efuse_raw(connection, 0, 3)?;
                let word5 = self.read_efuse_raw(connection, 0, 5)?;

                let rev_bit0 = (word3 >> 15) & 0x1;
                let rev_bit1 = (word5 >> 20) & 0x1;
                let rev_bit2 = (apb_ctl_date >> 31) & 0x1;

                let combine_value = (rev_bit2 << 2) | (rev_bit1 << 1) | rev_bit0;

                match combine_value {
                    1 => Ok(1),
                    3 => Ok(2),
                    7 => Ok(3),
                    _ => Ok(0),
                }
            }
            Chip::Esp32c2 => self.read_efuse(connection, efuse::esp32c2::WAFER_VERSION_MAJOR),
            Chip::Esp32c3 => self.read_efuse(connection, efuse::esp32c3::WAFER_VERSION_MAJOR),
            Chip::Esp32c5 => self.read_efuse(connection, efuse::esp32c5::WAFER_VERSION_MAJOR),
            Chip::Esp32c6 => self.read_efuse(connection, efuse::esp32c6::WAFER_VERSION_MAJOR),
            Chip::Esp32h2 => self.read_efuse(connection, efuse::esp32h2::WAFER_VERSION_MAJOR),
            Chip::Esp32p4 => {
                let hi = self.read_efuse(connection, efuse::esp32p4::WAFER_VERSION_MAJOR_HI)?;
                let lo = self.read_efuse(connection, efuse::esp32p4::WAFER_VERSION_MAJOR_LO)?;

                let version = (hi << 2) | lo;

                Ok(version)
            }
            Chip::Esp32s2 => self.read_efuse(connection, efuse::esp32s2::WAFER_VERSION_MAJOR),
            Chip::Esp32s3 => {
                if self.esp32s3_blk_version_major(connection)? == 1
                    && self.esp32s3_blk_version_minor(connection)? == 1
                {
                    Ok(0)
                } else {
                    self.read_efuse(connection, efuse::esp32s3::WAFER_VERSION_MAJOR)
                }
            }
        }
    }

    /// Returns the chip's minor version.
    #[cfg(feature = "serialport")]
    pub fn minor_version(&self, connection: &mut Connection) -> Result<u32, Error> {
        match self {
            Chip::Esp32 => self.read_efuse(connection, efuse::esp32::WAFER_VERSION_MINOR),
            Chip::Esp32c2 => self.read_efuse(connection, efuse::esp32c2::WAFER_VERSION_MINOR),
            Chip::Esp32c3 => {
                let hi = self.read_efuse(connection, efuse::esp32c3::WAFER_VERSION_MINOR_HI)?;
                let lo = self.read_efuse(connection, efuse::esp32c3::WAFER_VERSION_MINOR_LO)?;

                Ok((hi << 3) + lo)
            }
            Chip::Esp32c5 => self.read_efuse(connection, efuse::esp32c5::WAFER_VERSION_MINOR),
            Chip::Esp32c6 => self.read_efuse(connection, efuse::esp32c6::WAFER_VERSION_MINOR),
            Chip::Esp32h2 => self.read_efuse(connection, efuse::esp32h2::WAFER_VERSION_MINOR),
            Chip::Esp32p4 => self.read_efuse(connection, efuse::esp32p4::WAFER_VERSION_MINOR),
            Chip::Esp32s2 => {
                let hi = self.read_efuse(connection, efuse::esp32s2::WAFER_VERSION_MINOR_HI)?;
                let lo = self.read_efuse(connection, efuse::esp32s2::WAFER_VERSION_MINOR_LO)?;

                Ok((hi << 3) + lo)
            }
            Chip::Esp32s3 => {
                let hi = self.read_efuse(connection, efuse::esp32s3::WAFER_VERSION_MINOR_HI)?;
                let lo = self.read_efuse(connection, efuse::esp32s3::WAFER_VERSION_MINOR_LO)?;

                Ok((hi << 3) + lo)
            }
        }
    }

    #[cfg(feature = "serialport")]
    /// retrieve the xtal frequency of the chip.
    pub fn xtal_frequency(&self, connection: &mut Connection) -> Result<XtalFrequency, Error> {
        match self {
            Chip::Esp32 => {
                const UART_CLKDIV_REG: u32 = 0x3ff4_0014; // UART0_BASE_REG + 0x14
                const UART_CLKDIV_MASK: u32 = 0xfffff;
                const XTAL_CLK_DIVIDER: u32 = 1;

                let uart_div = connection.read_reg(UART_CLKDIV_REG)? & UART_CLKDIV_MASK;
                let est_xtal = (connection.baud()? * uart_div) / 1_000_000 / XTAL_CLK_DIVIDER;
                let norm_xtal = if est_xtal > 33 {
                    XtalFrequency::_40Mhz
                } else {
                    XtalFrequency::_26Mhz
                };

                Ok(norm_xtal)
            }
            Chip::Esp32c2 => {
                const UART_CLKDIV_REG: u32 = 0x6000_0014; // UART0_BASE_REG + 0x14
                const UART_CLKDIV_MASK: u32 = 0xfffff;
                const XTAL_CLK_DIVIDER: u32 = 1;

                let uart_div = connection.read_reg(UART_CLKDIV_REG)? & UART_CLKDIV_MASK;
                let est_xtal = (connection.baud()? * uart_div) / 1_000_000 / XTAL_CLK_DIVIDER;
                let norm_xtal = if est_xtal > 33 {
                    XtalFrequency::_40Mhz
                } else {
                    XtalFrequency::_26Mhz
                };

                Ok(norm_xtal)
            }
            Chip::Esp32c3 => Ok(XtalFrequency::_40Mhz), // Fixed frequency
            Chip::Esp32c5 => {
                const UART_CLKDIV_REG: u32 = 0x6000_0014; // UART0_BASE_REG + 0x14
                const UART_CLKDIV_MASK: u32 = 0xfffff;
                const XTAL_CLK_DIVIDER: u32 = 1;

                let uart_div = connection.read_reg(UART_CLKDIV_REG)? & UART_CLKDIV_MASK;
                let est_xtal = (connection.baud()? * uart_div) / 1_000_000 / XTAL_CLK_DIVIDER;
                let norm_xtal = if est_xtal > 45 {
                    XtalFrequency::_48Mhz
                } else {
                    XtalFrequency::_40Mhz
                };

                Ok(norm_xtal)
            }
            Chip::Esp32h2 => Ok(XtalFrequency::_32Mhz), // Fixed frequency
            Chip::Esp32c6 | Chip::Esp32p4 | Chip::Esp32s2 | Chip::Esp32s3 => {
                Ok(XtalFrequency::_40Mhz)
            } // Fixed frequency
        }
    }

    /// Numeric encodings for the flash frequencies supported by a chip
    pub fn flash_frequency_encodings(&self) -> HashMap<FlashFrequency, u8> {
        use FlashFrequency::*;

        HashMap::from(match self {
            Chip::Esp32h2 => [(_12Mhz, 0x2), (_16Mhz, 0x1), (_24Mhz, 0x0), (_48Mhz, 0xF)],
            Chip::Esp32c2 => [(_15Mhz, 0x2), (_20Mhz, 0x1), (_30Mhz, 0x0), (_60Mhz, 0xF)],
            _ => [(_20Mhz, 0x2), (_26Mhz, 0x1), (_40Mhz, 0x0), (_80Mhz, 0xf)],
        })
    }

    /// Write size for flashing operations
    pub fn flash_write_size(&self) -> usize {
        FLASH_WRITE_SIZE
    }

    #[cfg(feature = "serialport")]
    /// What is the MAC address?
    pub fn mac_address(&self, connection: &mut Connection) -> Result<String, Error> {
        let (mac0_field, mac1_field) = match self {
            Chip::Esp32 => (self::efuse::esp32::MAC0, self::efuse::esp32::MAC1),
            Chip::Esp32c2 => (self::efuse::esp32c2::MAC0, self::efuse::esp32c2::MAC1),
            Chip::Esp32c3 => (self::efuse::esp32c3::MAC0, self::efuse::esp32c3::MAC1),
            Chip::Esp32c5 => (self::efuse::esp32c5::MAC0, self::efuse::esp32c5::MAC1),
            Chip::Esp32c6 => (self::efuse::esp32c6::MAC0, self::efuse::esp32c6::MAC1),
            Chip::Esp32h2 => (self::efuse::esp32h2::MAC0, self::efuse::esp32h2::MAC1),
            Chip::Esp32p4 => (self::efuse::esp32p4::MAC0, self::efuse::esp32p4::MAC1),
            Chip::Esp32s2 => (self::efuse::esp32s2::MAC0, self::efuse::esp32s2::MAC1),
            Chip::Esp32s3 => (self::efuse::esp32s3::MAC0, self::efuse::esp32s3::MAC1),
        };

        let mac0 = self.read_efuse(connection, mac0_field)?;
        let mac1 = self.read_efuse(connection, mac1_field)?;

        let bytes = ((mac1 as u64) << 32) | mac0 as u64;
        let bytes = bytes.to_be_bytes();
        let bytes = &bytes[2..];

        let mac_addr = bytes
            .iter()
            .map(|b| format!("{:02x}", b))
            .collect::<Vec<_>>()
            .join(":");

        Ok(mac_addr)
    }

    /// Maximum RAM block size for writing
    pub fn max_ram_block_size(&self) -> usize {
        MAX_RAM_BLOCK_SIZE
    }

    /// SPI register addresses for a chip
    pub fn spi_registers(&self) -> SpiRegisters {
        match self {
            Chip::Esp32 => SpiRegisters {
                base: 0x3ff4_2000,
                usr_offset: 0x1c,
                usr1_offset: 0x20,
                usr2_offset: 0x24,
                w0_offset: 0x80,
                mosi_length_offset: Some(0x28),
                miso_length_offset: Some(0x2c),
            },
            Chip::Esp32c2 => SpiRegisters {
                base: 0x6000_3000,
                usr_offset: 0x18,
                usr1_offset: 0x1c,
                usr2_offset: 0x20,
                w0_offset: 0x58,
                mosi_length_offset: Some(0x24),
                miso_length_offset: Some(0x28),
            },
            Chip::Esp32c3 => SpiRegisters {
                base: 0x6000_3000,
                usr_offset: 0x18,
                usr1_offset: 0x1c,
                usr2_offset: 0x20,
                w0_offset: 0x58,
                mosi_length_offset: Some(0x24),
                miso_length_offset: Some(0x28),
            },
            Chip::Esp32c5 => SpiRegisters {
                base: 0x6000_3000,
                usr_offset: 0x18,
                usr1_offset: 0x1c,
                usr2_offset: 0x20,
                w0_offset: 0x58,
                mosi_length_offset: Some(0x24),
                miso_length_offset: Some(0x28),
            },
            Chip::Esp32c6 => SpiRegisters {
                base: 0x6000_3000,
                usr_offset: 0x18,
                usr1_offset: 0x1c,
                usr2_offset: 0x20,
                w0_offset: 0x58,
                mosi_length_offset: Some(0x24),
                miso_length_offset: Some(0x28),
            },
            Chip::Esp32h2 => SpiRegisters {
                base: 0x6000_3000,
                usr_offset: 0x18,
                usr1_offset: 0x1c,
                usr2_offset: 0x20,
                w0_offset: 0x58,
                mosi_length_offset: Some(0x24),
                miso_length_offset: Some(0x28),
            },
            Chip::Esp32p4 => SpiRegisters {
                base: 0x5008_D000,
                usr_offset: 0x18,
                usr1_offset: 0x1c,
                usr2_offset: 0x20,
                w0_offset: 0x58,
                mosi_length_offset: Some(0x24),
                miso_length_offset: Some(0x28),
            },
            Chip::Esp32s2 => SpiRegisters {
                base: 0x3f40_3000,
                usr_offset: 0x18,
                usr1_offset: 0x1c,
                usr2_offset: 0x20,
                w0_offset: 0x58,
                mosi_length_offset: Some(0x24),
                miso_length_offset: Some(0x28),
            },
            Chip::Esp32s3 => SpiRegisters {
                base: 0x6000_3000,
                usr_offset: 0x18,
                usr1_offset: 0x1c,
                usr2_offset: 0x20,
                w0_offset: 0x58,
                mosi_length_offset: Some(0x24),
                miso_length_offset: Some(0x28),
            },
        }
    }

    /// Build targets supported by a chip
    pub fn supported_build_targets(&self) -> &[&str] {
        match self {
            Chip::Esp32 => &["xtensa-esp32-espidf", "xtensa-esp32-none-elf"],
            Chip::Esp32c2 => &["riscv32imc-esp-espidf", "riscv32imc-unknown-none-elf"],
            Chip::Esp32c3 => &["riscv32imc-esp-espidf", "riscv32imc-unknown-none-elf"],
            Chip::Esp32c5 => &["riscv32imac-esp-espidf", "riscv32imac-unknown-none-elf"],
            Chip::Esp32c6 => &["riscv32imac-esp-espidf", "riscv32imac-unknown-none-elf"],
            Chip::Esp32h2 => &["riscv32imac-esp-espidf", "riscv32imac-unknown-none-elf"],
            Chip::Esp32p4 => &["riscv32imafc-esp-espidf", "riscv32imafc-unknown-none-elf"],
            Chip::Esp32s2 => &["xtensa-esp32s2-espidf", "xtensa-esp32s2-none-elf"],
            Chip::Esp32s3 => &["xtensa-esp32s3-espidf", "xtensa-esp32s3-none-elf"],
        }
    }

    /// Is the build target `target` supported by the chip?
    pub fn supports_build_target(&self, target: &str) -> bool {
        self.supported_build_targets().contains(&target)
    }

    // Helper methods for chip-specific functionality

    #[cfg(feature = "serialport")]
    /// Return the package version based on the eFuses for ESP32
    fn esp32_package_version(&self, connection: &mut Connection) -> Result<u32, Error> {
        let word3 = self.read_efuse_raw(connection, 0, 3)?;

        let pkg_version = (word3 >> 9) & 0x7;
        let pkg_version = pkg_version + (((word3 >> 2) & 0x1) << 3);

        Ok(pkg_version)
    }

    #[cfg(feature = "serialport")]
    /// Return the block2 version based on eFuses for ESP32-S2
    fn esp32s2_block2_version(&self, connection: &mut Connection) -> Result<u32, Error> {
        self.read_efuse(connection, efuse::esp32s2::BLK_VERSION_MINOR)
    }

    #[cfg(feature = "serialport")]
    /// Return the flash version based on eFuses for ESP32-S2
    fn esp32s2_flash_version(&self, connection: &mut Connection) -> Result<u32, Error> {
        self.read_efuse(connection, efuse::esp32s2::FLASH_VERSION)
    }

    #[cfg(feature = "serialport")]
    /// Return the PSRAM version based on eFuses for ESP32-S2
    fn esp32s2_psram_version(&self, connection: &mut Connection) -> Result<u32, Error> {
        self.read_efuse(connection, efuse::esp32s2::PSRAM_VERSION)
    }

    #[cfg(feature = "serialport")]
    /// Return the major BLK version based on eFuses for ESP32-S3
    fn esp32s3_blk_version_major(&self, connection: &mut Connection) -> Result<u32, Error> {
        self.read_efuse(connection, efuse::esp32s3::BLK_VERSION_MAJOR)
    }

    #[cfg(feature = "serialport")]
    /// Return the minor BLK version based on eFuses for ESP32-S3
    fn esp32s3_blk_version_minor(&self, connection: &mut Connection) -> Result<u32, Error> {
        self.read_efuse(connection, efuse::esp32s3::BLK_VERSION_MINOR)
    }
}

impl TryFrom<u16> for Chip {
    type Error = Error;

    fn try_from(value: u16) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Chip::Esp32),
            12 => Ok(Chip::Esp32c2),
            5 => Ok(Chip::Esp32c3),
            23 => Ok(Chip::Esp32c5),
            13 => Ok(Chip::Esp32c6),
            16 => Ok(Chip::Esp32h2),
            18 => Ok(Chip::Esp32p4),
            2 => Ok(Chip::Esp32s2),
            9 => Ok(Chip::Esp32s3),
            _ => Err(Error::ChipDetectError(format!(
                "unrecognized chip ID: {value}"
            ))),
        }
    }
}

/// SPI register addresses
#[derive(Debug)]
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
    /// Get the base address of the SPI registers.
    pub fn cmd(&self) -> u32 {
        self.base
    }

    /// Get the address of the USR register.
    pub fn usr(&self) -> u32 {
        self.base + self.usr_offset
    }

    /// Get the address of the USR1 register.
    pub fn usr1(&self) -> u32 {
        self.base + self.usr1_offset
    }

    /// Get the address of the USR2 register.
    pub fn usr2(&self) -> u32 {
        self.base + self.usr2_offset
    }

    /// Get the address of the W0 register.
    pub fn w0(&self) -> u32 {
        self.base + self.w0_offset
    }

    /// Get the address of the MOSI length register.
    pub fn mosi_length(&self) -> Option<u32> {
        self.mosi_length_offset.map(|offset| self.base + offset)
    }

    /// Get the address of the MISO length register.
    pub fn miso_length(&self) -> Option<u32> {
        self.miso_length_offset.map(|offset| self.base + offset)
    }
}

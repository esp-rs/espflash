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
    target::efuse::EfuseBlock,
};
#[cfg(feature = "serialport")]
use crate::{
    connection::Connection,
    flasher::SpiAttachParams,
    target::efuse::{EfuseBlockErrors, EfuseField},
};

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
#[derive(
    Debug,
    Clone,
    Copy,
    Hash,
    PartialEq,
    Eq,
    Display,
    EnumIter,
    EnumString,
    VariantNames,
    Deserialize,
    Serialize,
)]
#[non_exhaustive]
#[serde(rename_all = "lowercase")]
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
            Chip::Esp32c5 => [0x1101_406f, 0x63e1_406f, 0x5fd1_406f].contains(&value),
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

    /// Returns the default flash frequency for the [Chip].
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

    /// Returns the default crystal frequency for the [Chip].
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

    /// Returns the offset of BLOCK0 relative to the eFuse base register
    /// address.
    pub fn block0_offset(&self) -> u32 {
        match self {
            Chip::Esp32 => 0x0,
            Chip::Esp32c2 => 0x2C,
            Chip::Esp32c3 => 0x2C,
            Chip::Esp32c5 => 0x2C,
            Chip::Esp32c6 => 0x2C,
            Chip::Esp32h2 => 0x2C,
            Chip::Esp32p4 => 0x2C,
            Chip::Esp32s2 => 0x2C,
            Chip::Esp32s3 => 0x2C,
        }
    }

    /// Returns the eFuse block definition of the specified block.
    fn block(&self, block: u32) -> Result<EfuseBlock, Error> {
        let blocks = match self {
            Chip::Esp32 => efuse::esp32::BLOCKS,
            Chip::Esp32c2 => efuse::esp32c2::BLOCKS,
            Chip::Esp32c3 => efuse::esp32c3::BLOCKS,
            Chip::Esp32c5 => efuse::esp32c5::BLOCKS,
            Chip::Esp32c6 => efuse::esp32c6::BLOCKS,
            Chip::Esp32h2 => efuse::esp32h2::BLOCKS,
            Chip::Esp32p4 => efuse::esp32p4::BLOCKS,
            Chip::Esp32s2 => efuse::esp32s2::BLOCKS,
            Chip::Esp32s3 => efuse::esp32s3::BLOCKS,
        };

        if block as usize >= blocks.len() {
            return Err(Error::InvalidEfuseBlock(block));
        }

        Ok(blocks[block as usize])
    }

    /// Return the error definitions for a given eFuse block.
    ///
    /// This returns `Ok(None)` on ESP32 as it has a completely different error
    /// handling scheme.
    #[cfg(feature = "serialport")]
    fn block_errors(self, block: EfuseBlock) -> Result<Option<EfuseBlockErrors>, Error> {
        let block_errors = match self {
            Chip::Esp32 => return Ok(None),
            Chip::Esp32c2 => efuse::esp32c2::defines::BLOCK_ERRORS,
            Chip::Esp32c3 => efuse::esp32c3::defines::BLOCK_ERRORS,
            Chip::Esp32c5 => efuse::esp32c5::defines::BLOCK_ERRORS,
            Chip::Esp32c6 => efuse::esp32c6::defines::BLOCK_ERRORS,
            Chip::Esp32h2 => efuse::esp32h2::defines::BLOCK_ERRORS,
            Chip::Esp32p4 => efuse::esp32p4::defines::BLOCK_ERRORS,
            Chip::Esp32s2 => efuse::esp32s2::defines::BLOCK_ERRORS,
            Chip::Esp32s3 => efuse::esp32s3::defines::BLOCK_ERRORS,
        };

        if block.index as usize >= block_errors.len() {
            return Err(Error::InvalidEfuseBlock(block.index.into()));
        }

        Ok(Some(block_errors[block.index as usize]))
    }

    /// Returns the size of the specified block for the implementing target.
    /// device
    pub fn block_size(&self, block: usize) -> u32 {
        let block = self.block(block as u32).unwrap();
        block.length as u32 * 4
    }

    /// Given an active connection, read the specified field of the eFuse
    /// region.
    #[cfg(feature = "serialport")]
    #[deprecated(note = "This only support u32. Use read_efuse_le instead.")]
    pub fn read_efuse(&self, connection: &mut Connection, field: EfuseField) -> Result<u32, Error> {
        if field.bit_count > 32 {
            return Err(Error::EfuseFieldTooLarge);
        }

        self.read_efuse_le::<u32>(connection, field)
    }

    /// Given an active connection, read the specified field of the eFuse
    /// in little endian order.
    #[cfg(feature = "serialport")]
    pub fn read_efuse_le<T: bytemuck::AnyBitPattern>(
        &self,
        connection: &mut Connection,
        field: EfuseField,
    ) -> Result<T, Error> {
        // this is a port of the corresponding function in esp-hal
        // see <https://github.com/esp-rs/esp-hal/blob/b992e944a61b19f6096af5958fd88bb05b02eec2/esp-hal/src/efuse/mod.rs#L119-L183>
        let EfuseField {
            block,
            bit_start,
            bit_count,
            ..
        } = field;

        fn read_raw(connection: &mut Connection, addr: u32) -> Result<u32, Error> {
            connection.read_reg(addr)
        }

        // Represent output value as a bytes slice:
        let mut output = std::mem::MaybeUninit::<T>::uninit();
        let mut bytes = unsafe {
            // see https://docs.rs/bytemuck/1.24.0/bytemuck/trait.AnyBitPattern.html
            // and https://docs.rs/bytemuck/1.24.0/bytemuck/trait.Pod.html
            std::slice::from_raw_parts_mut(output.as_mut_ptr() as *mut u8, std::mem::size_of::<T>())
        };

        let bit_off = bit_start;
        let bit_end = std::cmp::min(bit_count, (bytes.len() * 8) as u32) + bit_off;

        let mut last_word_off = bit_off / 32;
        let mut last_word = read_raw(
            connection,
            self.block(block)?.read_address + last_word_off * 4,
        )?;

        let word_bit_off = bit_off % 32;
        let word_bit_ext = 32 - word_bit_off;

        let mut word_off = last_word_off;
        for bit_off in (bit_off..bit_end).step_by(32) {
            if word_off != last_word_off {
                // Read a new word:
                last_word_off = word_off;
                last_word = read_raw(
                    connection,
                    self.block(block)?.read_address + last_word_off * 4,
                )?;
            }

            let mut word = last_word >> word_bit_off;
            word_off += 1;

            let word_bit_len = std::cmp::min(bit_end - bit_off, 32);
            if word_bit_len > word_bit_ext {
                // Read the next word:
                last_word_off = word_off;
                last_word = read_raw(
                    connection,
                    self.block(block)?.read_address + last_word_off * 4,
                )?;
                // Append bits from a beginning of the next word:
                word |= last_word.wrapping_shl(32 - word_bit_off);
            };

            if word_bit_len < 32 {
                // Mask only needed bits of a word:
                word &= u32::MAX >> (32 - word_bit_len);
            }

            // Represent word as a byte slice:
            let byte_len = word_bit_len.div_ceil(8);
            let word_bytes = unsafe {
                std::slice::from_raw_parts(&word as *const u32 as *const u8, byte_len as usize)
            };

            // Copy word bytes to output value bytes:
            bytes[..byte_len as usize].copy_from_slice(word_bytes);

            // Move read window forward:
            bytes = &mut bytes[(byte_len as usize)..];
        }

        // Fill untouched bytes with zeros:
        bytes.fill(0);

        Ok(unsafe { output.assume_init() })
    }

    /// Read the raw word in the specified eFuse block, without performing any
    /// bit-shifting or masking of the read value.
    #[cfg(feature = "serialport")]
    pub fn read_efuse_raw(
        &self,
        connection: &mut Connection,
        block: u32,
        word: u32,
    ) -> Result<u32, Error> {
        let addr = self.block(block)?.read_address + (word * 0x4);

        connection.read_reg(addr)
    }

    /// Returns whether the provided address `addr` in flash.
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

    /// Enumerate the chip's features.
    #[cfg(feature = "serialport")]
    pub fn chip_features(&self, connection: &mut Connection) -> Result<Vec<&str>, Error> {
        match self {
            Chip::Esp32 => {
                let mut features = vec!["WiFi"];

                let disable_bt = self.read_efuse_le::<u32>(connection, efuse::esp32::DISABLE_BT)?;
                if disable_bt == 0 {
                    features.push("BT");
                }

                let disable_app_cpu =
                    self.read_efuse_le::<u32>(connection, efuse::esp32::DISABLE_APP_CPU)?;
                if disable_app_cpu == 0 {
                    features.push("Dual Core");
                } else {
                    features.push("Single Core");
                }

                let chip_cpu_freq_rated =
                    self.read_efuse_le::<u32>(connection, efuse::esp32::CHIP_CPU_FREQ_RATED)?;
                if chip_cpu_freq_rated != 0 {
                    let chip_cpu_freq_low =
                        self.read_efuse_le::<u32>(connection, efuse::esp32::CHIP_CPU_FREQ_LOW)?;
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

                let adc_vref = self.read_efuse_le::<u32>(connection, efuse::esp32::ADC_VREF)?;
                if adc_vref != 0 {
                    features.push("VRef calibration in efuse");
                }

                let blk3_part_reserve =
                    self.read_efuse_le::<u32>(connection, efuse::esp32::BLK3_PART_RESERVE)?;
                if blk3_part_reserve != 0 {
                    features.push("BLK3 partially reserved");
                }

                let coding_scheme =
                    self.read_efuse_le::<u32>(connection, efuse::esp32::CODING_SCHEME)?;
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
            Chip::Esp32c2 => {
                self.read_efuse_le::<u32>(connection, efuse::esp32c2::WAFER_VERSION_MAJOR)
            }
            Chip::Esp32c3 => {
                self.read_efuse_le::<u32>(connection, efuse::esp32c3::WAFER_VERSION_MAJOR)
            }
            Chip::Esp32c5 => {
                self.read_efuse_le::<u32>(connection, efuse::esp32c5::WAFER_VERSION_MAJOR)
            }
            Chip::Esp32c6 => {
                self.read_efuse_le::<u32>(connection, efuse::esp32c6::WAFER_VERSION_MAJOR)
            }
            Chip::Esp32h2 => {
                self.read_efuse_le::<u32>(connection, efuse::esp32h2::WAFER_VERSION_MAJOR)
            }
            Chip::Esp32p4 => {
                let hi =
                    self.read_efuse_le::<u32>(connection, efuse::esp32p4::WAFER_VERSION_MAJOR_HI)?;
                let lo =
                    self.read_efuse_le::<u32>(connection, efuse::esp32p4::WAFER_VERSION_MAJOR_LO)?;

                let version = (hi << 2) | lo;

                Ok(version)
            }
            Chip::Esp32s2 => {
                self.read_efuse_le::<u32>(connection, efuse::esp32s2::WAFER_VERSION_MAJOR)
            }
            Chip::Esp32s3 => {
                if self.esp32s3_blk_version_major(connection)? == 1
                    && self.esp32s3_blk_version_minor(connection)? == 1
                {
                    Ok(0)
                } else {
                    self.read_efuse_le::<u32>(connection, efuse::esp32s3::WAFER_VERSION_MAJOR)
                }
            }
        }
    }

    /// Returns the chip's minor version.
    #[cfg(feature = "serialport")]
    pub fn minor_version(&self, connection: &mut Connection) -> Result<u32, Error> {
        match self {
            Chip::Esp32 => self.read_efuse_le::<u32>(connection, efuse::esp32::WAFER_VERSION_MINOR),
            Chip::Esp32c2 => {
                self.read_efuse_le::<u32>(connection, efuse::esp32c2::WAFER_VERSION_MINOR)
            }
            Chip::Esp32c3 => {
                let hi =
                    self.read_efuse_le::<u32>(connection, efuse::esp32c3::WAFER_VERSION_MINOR_HI)?;
                let lo =
                    self.read_efuse_le::<u32>(connection, efuse::esp32c3::WAFER_VERSION_MINOR_LO)?;

                Ok((hi << 3) + lo)
            }
            Chip::Esp32c5 => {
                self.read_efuse_le::<u32>(connection, efuse::esp32c5::WAFER_VERSION_MINOR)
            }
            Chip::Esp32c6 => {
                self.read_efuse_le::<u32>(connection, efuse::esp32c6::WAFER_VERSION_MINOR)
            }
            Chip::Esp32h2 => {
                self.read_efuse_le::<u32>(connection, efuse::esp32h2::WAFER_VERSION_MINOR)
            }
            Chip::Esp32p4 => {
                self.read_efuse_le::<u32>(connection, efuse::esp32p4::WAFER_VERSION_MINOR)
            }
            Chip::Esp32s2 => {
                let hi =
                    self.read_efuse_le::<u32>(connection, efuse::esp32s2::WAFER_VERSION_MINOR_HI)?;
                let lo =
                    self.read_efuse_le::<u32>(connection, efuse::esp32s2::WAFER_VERSION_MINOR_LO)?;

                Ok((hi << 3) + lo)
            }
            Chip::Esp32s3 => {
                let hi =
                    self.read_efuse_le::<u32>(connection, efuse::esp32s3::WAFER_VERSION_MINOR_HI)?;
                let lo =
                    self.read_efuse_le::<u32>(connection, efuse::esp32s3::WAFER_VERSION_MINOR_LO)?;

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

        let mac0 = self.read_efuse_le::<u32>(connection, mac0_field)?;
        let mac1 = self.read_efuse_le::<u32>(connection, mac1_field)?;

        let bytes = ((mac1 as u64) << 32) | mac0 as u64;
        let bytes = bytes.to_be_bytes();
        let bytes = &bytes[2..];

        let mac_addr = bytes
            .iter()
            .map(|b| format!("{b:02x}"))
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
            Chip::Esp32c5 | Chip::Esp32c6 | Chip::Esp32h2 => SpiRegisters {
                base: 0x6000_3000,
                usr_offset: 0x18,
                usr1_offset: 0x1c,
                usr2_offset: 0x20,
                w0_offset: 0x58,
                mosi_length_offset: Some(0x24),
                miso_length_offset: Some(0x28),
            },
            Chip::Esp32c2 | Chip::Esp32c3 | Chip::Esp32s3 => SpiRegisters {
                base: 0x6000_2000,
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
                base: 0x3f40_2000,
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

    #[cfg(feature = "serialport")]
    /// Returns the package version based on the eFuses for ESP32
    fn esp32_package_version(&self, connection: &mut Connection) -> Result<u32, Error> {
        let word3 = self.read_efuse_raw(connection, 0, 3)?;

        let pkg_version = (word3 >> 9) & 0x7;
        let pkg_version = pkg_version + (((word3 >> 2) & 0x1) << 3);

        Ok(pkg_version)
    }

    #[cfg(feature = "serialport")]
    /// Returns the block2 version based on eFuses for ESP32-S2
    fn esp32s2_block2_version(&self, connection: &mut Connection) -> Result<u32, Error> {
        self.read_efuse_le::<u32>(connection, efuse::esp32s2::BLK_VERSION_MINOR)
    }

    #[cfg(feature = "serialport")]
    /// Returns the flash version based on eFuses for ESP32-S2
    fn esp32s2_flash_version(&self, connection: &mut Connection) -> Result<u32, Error> {
        self.read_efuse_le::<u32>(connection, efuse::esp32s2::FLASH_VERSION)
    }

    #[cfg(feature = "serialport")]
    /// Returns the PSRAM version based on eFuses for ESP32-S2
    fn esp32s2_psram_version(&self, connection: &mut Connection) -> Result<u32, Error> {
        self.read_efuse_le::<u32>(connection, efuse::esp32s2::PSRAM_VERSION)
    }

    #[cfg(feature = "serialport")]
    /// Returns the major BLK version based on eFuses for ESP32-S3
    fn esp32s3_blk_version_major(&self, connection: &mut Connection) -> Result<u32, Error> {
        self.read_efuse_le::<u32>(connection, efuse::esp32s3::BLK_VERSION_MAJOR)
    }

    #[cfg(feature = "serialport")]
    /// Returns the minor BLK version based on eFuses for ESP32-S3
    fn esp32s3_blk_version_minor(&self, connection: &mut Connection) -> Result<u32, Error> {
        self.read_efuse_le::<u32>(connection, efuse::esp32s3::BLK_VERSION_MINOR)
    }
}

#[cfg(feature = "serialport")]
impl Chip {
    /// Poll the eFuse controller status until it's idle.
    fn wait_efuse_idle(self, connection: &mut Connection) -> Result<(), Error> {
        let (cmd_reg, cmds) = match self {
            Chip::Esp32 => (efuse::esp32::defines::EFUSE_REG_CMD, u32::MAX),
            Chip::Esp32c2 => (
                efuse::esp32c2::defines::EFUSE_CMD_REG,
                efuse::esp32c2::defines::EFUSE_PGM_CMD | efuse::esp32c2::defines::EFUSE_READ_CMD,
            ),
            Chip::Esp32c3 => (
                efuse::esp32c3::defines::EFUSE_CMD_REG,
                efuse::esp32c3::defines::EFUSE_PGM_CMD | efuse::esp32c3::defines::EFUSE_READ_CMD,
            ),
            Chip::Esp32c5 => (
                efuse::esp32c5::defines::EFUSE_CMD_REG,
                efuse::esp32c5::defines::EFUSE_PGM_CMD | efuse::esp32c5::defines::EFUSE_READ_CMD,
            ),
            Chip::Esp32c6 => (
                efuse::esp32c6::defines::EFUSE_CMD_REG,
                efuse::esp32c6::defines::EFUSE_PGM_CMD | efuse::esp32c6::defines::EFUSE_READ_CMD,
            ),
            Chip::Esp32h2 => (
                efuse::esp32h2::defines::EFUSE_CMD_REG,
                efuse::esp32h2::defines::EFUSE_PGM_CMD | efuse::esp32h2::defines::EFUSE_READ_CMD,
            ),
            Chip::Esp32p4 => (
                efuse::esp32p4::defines::EFUSE_CMD_REG,
                efuse::esp32p4::defines::EFUSE_PGM_CMD | efuse::esp32p4::defines::EFUSE_READ_CMD,
            ),
            Chip::Esp32s2 => (
                efuse::esp32s2::defines::EFUSE_CMD_REG,
                efuse::esp32s2::defines::EFUSE_PGM_CMD | efuse::esp32s2::defines::EFUSE_READ_CMD,
            ),
            Chip::Esp32s3 => (
                efuse::esp32s3::defines::EFUSE_CMD_REG,
                efuse::esp32s3::defines::EFUSE_PGM_CMD | efuse::esp32s3::defines::EFUSE_READ_CMD,
            ),
        };

        // `esptool` has a 0.25 second timeout.
        let deadline = std::time::Instant::now() + std::time::Duration::from_millis(250);
        while std::time::Instant::now() < deadline {
            // Wait until `EFUSE_CMD_REG` reads as zero twice in a row.  `esptool.py` says
            // that "due to a hardware error, we have to read READ_CMD again to
            // make sure the efuse clock is normal" but doesn't provide any
            // references.  See if this is documented in the errata.
            if (connection.read_reg(cmd_reg)? & cmds) != 0 {
                continue;
            }
            if (connection.read_reg(cmd_reg)? & cmds) != 0 {
                continue;
            }

            return Ok(());
        }

        Err(Error::TimedOutWaitingForEfuseController)
    }

    /// Configure the eFuse controller for writing.
    fn configure_efuse_write_timing(&self, connection: &mut Connection) -> Result<(), Error> {
        self.wait_efuse_idle(connection)?;
        let xtal_freq = self.xtal_frequency(connection)?;

        match self {
            Chip::Esp32 => {
                let (clk_sel0, clk_sel1, dac_clk_div) = match xtal_freq {
                    XtalFrequency::_26Mhz => (250, 255, 52),
                    XtalFrequency::_40Mhz => (160, 255, 80),
                    other => {
                        return Err(Error::UnsupportedXtalFrequency(format!(
                            "Only 26 MHz and 40 MHz is supported (xtal was {other})"
                        )));
                    }
                };

                connection.update_reg(
                    efuse::esp32::defines::EFUSE_DAC_CONF_REG,
                    efuse::esp32::defines::EFUSE_DAC_CLK_DIV_MASK,
                    dac_clk_div,
                )?;
                connection.update_reg(
                    efuse::esp32::defines::EFUSE_CLK_REG,
                    efuse::esp32::defines::EFUSE_CLK_SEL0_MASK,
                    clk_sel0,
                )?;
                connection.update_reg(
                    efuse::esp32::defines::EFUSE_CLK_REG,
                    efuse::esp32::defines::EFUSE_CLK_SEL1_MASK,
                    clk_sel1,
                )?;
            }

            Chip::Esp32c2 => {
                if ![XtalFrequency::_26Mhz, XtalFrequency::_40Mhz].contains(&xtal_freq) {
                    return Err(Error::UnsupportedXtalFrequency(format!(
                        "Only 26 MHz and 40 MHz is supported (xtal was {xtal_freq})"
                    )));
                }

                connection.update_reg(
                    efuse::esp32c2::defines::EFUSE_DAC_CONF_REG,
                    efuse::esp32c2::defines::EFUSE_DAC_NUM_M,
                    0xFF,
                )?;
                connection.update_reg(
                    efuse::esp32c2::defines::EFUSE_DAC_CONF_REG,
                    efuse::esp32c2::defines::EFUSE_DAC_CLK_DIV_M,
                    0x28,
                )?;
                connection.update_reg(
                    efuse::esp32c2::defines::EFUSE_WR_TIM_CONF1_REG,
                    efuse::esp32c2::defines::EFUSE_PWR_ON_NUM_M,
                    0x3000,
                )?;
                connection.update_reg(
                    efuse::esp32c2::defines::EFUSE_WR_TIM_CONF2_REG,
                    efuse::esp32c2::defines::EFUSE_PWR_OFF_NUM_M,
                    0x190,
                )?;

                let tpgm_inactive_val = if xtal_freq == XtalFrequency::_40Mhz {
                    200
                } else {
                    130
                };
                connection.update_reg(
                    efuse::esp32c2::defines::EFUSE_WR_TIM_CONF0_REG,
                    efuse::esp32c2::defines::EFUSE_TPGM_INACTIVE_M,
                    tpgm_inactive_val,
                )?;
            }

            Chip::Esp32c3 => {
                if xtal_freq != XtalFrequency::_40Mhz {
                    return Err(Error::UnsupportedXtalFrequency(format!(
                        "Only 40 MHz is supported (xtal was {xtal_freq})"
                    )));
                }

                connection.update_reg(
                    efuse::esp32c3::defines::EFUSE_DAC_CONF_REG,
                    efuse::esp32c3::defines::EFUSE_DAC_NUM_M,
                    0xFF,
                )?;
                connection.update_reg(
                    efuse::esp32c3::defines::EFUSE_DAC_CONF_REG,
                    efuse::esp32c3::defines::EFUSE_DAC_CLK_DIV_M,
                    0x28,
                )?;
                connection.update_reg(
                    efuse::esp32c3::defines::EFUSE_WR_TIM_CONF1_REG,
                    efuse::esp32c3::defines::EFUSE_PWR_ON_NUM_M,
                    0x3000,
                )?;
                connection.update_reg(
                    efuse::esp32c3::defines::EFUSE_WR_TIM_CONF2_REG,
                    efuse::esp32c3::defines::EFUSE_PWR_OFF_NUM_M,
                    0x190,
                )?;
            }

            Chip::Esp32c5 => {
                if ![XtalFrequency::_40Mhz, XtalFrequency::_48Mhz].contains(&xtal_freq) {
                    return Err(Error::UnsupportedXtalFrequency(format!(
                        "Only 40 MHz and 48 MHz is supported (xtal was {xtal_freq})"
                    )));
                }

                connection.update_reg(
                    efuse::esp32c5::defines::EFUSE_DAC_CONF_REG,
                    efuse::esp32c5::defines::EFUSE_DAC_NUM_M,
                    0xFF,
                )?;
                connection.update_reg(
                    efuse::esp32c5::defines::EFUSE_DAC_CONF_REG,
                    efuse::esp32c5::defines::EFUSE_DAC_CLK_DIV_M,
                    0x28,
                )?;
                connection.update_reg(
                    efuse::esp32c5::defines::EFUSE_WR_TIM_CONF1_REG,
                    efuse::esp32c5::defines::EFUSE_PWR_ON_NUM_M,
                    0x3000,
                )?;
                connection.update_reg(
                    efuse::esp32c5::defines::EFUSE_WR_TIM_CONF2_REG,
                    efuse::esp32c5::defines::EFUSE_PWR_OFF_NUM_M,
                    0x190,
                )?;
            }

            Chip::Esp32c6 => {
                if xtal_freq != XtalFrequency::_40Mhz {
                    return Err(Error::UnsupportedXtalFrequency(format!(
                        "Only 40 MHz is supported (xtal was {xtal_freq})"
                    )));
                }

                connection.update_reg(
                    efuse::esp32c6::defines::EFUSE_DAC_CONF_REG,
                    efuse::esp32c6::defines::EFUSE_DAC_NUM_M,
                    0xFF,
                )?;
                connection.update_reg(
                    efuse::esp32c6::defines::EFUSE_DAC_CONF_REG,
                    efuse::esp32c6::defines::EFUSE_DAC_CLK_DIV_M,
                    0x28,
                )?;
                connection.update_reg(
                    efuse::esp32c6::defines::EFUSE_WR_TIM_CONF1_REG,
                    efuse::esp32c6::defines::EFUSE_PWR_ON_NUM_M,
                    0x3000,
                )?;
                connection.update_reg(
                    efuse::esp32c6::defines::EFUSE_WR_TIM_CONF2_REG,
                    efuse::esp32c6::defines::EFUSE_PWR_OFF_NUM_M,
                    0x190,
                )?;
            }

            Chip::Esp32h2 => {
                if xtal_freq != XtalFrequency::_32Mhz {
                    return Err(Error::UnsupportedXtalFrequency(format!(
                        "Only 32 MHz is supported (xtal was {xtal_freq})"
                    )));
                }

                connection.update_reg(
                    efuse::esp32s3::defines::EFUSE_DAC_CONF_REG,
                    efuse::esp32s3::defines::EFUSE_DAC_NUM_M,
                    0xFF,
                )?;
                connection.update_reg(
                    efuse::esp32s3::defines::EFUSE_DAC_CONF_REG,
                    efuse::esp32s3::defines::EFUSE_DAC_CLK_DIV_M,
                    0x28,
                )?;
                connection.update_reg(
                    efuse::esp32s3::defines::EFUSE_WR_TIM_CONF1_REG,
                    efuse::esp32s3::defines::EFUSE_PWR_ON_NUM_M,
                    0x3000,
                )?;
                connection.update_reg(
                    efuse::esp32s3::defines::EFUSE_WR_TIM_CONF2_REG,
                    efuse::esp32s3::defines::EFUSE_PWR_OFF_NUM_M,
                    0x190,
                )?;
            }

            Chip::Esp32p4 => {
                if xtal_freq != XtalFrequency::_40Mhz {
                    return Err(Error::UnsupportedXtalFrequency(format!(
                        "Only 40 MHz is supported (xtal was {xtal_freq})"
                    )));
                }
            }

            Chip::Esp32s2 => {
                // The datasheet lists parameters for 80 MHz and 20 MHz as well, but `esptool`
                // doesn't support detecting either of those frequencies, so it seems like we
                // only need to support 40 MHz here?
                if xtal_freq != XtalFrequency::_40Mhz {
                    return Err(Error::UnsupportedXtalFrequency(format!(
                        "Only 40 MHz is supported (xtal was {xtal_freq})"
                    )));
                }

                // From `EFUSE_PROGRAMMING_TIMING_PARAMETERS` in
                // `espefuse/efuse/esp32s2/mem_definition.py`
                let (efuse_tsup_a, efuse_tpgm, efuse_thp_a, efuse_tpgm_inactive) =
                    (0x1, 0x190, 0x1, 0x2);
                connection.update_reg(
                    efuse::esp32s2::defines::EFUSE_WR_TIM_CONF1_REG,
                    efuse::esp32s2::defines::EFUSE_TSUP_A_M,
                    efuse_tsup_a,
                )?;
                connection.update_reg(
                    efuse::esp32s2::defines::EFUSE_WR_TIM_CONF0_REG,
                    efuse::esp32s2::defines::EFUSE_TPGM_M,
                    efuse_tpgm,
                )?;
                connection.update_reg(
                    efuse::esp32s2::defines::EFUSE_WR_TIM_CONF0_REG,
                    efuse::esp32s2::defines::EFUSE_THP_A_M,
                    efuse_thp_a,
                )?;
                connection.update_reg(
                    efuse::esp32s2::defines::EFUSE_WR_TIM_CONF0_REG,
                    efuse::esp32s2::defines::EFUSE_TPGM_INACTIVE_M,
                    efuse_tpgm_inactive,
                )?;

                // From `VDDQ_TIMING_PARAMETERS` in `espefuse/efuse/esp32s2/mem_definition.py`
                let (efuse_dac_clk_div, efuse_pwr_on_num, efuse_pwr_off_num) = (0x50, 0x5100, 0x80);
                connection.update_reg(
                    efuse::esp32s2::defines::EFUSE_DAC_CONF_REG,
                    efuse::esp32s2::defines::EFUSE_DAC_CLK_DIV_M,
                    efuse_dac_clk_div,
                )?;
                connection.update_reg(
                    efuse::esp32s2::defines::EFUSE_WR_TIM_CONF1_REG,
                    efuse::esp32s2::defines::EFUSE_PWR_ON_NUM_M,
                    efuse_pwr_on_num,
                )?;
                connection.update_reg(
                    efuse::esp32s2::defines::EFUSE_WR_TIM_CONF2_REG,
                    efuse::esp32s2::defines::EFUSE_PWR_OFF_NUM_M,
                    efuse_pwr_off_num,
                )?;

                // From `EFUSE_READING_PARAMETERS` in `espefuse/efuse/esp32s2/mem_definition.py`
                let (_efuse_tsur_a, efuse_trd, efuse_thr_a) = (0x1, 0x2, 0x1);
                // This is commented out in `esptool` for some reason.
                // TODO: Check TRM and ask `esptool` devs whether this is correct, and
                // preferably why.
                //
                // connection.update_reg(
                //     efuse::esp32s2::defines::EFUSE_RD_TIM_CONF_REG,
                //     efuse::esp32s2::defines::EFUSE_TSUR_A_M,
                //     efuse_tsur_a,
                // )?;
                connection.update_reg(
                    efuse::esp32s2::defines::EFUSE_RD_TIM_CONF_REG,
                    efuse::esp32s2::defines::EFUSE_TRD_M,
                    efuse_trd,
                )?;
                connection.update_reg(
                    efuse::esp32s2::defines::EFUSE_RD_TIM_CONF_REG,
                    efuse::esp32s2::defines::EFUSE_THR_A_M,
                    efuse_thr_a,
                )?;
            }

            Chip::Esp32s3 => {
                if xtal_freq != XtalFrequency::_40Mhz {
                    return Err(Error::UnsupportedXtalFrequency(format!(
                        "Only 40 MHz is supported (xtal was {xtal_freq})"
                    )));
                }

                connection.update_reg(
                    efuse::esp32s3::defines::EFUSE_DAC_CONF_REG,
                    efuse::esp32s3::defines::EFUSE_DAC_NUM_M,
                    0xFF,
                )?;
                connection.update_reg(
                    efuse::esp32s3::defines::EFUSE_DAC_CONF_REG,
                    efuse::esp32s3::defines::EFUSE_DAC_CLK_DIV_M,
                    0x28,
                )?;
                connection.update_reg(
                    efuse::esp32s3::defines::EFUSE_WR_TIM_CONF1_REG,
                    efuse::esp32s3::defines::EFUSE_PWR_ON_NUM_M,
                    0x3000,
                )?;
                connection.update_reg(
                    efuse::esp32s3::defines::EFUSE_WR_TIM_CONF2_REG,
                    efuse::esp32s3::defines::EFUSE_PWR_OFF_NUM_M,
                    0x190,
                )?;
            }
        }

        Ok(())
    }

    fn efuse_coding_scheme(
        self,
        connection: &mut Connection,
        block: EfuseBlock,
    ) -> Result<CodingScheme, Error> {
        // Block 0 always has coding scheme `None`.
        if block.index == 0 {
            return Ok(CodingScheme::None);
        }

        match self {
            Chip::Esp32 => {
                match self.read_efuse_le::<u32>(connection, efuse::esp32::CODING_SCHEME)? {
                    efuse::esp32::defines::CODING_SCHEME_NONE => Ok(CodingScheme::None),
                    efuse::esp32::defines::CODING_SCHEME_NONE_RECOVERY => Ok(CodingScheme::None),
                    efuse::esp32::defines::CODING_SCHEME_34 => Ok(CodingScheme::_34),
                    efuse::esp32::defines::CODING_SCHEME_REPEAT => {
                        Err(Error::UnsupportedEfuseCodingScheme("Repeat".to_owned()))
                    }
                    invalid => Err(Error::UnsupportedEfuseCodingScheme(format!(
                        "Invalid scheme: {invalid}"
                    ))),
                }
            }
            Chip::Esp32c2
            | Chip::Esp32c3
            | Chip::Esp32c5
            | Chip::Esp32c6
            | Chip::Esp32h2
            | Chip::Esp32p4
            | Chip::Esp32s2
            | Chip::Esp32s3 => Ok(CodingScheme::ReedSolomon),
        }
    }

    /// Trigger the eFuse controller to update its internal registers.
    fn trigger_efuse_register_read(&self, connection: &mut Connection) -> Result<(), Error> {
        self.wait_efuse_idle(connection)?;

        let (conf_reg, conf_val, cmd_reg, cmd_val) = match self {
            Chip::Esp32 => (
                efuse::esp32::defines::EFUSE_REG_CONF,
                efuse::esp32::defines::EFUSE_CONF_READ,
                efuse::esp32::defines::EFUSE_REG_CMD,
                efuse::esp32::defines::EFUSE_CMD_READ,
            ),
            Chip::Esp32c2 => (
                efuse::esp32c2::defines::EFUSE_CONF_REG,
                efuse::esp32c2::defines::EFUSE_READ_OP_CODE,
                efuse::esp32c2::defines::EFUSE_CMD_REG,
                efuse::esp32c2::defines::EFUSE_READ_CMD,
            ),
            Chip::Esp32c3 => (
                efuse::esp32c3::defines::EFUSE_CONF_REG,
                efuse::esp32c3::defines::EFUSE_READ_OP_CODE,
                efuse::esp32c3::defines::EFUSE_CMD_REG,
                efuse::esp32c3::defines::EFUSE_READ_CMD,
            ),
            Chip::Esp32c5 => (
                efuse::esp32c5::defines::EFUSE_CONF_REG,
                efuse::esp32c5::defines::EFUSE_READ_OP_CODE,
                efuse::esp32c5::defines::EFUSE_CMD_REG,
                efuse::esp32c5::defines::EFUSE_READ_CMD,
            ),
            Chip::Esp32c6 => (
                efuse::esp32c6::defines::EFUSE_CONF_REG,
                efuse::esp32c6::defines::EFUSE_READ_OP_CODE,
                efuse::esp32c6::defines::EFUSE_CMD_REG,
                efuse::esp32c6::defines::EFUSE_READ_CMD,
            ),
            Chip::Esp32h2 => (
                efuse::esp32h2::defines::EFUSE_CONF_REG,
                efuse::esp32h2::defines::EFUSE_READ_OP_CODE,
                efuse::esp32h2::defines::EFUSE_CMD_REG,
                efuse::esp32h2::defines::EFUSE_READ_CMD,
            ),
            Chip::Esp32p4 => (
                efuse::esp32p4::defines::EFUSE_CONF_REG,
                efuse::esp32p4::defines::EFUSE_READ_OP_CODE,
                efuse::esp32p4::defines::EFUSE_CMD_REG,
                efuse::esp32p4::defines::EFUSE_READ_CMD,
            ),
            Chip::Esp32s2 => (
                efuse::esp32s2::defines::EFUSE_CONF_REG,
                efuse::esp32s2::defines::EFUSE_READ_OP_CODE,
                efuse::esp32s2::defines::EFUSE_CMD_REG,
                efuse::esp32s2::defines::EFUSE_READ_CMD,
            ),
            Chip::Esp32s3 => (
                efuse::esp32s3::defines::EFUSE_CONF_REG,
                efuse::esp32s3::defines::EFUSE_READ_OP_CODE,
                efuse::esp32s3::defines::EFUSE_CMD_REG,
                efuse::esp32s3::defines::EFUSE_READ_CMD,
            ),
        };

        connection.write_reg(conf_reg, conf_val, None)?;
        connection.write_reg(cmd_reg, cmd_val, None)?;

        // TODO: `esptool.py` says that if `EFUSE_ENABLE_SECURITY_DOWNLOAD` or
        // `DIS_DOWNLOAD_MODE` was just set then we need to reconnect.  It also
        // uses `dlay_after_us=1000` on the `EFUSE_READ_CMD` write.

        self.wait_efuse_idle(connection)?;

        Ok(())
    }

    /// Check whether any errors occurred while writing the eFuse.
    ///
    /// Returns `Ok(true)` if write errors did occur.  Returns `Err(_)` if we
    /// failed to communicate with the chip.
    fn efuse_write_failed(
        self,
        connection: &mut Connection,
        block: EfuseBlock,
    ) -> Result<bool, Error> {
        let Some(block_errors) = self.block_errors(block)? else {
            // ESP32 chips can only detect write errors while using the 3/4 encoding scheme,
            // in other cases the return value is meaningless.
            if self.efuse_coding_scheme(connection, block)? != CodingScheme::_34 {
                return Ok(true);
            }

            let errors = connection.read_reg(efuse::esp32::defines::EFUSE_REG_DEC_STATUS)?
                & efuse::esp32::defines::EFUSE_REG_DEC_STATUS_MASK;

            return Ok(errors != 0);
        };

        if block.index == 0 {
            let (reg, count) = match self {
                Chip::Esp32 => unreachable!(),
                Chip::Esp32c2 => (efuse::esp32c2::defines::EFUSE_RD_REPEAT_ERR_REG, 1),
                Chip::Esp32c3 => (efuse::esp32c3::defines::EFUSE_RD_REPEAT_ERR0_REG, 5),
                Chip::Esp32c5 => (efuse::esp32c5::defines::EFUSE_RD_REPEAT_ERR0_REG, 5),
                Chip::Esp32c6 => (efuse::esp32c6::defines::EFUSE_RD_REPEAT_ERR0_REG, 5),
                Chip::Esp32h2 => (efuse::esp32h2::defines::EFUSE_RD_REPEAT_ERR0_REG, 5),
                Chip::Esp32p4 => (efuse::esp32p4::defines::EFUSE_RD_REPEAT_ERR0_REG, 5),
                Chip::Esp32s2 => (efuse::esp32s2::defines::EFUSE_RD_REPEAT_ERR0_REG, 5),
                Chip::Esp32s3 => (efuse::esp32s3::defines::EFUSE_RD_REPEAT_ERR0_REG, 5),
            };

            let errors = (0..count)
                .map(|idx| connection.read_reg(reg + (idx * 4)))
                .collect::<Result<Vec<_>, _>>()?;
            let any_errors = errors.into_iter().reduce(|a, b| a | b).unwrap() != 0;

            return Ok(any_errors);
        }

        let EfuseBlockErrors {
            err_num_reg,
            err_num_mask: Some(err_num_mask),
            err_num_offset: Some(err_num_offset),
            fail_bit_reg,
            fail_bit_offset: Some(fail_bit_offset),
        } = block_errors
        else {
            unreachable!("eFuse block errors weren't set for a non-BLOCK0 block");
        };

        let any_errors = connection.read_reg(err_num_reg)? & (err_num_mask << err_num_offset)
            | connection.read_reg(fail_bit_reg)? & (1 << fail_bit_offset);

        Ok(any_errors != 0)
    }

    fn clear_efuse_programming_registers(
        self,
        connection: &mut Connection,
        block: EfuseBlock,
    ) -> Result<(), Error> {
        self.wait_efuse_idle(connection)?;

        let words: u32 = if self == Chip::Esp32 {
            // All ESP32 eFuse blocks have 8 data registers with no separate check
            // registers.
            8
        } else {
            // All other chips a shared set of 8 data registers and 3 check registers.
            8 + 3
        };
        for word in 0..words {
            connection.write_reg(block.write_address + (word * 4), 0x00, None)?;
        }

        Ok(())
    }

    /// Write a value to an eFuse.
    pub fn write_efuse(
        self,
        connection: &mut Connection,
        block: u32,
        data: &[u8],
    ) -> Result<(), Error> {
        let block = self.block(block)?;
        if data.len() > (block.length as usize * 4) {
            return Err(Error::WritingEfuseFailed(format!(
                "Tried to write {} bytes to an eFuse of {} bytes",
                data.len(),
                block.length as usize * 4
            )));
        }

        self.configure_efuse_write_timing(connection)?;
        self.clear_efuse_programming_registers(connection, block)?;

        // Apply the coding scheme and convert the data into a vector of 4-byte words.
        let coded_data: Vec<u32> = {
            // Make sure that the data is padded with zeroes to the full size of the eFuse
            // block.
            let data = {
                let mut buf = vec![0u8; block.length as usize * 4];
                buf[0..data.len()].copy_from_slice(data);
                buf
            };

            let bytes = match self.efuse_coding_scheme(connection, block)? {
                CodingScheme::None => data,
                CodingScheme::_34 => {
                    return Err(Error::UnsupportedEfuseCodingScheme(
                        "3/4 coding is unimplemented".to_owned(),
                    ));
                }
                CodingScheme::ReedSolomon => reed_solomon::Encoder::new(12)
                    .encode(&data)
                    .iter()
                    .copied()
                    .collect(),
            };

            // Turn the vector of bytes into a vector of words.
            //
            // We know that the vector contains an even number of bytes because of how we
            // allocated this vector at the beginning of this block.
            bytes
                .chunks(4)
                .map(|bytes| u32::from_le_bytes(bytes.try_into().unwrap()))
                .collect()
        };

        // Figure out the correct programming commands to flash the eFuse.
        let (conf_reg, conf_val, cmd_reg, cmd_val) = match self {
            Chip::Esp32 => (
                efuse::esp32::defines::EFUSE_REG_CONF,
                efuse::esp32::defines::EFUSE_CONF_WRITE,
                efuse::esp32::defines::EFUSE_REG_CMD,
                efuse::esp32::defines::EFUSE_CMD_WRITE,
            ),
            Chip::Esp32c2 => (
                efuse::esp32c2::defines::EFUSE_CONF_REG,
                efuse::esp32c2::defines::EFUSE_WRITE_OP_CODE,
                efuse::esp32c2::defines::EFUSE_CMD_REG,
                efuse::esp32c2::defines::EFUSE_PGM_CMD | ((block.index as u32) << 2),
            ),
            Chip::Esp32c3 => (
                efuse::esp32c3::defines::EFUSE_CONF_REG,
                efuse::esp32c3::defines::EFUSE_WRITE_OP_CODE,
                efuse::esp32c3::defines::EFUSE_CMD_REG,
                efuse::esp32c3::defines::EFUSE_PGM_CMD | ((block.index as u32) << 2),
            ),
            Chip::Esp32c5 => (
                efuse::esp32c5::defines::EFUSE_CONF_REG,
                efuse::esp32c5::defines::EFUSE_WRITE_OP_CODE,
                efuse::esp32c5::defines::EFUSE_CMD_REG,
                efuse::esp32c5::defines::EFUSE_PGM_CMD | ((block.index as u32) << 2),
            ),
            Chip::Esp32c6 => (
                efuse::esp32c6::defines::EFUSE_CONF_REG,
                efuse::esp32c6::defines::EFUSE_WRITE_OP_CODE,
                efuse::esp32c6::defines::EFUSE_CMD_REG,
                efuse::esp32c6::defines::EFUSE_PGM_CMD | ((block.index as u32) << 2),
            ),
            Chip::Esp32h2 => (
                efuse::esp32h2::defines::EFUSE_CONF_REG,
                efuse::esp32h2::defines::EFUSE_WRITE_OP_CODE,
                efuse::esp32h2::defines::EFUSE_CMD_REG,
                efuse::esp32h2::defines::EFUSE_PGM_CMD | ((block.index as u32) << 2),
            ),
            Chip::Esp32p4 => (
                efuse::esp32p4::defines::EFUSE_CONF_REG,
                efuse::esp32p4::defines::EFUSE_WRITE_OP_CODE,
                efuse::esp32p4::defines::EFUSE_CMD_REG,
                efuse::esp32p4::defines::EFUSE_PGM_CMD | ((block.index as u32) << 2),
            ),
            Chip::Esp32s2 => (
                efuse::esp32s2::defines::EFUSE_CONF_REG,
                efuse::esp32s2::defines::EFUSE_WRITE_OP_CODE,
                efuse::esp32s2::defines::EFUSE_CMD_REG,
                efuse::esp32s2::defines::EFUSE_PGM_CMD | ((block.index as u32) << 2),
            ),
            Chip::Esp32s3 => (
                efuse::esp32s3::defines::EFUSE_CONF_REG,
                efuse::esp32s3::defines::EFUSE_WRITE_OP_CODE,
                efuse::esp32s3::defines::EFUSE_CMD_REG,
                efuse::esp32s3::defines::EFUSE_PGM_CMD | ((block.index as u32) << 2),
            ),
        };

        // Try to flash the eFuse up to 3 times in case not all bits ended up being
        // burned.
        let mut err = None;
        for _ in 0..3 {
            self.wait_efuse_idle(connection)?;

            // Write the encoded data to the block's write address.
            //
            // The check value registers for the Reed-Solomon follow after the data
            // registers.
            for (idx, word) in coded_data.iter().enumerate() {
                connection.write_reg(block.write_address + (idx as u32 * 4), *word, None)?;
            }

            // Trigger the eFuse write and wait for the burning process to finish.
            connection.write_reg(conf_reg, conf_val, None)?;
            connection.write_reg(cmd_reg, cmd_val, None)?;
            self.wait_efuse_idle(connection)?;

            // Clear the parameter registers to avoid leaking the programmed contents.
            self.clear_efuse_programming_registers(connection, block)?;

            // Trigger eFuse controller to update its internal registers.
            self.trigger_efuse_register_read(connection)?;

            // Got at least one error, try burning the eFuse again.
            if self.efuse_write_failed(connection, block)? {
                let _ = err.insert("eFuse controller returned unreliable burn");
                continue;
            }

            // Check that the bits we wrote are actually set.  If there are any differences
            // we perform the burn again.
            for word in 0..block.length {
                let rd_word = self.read_efuse_raw(connection, block.index.into(), word.into())?;
                let wr_word = coded_data[word as usize];
                if (rd_word & wr_word) != wr_word {
                    let _ = err.insert("Not all bits were set after burning");
                    continue;
                }
            }

            return Ok(());
        }

        // Reaching this point means that we failed to burn the eFuse 3 times in a row.
        Err(Error::WritingEfuseFailed(err.unwrap().to_string()))
    }
}

#[cfg(feature = "serialport")]
#[derive(PartialEq)]
enum CodingScheme {
    None,
    _34,
    ReedSolomon,
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
#[derive(Copy, Clone, Hash, Debug, PartialEq, Eq)]
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

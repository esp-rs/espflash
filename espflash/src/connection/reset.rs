//! Reset strategies for resetting a target device.
//!
//! This module defines the traits and types used for resetting a target device.

// Most of this module is copied from `esptool.py`:
// https://github.com/espressif/esptool/blob/a8586d0/esptool/reset.py

#[cfg(unix)]
use std::{io, os::fd::AsRawFd};
use std::{thread::sleep, time::Duration};

#[cfg(unix)]
use libc::ioctl;
use log::debug;
use serde::{Deserialize, Serialize};
use serialport::SerialPort;
use strum::{Display, EnumIter, EnumString, VariantNames};

use super::{Connection, Port, USB_SERIAL_JTAG_PID};
use crate::{
    Error,
    command::{Command, CommandType},
    flasher::FLASH_WRITE_SIZE,
};

/// Default time to wait before releasing the boot pin after a reset.
const DEFAULT_RESET_DELAY: u64 = 50; // ms
/// Amount of time to wait if the default reset delay does not work.
const EXTRA_RESET_DELAY: u64 = 500; // ms

/// Reset strategies for resetting a target device.
pub trait ResetStrategy {
    fn reset(&self, serial_port: &mut Port) -> Result<(), Error>;

    fn set_dtr(&self, serial_port: &mut Port, level: bool) -> Result<(), Error> {
        serial_port.write_data_terminal_ready(level)?;

        Ok(())
    }

    fn set_rts(&self, serial_port: &mut Port, level: bool) -> Result<(), Error> {
        serial_port.write_request_to_send(level)?;

        Ok(())
    }

    #[cfg(unix)]
    fn set_dtr_rts(
        &self,
        serial_port: &mut Port,
        dtr_level: bool,
        rts_level: bool,
    ) -> Result<(), Error> {
        let fd = serial_port.as_raw_fd();
        let mut status: i32 = 0;
        match unsafe { ioctl(fd, libc::TIOCMGET, &status) } {
            0 => (),
            _ => return Err(io::Error::last_os_error().into()),
        }

        if dtr_level {
            status |= libc::TIOCM_DTR
        } else {
            status &= !libc::TIOCM_DTR
        }

        if rts_level {
            status |= libc::TIOCM_RTS
        } else {
            status &= !libc::TIOCM_RTS
        }

        match unsafe { ioctl(fd, libc::TIOCMSET, &status) } {
            0 => (),
            _ => return Err(io::Error::last_os_error().into()),
        }
        Ok(())
    }
}

/// Classic reset sequence, sets DTR and RTS sequentially.
#[derive(Debug, Clone, Copy, Serialize, Hash, Deserialize)]
pub struct ClassicReset {
    delay: u64,
}

impl ClassicReset {
    /// Create a new `ClassicReset` strategy with the given delay.
    pub fn new(extra_delay: bool) -> Self {
        let delay = if extra_delay {
            EXTRA_RESET_DELAY
        } else {
            DEFAULT_RESET_DELAY
        };

        Self { delay }
    }
}

impl ResetStrategy for ClassicReset {
    fn reset(&self, serial_port: &mut Port) -> Result<(), Error> {
        debug!(
            "Using Classic reset strategy with delay of {}ms",
            self.delay
        );
        self.set_rts(serial_port, false)?;
        self.set_dtr(serial_port, false)?;

        self.set_rts(serial_port, true)?;
        self.set_dtr(serial_port, true)?;

        self.set_rts(serial_port, true)?; // EN = LOW, chip in reset
        self.set_dtr(serial_port, false)?; // IO0 = HIGH

        sleep(Duration::from_millis(100));

        self.set_rts(serial_port, false)?; // EN = HIGH, chip out of reset
        self.set_dtr(serial_port, true)?; // IO0 = LOW

        sleep(Duration::from_millis(self.delay));

        self.set_rts(serial_port, false)?;
        self.set_dtr(serial_port, false)?; // IO0 = HIGH, done

        Ok(())
    }
}

/// UNIX-only reset sequence with custom implementation, which allows setting
/// DTR and RTS lines at the same time.
#[cfg(unix)]
#[derive(Debug, Clone, Copy, Serialize, Hash, Deserialize)]
pub struct UnixTightReset {
    delay: u64,
}

#[cfg(unix)]
impl UnixTightReset {
    /// Create a new `UnixTightReset` strategy with the given delay.
    pub fn new(extra_delay: bool) -> Self {
        let delay = if extra_delay {
            EXTRA_RESET_DELAY
        } else {
            DEFAULT_RESET_DELAY
        };

        Self { delay }
    }
}

#[cfg(unix)]
impl ResetStrategy for UnixTightReset {
    fn reset(&self, serial_port: &mut Port) -> Result<(), Error> {
        debug!(
            "Using UnixTight reset strategy with delay of {}ms",
            self.delay
        );

        self.set_dtr_rts(serial_port, false, false)?;
        self.set_dtr_rts(serial_port, true, true)?;
        self.set_dtr_rts(serial_port, false, true)?; // IO = HIGH, EN = LOW, chip in reset

        sleep(Duration::from_millis(100));

        self.set_dtr_rts(serial_port, true, false)?; // IO0 = LOW, EN = HIGH, chip out of reset

        sleep(Duration::from_millis(self.delay));

        self.set_dtr_rts(serial_port, false, false)?; // IO0 = HIGH, done
        self.set_dtr(serial_port, false)?; // Needed in some environments to ensure IO0 = HIGH

        Ok(())
    }
}

/// Custom reset sequence, which is required when the device is connecting via
/// its USB-JTAG-Serial peripheral.
#[derive(Debug, Clone, Copy, Serialize, Hash, Deserialize)]
pub struct UsbJtagSerialReset;

impl ResetStrategy for UsbJtagSerialReset {
    fn reset(&self, serial_port: &mut Port) -> Result<(), Error> {
        debug!("Using UsbJtagSerial reset strategy");

        self.set_rts(serial_port, false)?;
        self.set_dtr(serial_port, false)?; // Idle

        sleep(Duration::from_millis(100));

        self.set_rts(serial_port, false)?;
        self.set_dtr(serial_port, true)?; // Set IO0

        sleep(Duration::from_millis(100));

        self.set_rts(serial_port, true)?; // Reset. Calls inverted to go through (1,1) instead of (0,0)
        self.set_dtr(serial_port, false)?;
        self.set_rts(serial_port, true)?; // RTS set as Windows only propagates DTR on RTS setting

        sleep(Duration::from_millis(100));

        self.set_rts(serial_port, false)?;
        self.set_dtr(serial_port, false)?;

        Ok(())
    }
}

/// Resets the target device.
pub fn reset_after_flash(serial: &mut Port, pid: u16) -> Result<(), serialport::Error> {
    sleep(Duration::from_millis(100));

    if pid == USB_SERIAL_JTAG_PID {
        serial.write_data_terminal_ready(false)?;

        sleep(Duration::from_millis(100));

        serial.write_request_to_send(true)?;
        serial.write_data_terminal_ready(false)?;
        serial.write_request_to_send(true)?;

        sleep(Duration::from_millis(100));

        serial.write_request_to_send(false)?;
    } else {
        serial.write_request_to_send(true)?;

        sleep(Duration::from_millis(100));

        serial.write_request_to_send(false)?;
    }

    Ok(())
}

/// Performs a hard reset of the chip.
pub fn hard_reset(serial_port: &mut Port, pid: u16) -> Result<(), Error> {
    debug!("Using HardReset reset strategy");

    // Using esptool HardReset strategy (https://github.com/espressif/esptool/blob/3301d0ff4638d4db1760a22540dbd9d07c55ec37/esptool/reset.py#L132-L153)
    // leads to https://github.com/esp-rs/espflash/issues/592 in Windows, using `reset_after_flash` instead works fine for all platforms.
    // We had similar issues in the past: https://github.com/esp-rs/espflash/pull/157
    reset_after_flash(serial_port, pid)?;

    Ok(())
}

/// Performs a soft reset of the device.
pub fn soft_reset(
    connection: &mut Connection,
    stay_in_bootloader: bool,
    is_stub: bool,
) -> Result<(), Error> {
    debug!("Using SoftReset reset strategy");
    if !is_stub {
        if stay_in_bootloader {
            // ROM bootloader is already in bootloader
            return Ok(());
        } else {
            //  'run user code' is as close to a soft reset as we can do
            connection.with_timeout(CommandType::FlashBegin.timeout(), |connection| {
                let size: u32 = 0;
                let offset: u32 = 0;
                let blocks: u32 = size.div_ceil(FLASH_WRITE_SIZE as u32);
                connection.command(Command::FlashBegin {
                    size,
                    blocks,
                    block_size: FLASH_WRITE_SIZE.try_into().unwrap(),
                    offset,
                    supports_encryption: false,
                })
            })?;
            connection.with_timeout(CommandType::FlashEnd.timeout(), |connection| {
                connection.write_command(Command::FlashEnd { reboot: false })
            })?;
        }
    } else if stay_in_bootloader {
        // Soft resetting from the stub loader will re-load the ROM bootloader
        connection.with_timeout(CommandType::FlashBegin.timeout(), |connection| {
            let size: u32 = 0;
            let offset: u32 = 0;
            let blocks: u32 = size.div_ceil(FLASH_WRITE_SIZE as u32);
            connection.command(Command::FlashBegin {
                size,
                blocks,
                block_size: FLASH_WRITE_SIZE.try_into().unwrap(),
                offset,
                supports_encryption: false,
            })
        })?;
        connection.with_timeout(CommandType::FlashEnd.timeout(), |connection| {
            connection.write_command(Command::FlashEnd { reboot: true })
        })?;
    } else {
        // Running user code from stub loader requires some hacks in the stub loader
        connection.with_timeout(CommandType::RunUserCode.timeout(), |connection| {
            connection.command(Command::RunUserCode)
        })?;
    }

    Ok(())
}

/// Constructs a sequence of reset strategies based on the OS and chip.
///
/// Returns a [Vec] containing one or more reset strategies to be attempted
/// sequentially.
#[allow(unused_variables)]
pub fn construct_reset_strategy_sequence(
    port_name: &str,
    pid: u16,
    mode: ResetBeforeOperation,
) -> Vec<Box<dyn ResetStrategy>> {
    // USB-JTAG/Serial mode
    if pid == USB_SERIAL_JTAG_PID || mode == ResetBeforeOperation::UsbReset {
        return vec![Box::new(UsbJtagSerialReset)];
    }

    // USB-to-Serial bridge
    #[cfg(unix)]
    if cfg!(unix) && !port_name.starts_with("rfc2217:") {
        return vec![
            Box::new(UnixTightReset::new(false)),
            Box::new(UnixTightReset::new(true)),
            Box::new(ClassicReset::new(false)),
            Box::new(ClassicReset::new(true)),
        ];
    }

    // Windows
    vec![
        Box::new(ClassicReset::new(false)),
        Box::new(ClassicReset::new(true)),
    ]
}

/// Enum to represent different reset behaviors before an operation.
#[cfg_attr(feature = "cli", derive(clap::ValueEnum))]
#[derive(
    Debug,
    Default,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Display,
    EnumIter,
    EnumString,
    VariantNames,
    Hash,
    Serialize,
    Deserialize,
)]
#[non_exhaustive]
#[strum(serialize_all = "lowercase")]
pub enum ResetBeforeOperation {
    /// Uses DTR & RTS serial control lines to try to reset the chip into
    /// bootloader mode.
    #[default]
    DefaultReset,
    /// Skips DTR/RTS control signal assignments and just start sending a serial
    /// synchronisation command to the chip.
    NoReset,
    /// Skips DTR/RTS control signal assignments and also skips the serial
    /// synchronization command.
    NoResetNoSync,
    /// Reset sequence for USB-JTAG-Serial peripheral.
    UsbReset,
}

/// Enum to represent different reset behaviors after an operation.
#[cfg_attr(feature = "cli", derive(clap::ValueEnum))]
#[derive(
    Debug,
    Default,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Display,
    EnumIter,
    EnumString,
    VariantNames,
    Hash,
    Serialize,
    Deserialize,
)]
#[non_exhaustive]
pub enum ResetAfterOperation {
    /// The DTR serial control line is used to reset the chip into a normal boot
    /// sequence.
    #[default]
    HardReset,
    /// Leaves the chip in the serial bootloader, no reset is performed.
    NoReset,
    /// Leaves the chip in the stub bootloader, no reset is performed.
    NoResetNoStub,
    /// Hard-resets the chip by triggering an internal watchdog reset.
    WatchdogReset,
}

//! This entire module is copied from `esptool.py` (https://github.com/espressif/esptool/blob/a8586d02b1305ebc687d31783437a7f4d4dbb70f/esptool/reset.py)

#[cfg(unix)]
use std::{io, os::fd::AsRawFd};
use std::{thread::sleep, time::Duration};
use strum::{Display, EnumIter, EnumString, EnumVariantNames};

use log::debug;

use crate::{
    command::{Command, CommandType},
    connection::Connection,
    connection::USB_SERIAL_JTAG_PID,
    error::Error,
    flasher,
    interface::Interface,
    targets::Chip,
};

/// Default time to wait before releasing the boot pin after a reset
const DEFAULT_RESET_DELAY: u64 = 50; // ms
/// Amount of time to wait if the default reset delay does not work
const EXTRA_RESET_DELAY: u64 = 500; // ms

#[cfg(unix)]
use libc::ioctl;

/// Some strategy for resting a target device
pub trait ResetStrategy {
    fn reset(&self, interface: &mut Interface) -> Result<(), Error>;

    fn set_dtr(&self, interface: &mut Interface, level: bool) -> Result<(), Error> {
        interface
            .serial_port_mut()
            .write_data_terminal_ready(level)?;

        Ok(())
    }

    fn set_rts(&self, interface: &mut Interface, level: bool) -> Result<(), Error> {
        interface.serial_port_mut().write_request_to_send(level)?;

        Ok(())
    }

    #[cfg(unix)]
    fn set_dtr_rts(
        &self,
        interface: &mut Interface,
        dtr_level: bool,
        rts_level: bool,
    ) -> Result<(), Error> {
        let fd = interface.as_raw_fd();
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
#[derive(Debug, Clone, Copy)]
pub struct ClassicReset {
    delay: u64,
}

impl ClassicReset {
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
    fn reset(&self, interface: &mut Interface) -> Result<(), Error> {
        debug!(
            "Using Classic reset strategy with delay of {}ms",
            self.delay
        );
        self.set_dtr(interface, false)?;
        self.set_rts(interface, false)?;

        self.set_dtr(interface, true)?;
        self.set_rts(interface, true)?;

        self.set_dtr(interface, false)?; // IO0 = HIGH
        self.set_rts(interface, true)?; // EN = LOW, chip in reset

        sleep(Duration::from_millis(100));

        self.set_dtr(interface, true)?; // IO0 = LOW
        self.set_rts(interface, false)?; // EN = HIGH, chip out of reset

        sleep(Duration::from_millis(self.delay));

        self.set_dtr(interface, false)?; // IO0 = HIGH, done
        self.set_rts(interface, false)?;

        Ok(())
    }
}

/// UNIX-only reset sequence with custom implementation, which allows setting
/// DTR and RTS lines at the same time.
#[cfg(unix)]
#[derive(Debug, Clone, Copy)]
pub struct UnixTightReset {
    delay: u64,
}

#[cfg(unix)]
impl UnixTightReset {
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
    fn reset(&self, interface: &mut Interface) -> Result<(), Error> {
        debug!(
            "Using UnixTight reset strategy with delay of {}ms",
            self.delay
        );

        self.set_dtr_rts(interface, false, false)?;
        self.set_dtr_rts(interface, true, true)?;
        self.set_dtr_rts(interface, false, true)?; // IO = HIGH, EN = LOW, chip in reset

        sleep(Duration::from_millis(100));

        self.set_dtr_rts(interface, true, false)?; // IO0 = LOW, EN = HIGH, chip out of reset

        sleep(Duration::from_millis(self.delay));

        self.set_dtr_rts(interface, false, false)?; // IO0 = HIGH, done
        self.set_dtr(interface, false)?; // Needed in some environments to ensure IO0 = HIGH

        Ok(())
    }
}

/// Custom reset sequence, which is required when the device is connecting via
/// its USB-JTAG-Serial peripheral.
#[derive(Debug, Clone, Copy)]
pub struct UsbJtagSerialReset;

impl ResetStrategy for UsbJtagSerialReset {
    fn reset(&self, interface: &mut Interface) -> Result<(), Error> {
        debug!("Using UsbJtagSerial reset strategy");

        self.set_dtr(interface, false)?; // Idle
        self.set_rts(interface, false)?;

        sleep(Duration::from_millis(100));

        self.set_dtr(interface, true)?; // Set IO0
        self.set_rts(interface, false)?;

        sleep(Duration::from_millis(100));

        self.set_rts(interface, true)?; // Reset. Calls inverted to go through (1,1) instead of (0,0)
        self.set_dtr(interface, false)?;
        self.set_rts(interface, true)?; // RTS set as Windows only propagates DTR on RTS setting

        sleep(Duration::from_millis(100));

        self.set_dtr(interface, false)?;
        self.set_rts(interface, false)?;

        Ok(())
    }
}

/// Reset sequence for hard resetting the chip.
///
/// Can be used to reset out of the bootloader or to restart a running app.
#[derive(Debug, Clone, Copy)]
pub struct HardReset;

impl ResetStrategy for HardReset {
    fn reset(&self, interface: &mut Interface) -> Result<(), Error> {
        debug!("Using HardReset reset strategy");

        self.set_rts(interface, true)?;
        sleep(Duration::from_millis(100));
        self.set_rts(interface, false)?;

        Ok(())
    }
}

///
pub fn soft_reset(
    connection: &mut Connection,
    stay_in_bootloader: bool,
    is_stub: bool,
    chip: Chip,
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
                let blocks: u32 = (size + flasher::FLASH_WRITE_SIZE as u32 - 1)
                    / flasher::FLASH_WRITE_SIZE as u32;
                connection.command(Command::FlashBegin {
                    size,
                    blocks,
                    block_size: flasher::FLASH_WRITE_SIZE.try_into().unwrap(),
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
            let blocks: u32 =
                (size + flasher::FLASH_WRITE_SIZE as u32 - 1) / flasher::FLASH_WRITE_SIZE as u32;
            connection.command(Command::FlashBegin {
                size,
                blocks,
                block_size: flasher::FLASH_WRITE_SIZE.try_into().unwrap(),
                offset,
                supports_encryption: false,
            })
        })?;
        connection.with_timeout(CommandType::FlashEnd.timeout(), |connection| {
            connection.write_command(Command::FlashEnd { reboot: true })
        })?;
    } else if chip != Chip::Esp8266 {
        return Err(Error::SoftResetNotAvailable);
    } else {
        // Running user code from stub loader requires some hacks in the stub loader
        connection.with_timeout(CommandType::RunUserCode.timeout(), |connection| {
            connection.command(Command::RunUserCode)
        })?;
    }

    Ok(())
}

/// Construct a sequence of reset strategies based on the OS and chip.
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

#[cfg_attr(feature = "cli", derive(clap::ValueEnum))]
#[derive(
    Debug, Default, Clone, Copy, PartialEq, Eq, Display, EnumIter, EnumString, EnumVariantNames,
)]
#[non_exhaustive]
#[strum(serialize_all = "lowercase")]
pub enum ResetBeforeOperation {
    /// Uses DTR & RTS serial control lines to try to reset the chip into bootloader mode.
    #[default]
    DefaultReset,
    /// Skips DTR/RTS control signal assignments and just start sending a serial synchronisation command to the chip.
    NoReset,
    /// Skips DTR/RTS control signal assignments and also skips the serial synchronization command.
    NoResetNoSync,
    /// Reset sequence for USB-JTAG-Serial peripheral
    UsbReset,
}

#[cfg_attr(feature = "cli", derive(clap::ValueEnum))]
#[derive(
    Debug, Default, Clone, Copy, PartialEq, Eq, Display, EnumIter, EnumString, EnumVariantNames,
)]
#[non_exhaustive]
pub enum ResetAfterOperation {
    /// The DTR serial control line is used to reset the chip into a normal boot sequence.
    #[default]
    HardReset,
    /// Runs the user firmware, but any subsequent reset will return to the serial bootloader.
    ///
    /// Only supported on ESP8266.
    SoftReset,
    /// Leaves the chip in the serial bootloader, no reset is performed.
    NoReset,
    /// Leaves the chip in the stub bootloader, no reset is performed.
    NoResetNoStub,
}

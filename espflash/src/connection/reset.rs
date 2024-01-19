//! This entire module is copied from `esptool.py` (https://github.com/espressif/esptool/blob/a8586d02b1305ebc687d31783437a7f4d4dbb70f/esptool/reset.py)

#[cfg(unix)]
use std::{io, os::fd::AsRawFd};
use std::{thread::sleep, time::Duration};

use log::debug;

use crate::{connection::USB_SERIAL_JTAG_PID, error::Error, interface::Interface};

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
        self.set_rts(interface, false)?;
        self.set_dtr(interface, false)?;

        self.set_rts(interface, true)?;
        self.set_dtr(interface, true)?;

        self.set_rts(interface, true)?; // EN = LOW, chip in reset
        self.set_dtr(interface, false)?; // IO0 = HIGH

        sleep(Duration::from_millis(100));

        self.set_rts(interface, false)?; // EN = HIGH, chip out of reset
        self.set_dtr(interface, true)?; // IO0 = LOW

        sleep(Duration::from_millis(self.delay));

        self.set_rts(interface, false)?;
        self.set_dtr(interface, false)?; // IO0 = HIGH, done

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

/// Construct a sequence of reset strategies based on the OS and chip.
///
/// Returns a [Vec] containing one or more reset strategies to be attempted
/// sequentially.
#[allow(unused_variables)]
pub fn construct_reset_strategy_sequence(port_name: &str, pid: u16) -> Vec<Box<dyn ResetStrategy>> {
    // USB-JTAG/Serial mode
    if pid == USB_SERIAL_JTAG_PID {
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

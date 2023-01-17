//! This entire module is copied from `esptool.py` :)

use std::{thread::sleep, time::Duration};

use log::debug;

use crate::{connection::USB_SERIAL_JTAG_PID, error::Error, interface::Interface};

/// Default time to wait before releasing the boot pin after a reset
const DEFAULT_RESET_DELAY: u64 = 50; // ms
/// Amount of time to wait if the default reset delay does not work
const EXTRA_RESET_DELAY: u64 = 550; // ms

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

        self.set_dtr(interface, false)?; // IO0 = HIGH
        self.set_rts(interface, true)?; // EN = LOW, chip in reset

        sleep(Duration::from_millis(100));

        self.set_dtr(interface, true)?; // IO0 = LOW
        self.set_rts(interface, false)?; // EN = HIGH, chip out of reset

        sleep(Duration::from_millis(self.delay));

        self.set_dtr(interface, false)?; // IO0 = HIGH, done

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

        self.set_rts(interface, false)?;
        self.set_dtr(interface, false)?; // Idle

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
pub fn construct_reset_strategy_sequence(port_name: &str, pid: u16) -> Vec<Box<dyn ResetStrategy>> {
    // USB-JTAG/Serial mode
    if pid == USB_SERIAL_JTAG_PID {
        return vec![Box::new(UsbJtagSerialReset)];
    }

    vec![
        Box::new(ClassicReset::new(false)),
        Box::new(ClassicReset::new(true)),
    ]
}

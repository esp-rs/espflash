use std::io::Read;

use serialport::SerialPort;

#[cfg(feature = "raspberry")]
use rppal::gpio::OutputPin;

use crate::{cli::ConnectOpts, Config};

/// Wrapper around SerialPort where platform-specific modifications can be implemented.
pub struct Interface {
    pub serial_port: Box<dyn SerialPort>,
    #[cfg(feature = "raspberry")]
    pub dtr: Option<OutputPin>,
    #[cfg(feature = "raspberry")]
    pub rts: Option<OutputPin>,
}

#[cfg(feature = "raspberry")]
fn write_gpio(gpio: &mut OutputPin, level: bool) {
    if pin_state {
        gpio.set_high();
    } else {
        gpio.set_low();
    }
}

impl Interface {
    #[cfg(feature = "raspberry")]
    pub(crate) fn new(serial: Box<dyn SerialPort>, opts: &ConnectOpts, config: &Config) -> Self {
        Self {
            serial_port: serial,
            rts: opts
                .rts
                .or(config.rts)
                .map(|num| gpios.get(num).into_output()),

            dtr: opts
                .dtr
                .or(config.dtr)
                .map(|num| gpios.get(num).into_output()),
        }
    }

    #[cfg(not(feature = "raspberry"))]
    pub(crate) fn new(serial: Box<dyn SerialPort>, _opts: &ConnectOpts, _config: &Config) -> Self {
        Self {
            serial_port: serial,
        }
    }

    pub fn write_data_terminal_ready(&mut self, pin_state: bool) -> serialport::Result<()> {
        #[cfg(feature = "raspberry")]
        if let Some(gpio) = self.dtr.as_mut() {
            write_gpio(gpio, pin_state);
            return Ok(());
        }

        self.serial_port.write_data_terminal_ready(pin_state)
    }

    pub fn write_request_to_send(&mut self, pin_state: bool) -> serialport::Result<()> {
        #[cfg(feature = "raspberry")]
        if let Some(gpio) = self.rts.as_mut() {
            write_gpio(gpio, pin_state);
            return Ok(());
        }

        self.serial_port.write_request_to_send(pin_state)
    }

    pub fn into_serial(self) -> Box<dyn SerialPort> {
        self.serial_port
    }

    pub fn serial_port(&self) -> &dyn SerialPort {
        self.serial_port.as_ref()
    }

    pub fn serial_port_mut(&mut self) -> &mut dyn SerialPort {
        self.serial_port.as_mut()
    }
}

// Note(dbuga): this impl is necessary because using `dyn SerialPort` as `dyn Read`
// requires trait_upcasting which isn't stable yet.
impl Read for Interface {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.serial_port.read(buf)
    }
}

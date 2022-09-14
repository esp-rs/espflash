use std::io::Read;

use serialport::SerialPort;

#[cfg(feature = "raspberry")]
use rppal::gpio::OutputPin;

use crate::{cli::ConnectOpts, Config};

#[derive(thiserror::Error, Debug)]
pub enum SerialConfigError {
    #[cfg(feature = "raspberry")]
    #[error("You need to specify DTR when using an internal UART peripheral")]
    MissingDtrForInternalUart,
}

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
    pub(crate) fn new(
        serial: Box<dyn SerialPort>,
        opts: &ConnectOpts,
        config: &Config,
    ) -> Result<Self, SerialConfigError> {
        let rts_gpio = opts.rts.or(config.rts);
        let dtr_gpio = opts.dtr.or(config.dtr);

        Ok(Self {
            serial_port: serial,
            rts: rts_gpio.map(|num| gpios.get(num).into_output()),
            dtr: dtr_gpio.map(|num| gpios.get(num).into_output()),
        })
    }

    #[cfg(not(feature = "raspberry"))]
    pub(crate) fn new(
        serial: Box<dyn SerialPort>,
        _opts: &ConnectOpts,
        _config: &Config,
    ) -> Result<Self, SerialConfigError> {
        Ok(Self {
            serial_port: serial,
        })
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

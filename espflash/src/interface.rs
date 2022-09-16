use std::io::Read;

use crate::{cli::ConnectOpts, Config, Error};
use miette::Context;
use serialport::{FlowControl, SerialPort, SerialPortInfo};

#[cfg(feature = "raspberry")]
use rppal::gpio::{Gpio, OutputPin};

#[derive(thiserror::Error, Debug)]
pub enum SerialConfigError {
    #[cfg(feature = "raspberry")]
    #[error("You need to specify both DTR and RTS pins when using an internal UART peripheral")]
    MissingDtrRtsForInternalUart,

    #[cfg(feature = "raspberry")]
    #[error("GPIO {0} is not available")]
    GpioUnavailable(u8),
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
        port_info: &SerialPortInfo,
        opts: &ConnectOpts,
        config: &Config,
    ) -> Result<Self, Error> {
        let rts_gpio = opts.rts.or(config.rts);
        let dtr_gpio = opts.dtr.or(config.dtr);

        if port_info.port_type == serialport::SerialPortType::Unknown
            && (dtr_gpio.is_none() || rts_gpio.is_none())
        {
            // Assume internal UART, which has no DTR pin and usually no RTS either.
            return Err(Error::from(SerialConfigError::MissingDtrRtsForInternalUart));
        }

        let mut gpios = Gpio::new().unwrap();

        let rts = if let Some(gpio) = rts_gpio {
            match gpios.get(gpio) {
                Ok(pin) => Some(pin.into_output()),
                Err(_) => return Err(SerialConfigError::GpioUnavailable),
            }
        } else {
            None
        };

        let dtr = if let Some(gpio) = dtr_gpio {
            match gpios.get(gpio) {
                Ok(pin) => Some(pin.into_output()),
                Err(_) => return Err(SerialConfigError::GpioUnavailable),
            }
        } else {
            None
        };

        let serial = serialport::new(&port_info.port_name, 115_200)
            .flow_control(FlowControl::None)
            .open()
            .map_err(Error::from)
            .wrap_err_with(|| format!("Failed to open serial port {}", port_info.port_name))?;

        Ok(Self {
            serial_port: serial,
            rts,
            dtr,
        })
    }

    #[cfg(not(feature = "raspberry"))]
    pub(crate) fn new(
        port_info: &SerialPortInfo,
        _opts: &ConnectOpts,
        _config: &Config,
    ) -> Result<Self, Error> {
        let serial = serialport::new(&port_info.port_name, 115_200)
            .flow_control(FlowControl::None)
            .open()
            .map_err(Error::from)
            .wrap_err_with(|| format!("Failed to open serial port {}", port_info.port_name))?;

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

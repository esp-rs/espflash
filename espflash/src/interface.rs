use std::io::Read;

use crate::{cli::ConnectArgs, Config, Error};
use miette::{Context, Result};
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
    if level {
        gpio.set_high();
    } else {
        gpio.set_low();
    }
}

fn open_port(port_info: &SerialPortInfo) -> Result<Box<dyn SerialPort>> {
    serialport::new(&port_info.port_name, 115_200)
        .flow_control(FlowControl::None)
        .open()
        .map_err(Error::from)
        .wrap_err_with(|| format!("Failed to open serial port {}", port_info.port_name))
}

impl Interface {
    #[cfg(feature = "raspberry")]
    pub(crate) fn new(
        port_info: &SerialPortInfo,
        args: &ConnectArgs,
        config: &Config,
    ) -> Result<Self> {
        let rts_gpio = args.rts.or(config.connection.rts);
        let dtr_gpio = args.dtr.or(config.connection.dtr);

        if port_info.port_type == serialport::SerialPortType::Unknown
            && (dtr_gpio.is_none() || rts_gpio.is_none())
        {
            // Assume internal UART, which has no DTR pin and usually no RTS either.
            return Err(Error::from(SerialConfigError::MissingDtrRtsForInternalUart).into());
        }

        let gpios = Gpio::new().unwrap();

        let rts = if let Some(gpio) = rts_gpio {
            match gpios.get(gpio) {
                Ok(pin) => Some(pin.into_output()),
                Err(_) => return Err(Error::from(SerialConfigError::GpioUnavailable(gpio)).into()),
            }
        } else {
            None
        };

        let dtr = if let Some(gpio) = dtr_gpio {
            match gpios.get(gpio) {
                Ok(pin) => Some(pin.into_output()),
                Err(_) => return Err(Error::from(SerialConfigError::GpioUnavailable(gpio)).into()),
            }
        } else {
            None
        };

        Ok(Self {
            serial_port: open_port(port_info)?,
            rts,
            dtr,
        })
    }

    #[cfg(not(feature = "raspberry"))]
    pub(crate) fn new(
        port_info: &SerialPortInfo,
        _args: &ConnectArgs,
        _config: &Config,
    ) -> Result<Self> {
        Ok(Self {
            serial_port: open_port(port_info)?,
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

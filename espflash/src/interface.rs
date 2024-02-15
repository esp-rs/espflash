//! Serial port wrapper to support platform-specific functionality

use std::io::Read;
#[cfg(unix)]
use std::os::fd::{AsRawFd, RawFd};

use miette::{Context, Result};
use serialport::{FlowControl, SerialPort, SerialPortInfo};

use crate::error::Error;

#[cfg(unix)]
type Port = serialport::TTYPort;
#[cfg(windows)]
type Port = serialport::COMPort;

/// Wrapper around SerialPort where platform-specific modifications can be
/// implemented.
pub struct Interface {
    /// Hardware serial port used for communication
    pub serial_port: Port,
}

/// Open a serial port
fn open_port(port_info: &SerialPortInfo) -> Result<Port> {
    serialport::new(&port_info.port_name, 115_200)
        .flow_control(FlowControl::None)
        .open_native()
        .map_err(Error::from)
        .wrap_err_with(|| format!("Failed to open serial port {}", port_info.port_name))
}

impl Interface {
    pub fn new(port_info: &SerialPortInfo) -> Result<Self> {
        Ok(Self {
            serial_port: open_port(port_info)?,
        })
    }

    /// Set the level of the DTR pin
    pub fn write_data_terminal_ready(&mut self, pin_state: bool) -> serialport::Result<()> {
        self.serial_port.write_data_terminal_ready(pin_state)
    }

    /// Set the level of the RTS pin
    pub fn write_request_to_send(&mut self, pin_state: bool) -> serialport::Result<()> {
        self.serial_port.write_request_to_send(pin_state)
    }

    /// Turn an [Interface] into a [SerialPort]
    pub fn into_serial(self) -> Box<dyn SerialPort> {
        Box::new(self.serial_port)
    }

    /// Turn an [Interface] into a `&`[SerialPort]
    pub fn serial_port(&self) -> &dyn SerialPort {
        &self.serial_port
    }

    /// Turn an [Interface] into a  `&mut `[SerialPort]
    pub fn serial_port_mut(&mut self) -> &mut dyn SerialPort {
        &mut self.serial_port
    }
}

// Note(dbuga): this `impl` is necessary because using `dyn SerialPort` as `dyn
// Read` requires trait_upcasting which isn't stable yet.
impl Read for Interface {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.serial_port.read(buf)
    }
}

#[cfg(unix)]
impl AsRawFd for Interface {
    fn as_raw_fd(&self) -> RawFd {
        self.serial_port.as_raw_fd()
    }
}

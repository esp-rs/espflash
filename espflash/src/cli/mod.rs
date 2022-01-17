/// CLI utilities shared between espflash and cargo-espflash
///
/// No stability guaranties apply
use config::Config;
use miette::{Result, WrapErr};
use serialport::{FlowControl, SerialPortType};

use self::clap::ConnectArgs;
use crate::{cli::serial::get_serial_port_info, error::Error, Flasher};

pub mod clap;
pub mod config;
pub mod monitor;

mod line_endings;
mod serial;

pub fn connect(matches: &ConnectArgs, config: &Config) -> Result<Flasher> {
    let port_info = get_serial_port_info(matches, config)?;

    // Attempt to open the serial port and set its initial baud rate.
    println!("Serial port: {}", port_info.port_name);
    println!("Connecting...\n");
    let serial = serialport::new(&port_info.port_name, 115_200)
        .flow_control(FlowControl::None)
        .open()
        .map_err(Error::from)
        .wrap_err_with(|| format!("Failed to open serial port {}", port_info.port_name))?;

    // NOTE: since `get_serial_port_info` filters out all non-USB serial ports, we
    //       can just pretend the remaining types don't exist here.
    let port_info = match port_info.port_type {
        SerialPortType::UsbPort(info) => info,
        _ => unreachable!(),
    };

    Ok(Flasher::connect(serial, port_info, matches.speed)?)
}

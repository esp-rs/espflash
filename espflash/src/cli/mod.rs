use self::clap::ConnectArgs;
/// CLI utilities shared between espflash and cargo-espflash
///
/// No stability guaranties applies
use config::Config;
use miette::{Result, WrapErr};
use serialport::FlowControl;

use crate::cli::serial::get_serial_port;
use crate::{error::Error, Flasher};

pub mod clap;
pub mod config;
mod line_endings;
pub mod monitor;
mod serial;

pub fn connect(matches: &ConnectArgs, config: &Config) -> Result<Flasher> {
    let port = get_serial_port(matches, config)?;

    // Attempt to open the serial port and set its initial baud rate.
    println!("Serial port: {}", port);
    println!("Connecting...\n");
    let serial = serialport::new(&port, 115_200)
        .flow_control(FlowControl::None)
        .open()
        .map_err(Error::from)
        .wrap_err_with(|| format!("Failed to open serial port {}", port))?;

    Ok(Flasher::connect(serial, matches.speed)?)
}

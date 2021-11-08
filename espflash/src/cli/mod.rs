/// CLI utilities shared between espflash and cargo-espflash
///
/// No stability guaranties applies
pub mod clap;
pub mod config;
mod line_endings;
pub mod monitor;
use self::clap::ConnectArgs;
use crate::error::Error;
use crate::Flasher;
use config::Config;
use miette::{Result, WrapErr};
use serialport::FlowControl;

pub fn get_serial_port(matches: &ConnectArgs, config: &Config) -> Option<String> {
    // The serial port must be specified, either as a command-line argument or in
    // the cargo configuration file. In the case that both have been provided the
    // command-line argument will take precedence.
    if let Some(serial) = &matches.serial {
        Some(serial.to_string())
    } else {
        config
            .connection
            .serial
            .as_ref()
            .map(|serial| serial.into())
    }
}

pub fn connect(matches: &ConnectArgs, config: &Config) -> Result<Flasher> {
    let port = get_serial_port(matches, config).ok_or(Error::NoSerial)?;

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

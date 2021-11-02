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
use miette::{IntoDiagnostic, Result, WrapErr};
use serial::{BaudRate, FlowControl, SerialPort};

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
    let mut serial = serial::open(&port)
        .map_err(Error::from)
        .wrap_err_with(|| format!("Failed to open serial port {}", port))?;
    serial
        .reconfigure(&|settings| {
            settings.set_flow_control(FlowControl::FlowNone);
            settings.set_baud_rate(BaudRate::Baud115200)?;
            Ok(())
        })
        .into_diagnostic()?;

    // Parse the baud rate if provided as as a command-line argument.
    let speed = matches.speed.map(BaudRate::from_speed);

    Ok(Flasher::connect(serial, speed)?)
}

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
use dialoguer::{theme::ColorfulTheme, Select};
use miette::{IntoDiagnostic, Result, WrapErr};
use serialport::{available_ports, FlowControl, SerialPortType};

fn get_serial_port(matches: &ConnectArgs, config: &Config) -> Result<String, Error> {
    // The serial port should be specified, either as a command-line argument or in
    // the cargo configuration file. In the case that both have been provided the
    // command-line argument will take precedence.
    //
    // If neither have been provided:
    //   a) if there is only one serial port detected, it will be used
    //   b) if there is more than one serial port detected, the user will be
    //      prompted to select one or exit
    if let Some(serial) = &matches.serial {
        Ok(serial.into())
    } else if let Some(serial) = &config.connection.serial {
        Ok(serial.into())
    } else if let Ok(ports) = detect_serial_ports() {
        let maybe_serial = if ports.len() > 1 {
            println!(
                "{} serial ports detected, please select one or press Ctrl+c to exit\n",
                ports.len()
            );
            let index = Select::with_theme(&ColorfulTheme::default())
                .items(&ports)
                .default(0)
                .interact()
                .unwrap();

            ports.get(index)
        } else {
            ports.get(0)
        };

        match maybe_serial {
            Some(serial) => Ok(serial.into()),
            None => Err(Error::NoSerial),
        }
    } else {
        Err(Error::NoSerial)
    }
}

fn detect_serial_ports() -> Result<Vec<String>> {
    // Find all available serial ports on the host and filter them down to only
    // those which are likely candidates for ESP devices. At this time we are only
    // interested in USB devices and no further filtering is being done, however
    // this may change.
    let ports = available_ports().into_diagnostic()?;
    let ports = ports
        .iter()
        .filter(|&port| matches!(&port.port_type, SerialPortType::UsbPort(..)));

    // Now that we have a vector of candidate serial ports, the only information we
    // need from them are the ports' names.
    let port_names = ports
        .cloned()
        .map(|port| port.port_name)
        .collect::<Vec<_>>();

    Ok(port_names)
}

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

/// CLI utilities shared between espflash and cargo-espflash
///
/// No stability guaranties applies
use config::Config;
use crossterm::style::Stylize;
use dialoguer::{theme::ColorfulTheme, Confirm, Select};
use miette::{IntoDiagnostic, Result, WrapErr};
use serialport::{available_ports, FlowControl, SerialPortInfo, SerialPortType};

use self::clap::ConnectArgs;
use crate::cli::config::UsbDevice;
use crate::{error::Error, Flasher};

pub mod clap;
pub mod config;
mod line_endings;
pub mod monitor;

fn get_serial_port(matches: &ConnectArgs, config: &Config) -> Result<String, Error> {
    // A serial port should be specified either as a command-line argument or in a
    // configuration file. In the case that both have been provided the command-line
    // argument takes precedence.
    //
    // Users may optionally specify the device's VID and PID in the configuration
    // file. If no VID/PID have been provided, the user will always be prompted to
    // select a serial device. If some VID/PID have been provided the user will be
    // prompted to select a serial device, unless there is only one found and its
    // VID/PID matches the configured values.
    if let Some(serial) = &matches.serial {
        Ok(serial.to_owned())
    } else if let Some(serial) = &config.connection.serial {
        Ok(serial.to_owned())
    } else if let Ok(ports) = detect_usb_serial_ports() {
        select_serial_port(ports, &config.usb_device)
    } else {
        Err(Error::NoSerial)
    }
}

fn detect_usb_serial_ports() -> Result<Vec<SerialPortInfo>> {
    let ports = available_ports().into_diagnostic()?;
    let ports = ports
        .iter()
        .filter_map(|port_info| match port_info.port_type {
            SerialPortType::UsbPort(..) => Some(port_info.to_owned()),
            _ => None,
        })
        .collect::<Vec<_>>();

    Ok(ports)
}

fn select_serial_port(ports: Vec<SerialPortInfo>, devices: &[UsbDevice]) -> Result<String, Error> {
    let device_matches = |info| devices.iter().any(|dev| dev.matches(info));

    if ports.len() > 1 {
        // Multiple serial ports detected
        println!(
            "Detected {} serial ports. Ports with VID/PID matching configured values are bolded.\n",
            ports.len()
        );

        let port_names = ports
            .iter()
            .map(|port_info| match &port_info.port_type {
                SerialPortType::UsbPort(info) => {
                    if device_matches(info) {
                        format!("{}", port_info.port_name.as_str().bold())
                    } else {
                        port_info.port_name.clone()
                    }
                }
                _ => port_info.port_name.clone(),
            })
            .collect::<Vec<_>>();
        let index = Select::with_theme(&ColorfulTheme::default())
            .items(&port_names)
            .default(0)
            .interact()?;

        match ports.get(index) {
            Some(port_info) => Ok(port_info.port_name.to_owned()),
            None => Err(Error::NoSerial),
        }
    } else if let [port] = ports.as_slice() {
        // Single serial port detected
        let port_name = port.port_name.clone();
        let port_info = match &port.port_type {
            SerialPortType::UsbPort(info) => info,
            _ => unreachable!(),
        };

        if device_matches(port_info)
            || Confirm::with_theme(&ColorfulTheme::default())
                .with_prompt(format!("Use serial port '{}'?", port_name))
                .interact()?
        {
            Ok(port_name)
        } else {
            Err(Error::NoSerial)
        }
    } else {
        // No serial ports detected
        Err(Error::NoSerial)
    }
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

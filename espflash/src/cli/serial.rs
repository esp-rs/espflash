use super::config::Config;
use crossterm::style::Stylize;
use dialoguer::{theme::ColorfulTheme, Confirm, Select};
use miette::{IntoDiagnostic, Result};
use serialport::{available_ports, SerialPortInfo, SerialPortType};

use super::clap::ConnectArgs;
use crate::cli::config::UsbDevice;
use crate::error::Error;

pub fn get_serial_port(matches: &ConnectArgs, config: &Config) -> Result<String, Error> {
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
        let (port, matches) = select_serial_port(ports, config)?;
        match port.port_type {
            SerialPortType::UsbPort(usb_info) if !matches => {
                if Confirm::with_theme(&ColorfulTheme::default())
                    .with_prompt("Remember this serial port for future use?")
                    .interact_opt()?
                    .unwrap_or_default()
                {
                    if let Err(e) = config.save_with(|config| {
                        config.usb_device.push(UsbDevice {
                            vid: usb_info.vid,
                            pid: usb_info.pid,
                        })
                    }) {
                        eprintln!("Failed to save config {:#}", e);
                    }
                }
            }
            _ => {}
        }
        Ok(port.port_name)
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

/// USB UART adapters which are known to be on common dev boards
const KNOWN_DEVICES: &[UsbDevice] = &[
    UsbDevice {
        vid: 0x10c4,
        pid: 0xea60,
    }, // Silicon Labs CP210x UART Bridge
    UsbDevice {
        vid: 0x1a86,
        pid: 0x7523,
    }, // QinHeng Electronics CH340 serial converter
];

fn select_serial_port(
    ports: Vec<SerialPortInfo>,
    config: &Config,
) -> Result<(SerialPortInfo, bool), Error> {
    let device_matches = |info| {
        config
            .usb_device
            .iter()
            .chain(KNOWN_DEVICES.iter())
            .any(|dev| dev.matches(info))
    };

    if ports.len() > 1 {
        // Multiple serial ports detected
        println!(
            "Detected {} serial ports. Ports with match a known common dev board are highlighted.\n",
            ports.len()
        );

        let port_names = ports
            .iter()
            .map(|port_info| match &port_info.port_type {
                SerialPortType::UsbPort(info) => {
                    let formatted = if device_matches(info) {
                        port_info.port_name.as_str().bold()
                    } else {
                        port_info.port_name.as_str().reset()
                    };
                    if let Some(product) = &info.product {
                        format!("{} - {}", formatted, product)
                    } else {
                        format!("{}", formatted)
                    }
                }
                _ => port_info.port_name.clone(),
            })
            .collect::<Vec<_>>();
        let index = Select::with_theme(&ColorfulTheme::default())
            .items(&port_names)
            .default(0)
            .interact_opt()?
            .ok_or(Error::Canceled)?;

        match ports.get(index) {
            Some(
                port_info
                @
                SerialPortInfo {
                    port_type: SerialPortType::UsbPort(usb_info),
                    ..
                },
            ) => Ok((port_info.clone(), device_matches(usb_info))),
            Some(port_info) => Ok((port_info.clone(), false)),
            None => Err(Error::NoSerial),
        }
    } else if let [port] = ports.as_slice() {
        // Single serial port detected
        let port_name = port.port_name.clone();
        let port_info = match &port.port_type {
            SerialPortType::UsbPort(info) => info,
            _ => unreachable!(),
        };

        if device_matches(port_info) {
            Ok((port.clone(), true))
        } else if Confirm::with_theme(&ColorfulTheme::default())
            .with_prompt({
                if let Some(product) = &port_info.product {
                    format!("Use serial port '{}' - {}?", port_name, product)
                } else {
                    format!("Use serial port '{}'?", port_name)
                }
            })
            .interact_opt()?
            .ok_or(Error::Canceled)?
        {
            Ok((port.clone(), false))
        } else {
            Err(Error::NoSerial)
        }
    } else {
        // No serial ports detected
        Err(Error::NoSerial)
    }
}

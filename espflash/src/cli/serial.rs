use crossterm::style::Stylize;
use dialoguer::{theme::ColorfulTheme, Confirm, Select};
use miette::{IntoDiagnostic, Result};
use serialport::{available_ports, SerialPortInfo, SerialPortType, UsbPortInfo};

use super::{config::Config, ConnectOpts};
use crate::{cli::config::UsbDevice, error::Error};

pub fn get_serial_port_info(
    matches: &ConnectOpts,
    config: &Config,
) -> Result<SerialPortInfo, Error> {
    // A serial port should be specified either as a command-line argument or in a
    // configuration file. In the case that both have been provided the command-line
    // argument takes precedence.
    //
    // Users may optionally specify the device's VID and PID in the configuration
    // file. If no VID/PID has been provided, the user will always be prompted to
    // select a serial port. If some VID and PID were provided then the user will
    // also be prompted to select a port, unless there is only one found and its VID
    // and PID match the configured values.
    //
    // The call to canonicalize() was originally added to resolve https://github.com/esp-rs/espflash/issues/177,
    // however, canonicalize  doesn't work (on Windows) with "dummy" device paths like `COM4`.
    // That's the reason we need to handle Windows/Posix differently.

    let ports = detect_usb_serial_ports().unwrap_or_default();

    if let Some(serial) = &matches.serial {
        #[cfg(not(target_os = "windows"))]
        let serial = std::fs::canonicalize(serial)?.to_string_lossy().to_string();
        find_serial_port(&ports, &serial)
    } else if let Some(serial) = &config.connection.serial {
        #[cfg(not(target_os = "windows"))]
        let serial = std::fs::canonicalize(serial)?.to_string_lossy().to_string();
        find_serial_port(&ports, &serial)
    } else {
        let (port, matches) = select_serial_port(ports, config)?;

        match &port.port_type {
            SerialPortType::UsbPort(usb_info) if !matches => {
                let remember = Confirm::with_theme(&ColorfulTheme::default())
                    .with_prompt("Remember this serial port for future use?")
                    .interact_opt()?
                    .unwrap_or_default();

                if remember {
                    // Allow this operation to fail without terminating the
                    // application, but inform the user if something goes wrong.
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

        Ok(port)
    }
}

/// Given a vector of `SerialPortInfo` structs, attempt to find and return one
/// whose `port_name` field matches the provided `name` argument.
fn find_serial_port(ports: &[SerialPortInfo], name: &str) -> Result<SerialPortInfo, Error> {
    let port_info = ports
        .iter()
        .find(|port| port.port_name.to_lowercase() == name.to_lowercase());

    if let Some(port) = port_info {
        Ok(port.to_owned())
    } else {
        Err(Error::SerialNotFound(name.to_owned()))
    }
}

/// serialport's autodetect doesn't provide any port information when using musl
/// linux we can do some manual parsing of sysfs to get the relevant bits
/// without udev
#[cfg(all(target_os = "linux", target_env = "musl"))]
fn detect_usb_serial_ports() -> Result<Vec<SerialPortInfo>> {
    use std::{
        fs::{read_link, read_to_string},
        path::PathBuf,
    };

    let ports = available_ports().into_diagnostic()?;
    let ports = ports
        .into_iter()
        .filter_map(|port_info| {
            // with musl, the paths we get are `/sys/class/tty/*`
            let path = PathBuf::from(&port_info.port_name);

            // this will give something like:
            // `/sys/devices/pci0000:00/0000:00:07.1/0000:0c:00.3/usb5/5-3/5-3.1/5-3.1:1.0/ttyUSB0/tty/ttyUSB0`
            let mut parent_dev = path.canonicalize().ok()?;

            // walk up 3 dirs to get to the device hosting the tty:
            // `/sys/devices/pci0000:00/0000:00:07.1/0000:0c:00.3/usb5/5-3/5-3.1/5-3.1:1.0`
            parent_dev.pop();
            parent_dev.pop();
            parent_dev.pop();

            // check that the device is using the usb subsystem
            read_link(parent_dev.join("subsystem"))
                .ok()
                .filter(|subsystem| subsystem.ends_with("usb"))?;

            let interface = read_to_string(parent_dev.join("interface"))
                .ok()
                .map(|s| s.trim().to_string());

            // /sys/devices/pci0000:00/0000:00:07.1/0000:0c:00.3/usb5/5-3/5-3.1
            parent_dev.pop();

            let vid = read_to_string(parent_dev.join("idVendor")).ok()?;
            let pid = read_to_string(parent_dev.join("idProduct")).ok()?;

            Some(SerialPortInfo {
                port_type: SerialPortType::UsbPort(UsbPortInfo {
                    vid: u16::from_str_radix(vid.trim(), 16).ok()?,
                    pid: u16::from_str_radix(pid.trim(), 16).ok()?,
                    product: interface,
                    serial_number: None,
                    manufacturer: None,
                }),
                port_name: format!("/dev/{}", path.file_name()?.to_str()?),
            })
        })
        .collect::<Vec<_>>();

    Ok(ports)
}

#[cfg(not(all(target_os = "linux", target_env = "musl")))]
fn detect_usb_serial_ports() -> Result<Vec<SerialPortInfo>> {
    let ports = available_ports().into_diagnostic()?;
    let ports = ports
        .into_iter()
        .filter(|port_info| {
            matches!(
                &port_info.port_type,
                SerialPortType::UsbPort(..) | SerialPortType::Unknown
            )
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
            "Detected {} serial ports. Ports which match a known common dev board are highlighted.\n",
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
                        formatted.to_string()
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
            Some(port_info) => {
                let matches = if let SerialPortType::UsbPort(usb_info) = &port_info.port_type {
                    device_matches(usb_info)
                } else {
                    false
                };

                Ok((port_info.to_owned(), matches))
            }
            None => Err(Error::SerialNotFound(
                port_names.get(index).unwrap().to_string(),
            )),
        }
    } else if let [port] = ports.as_slice() {
        // Single serial port detected
        let port_name = port.port_name.clone();
        let port_info = match &port.port_type {
            SerialPortType::UsbPort(info) => info,
            SerialPortType::Unknown => &UsbPortInfo {
                vid: 0,
                pid: 0,
                serial_number: None,
                manufacturer: None,
                product: None,
            },
            _ => unreachable!(),
        };

        if device_matches(port_info) {
            Ok((port.to_owned(), true))
        } else if confirm_port(&port_name, port_info)? {
            Ok((port.to_owned(), false))
        } else {
            Err(Error::SerialNotFound(port_name))
        }
    } else {
        // No serial ports detected
        Err(Error::NoSerial)
    }
}

fn confirm_port(port_name: &str, port_info: &UsbPortInfo) -> Result<bool, Error> {
    Confirm::with_theme(&ColorfulTheme::default())
        .with_prompt({
            if let Some(product) = &port_info.product {
                format!("Use serial port '{}' - {}?", port_name, product)
            } else {
                format!("Use serial port '{}'?", port_name)
            }
        })
        .interact_opt()?
        .ok_or(Error::Canceled)
}

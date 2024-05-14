#[cfg(not(target_os = "windows"))]
use std::fs;

use crossterm::style::Stylize;
use dialoguer::{theme::ColorfulTheme, Confirm, Select};
use log::{error, info};
use miette::{IntoDiagnostic, Result};
use serialport::{available_ports, SerialPortInfo, SerialPortType};

use crate::{
    cli::{config::UsbDevice, Config, ConnectArgs},
    error::Error,
};

/// Return the information of a serial port taking into account the different
/// ways of choosing a port.
pub fn get_serial_port_info(
    matches: &ConnectArgs,
    config: &Config,
) -> Result<SerialPortInfo, Error> {
    // A serial port should be specified either as a command-line argument or in a
    // configuration file. In the case that both have been provided the command-line
    // argument takes precedence.
    //
    // Users may optionally specify the device's VID and PID in the configuration
    // file. If no VID/PID has been provided, the user will always be prompted to
    // select a serial port. If some VID and PID were provided then the user will
    // also be prompted to select a port, unless there is only one found whose VID
    // and PID match the configured values.
    //
    // The call to canonicalize() was originally added to resolve
    // https://github.com/esp-rs/espflash/issues/177, however, canonicalize
    // doesn't work (on Windows) with "dummy" device paths like `COM4`. That's
    // the reason we need to handle Windows/Posix differently.

    let ports = detect_usb_serial_ports(matches.list_all_ports).unwrap_or_default();

    if let Some(serial) = &matches.port {
        find_serial_port(&ports, serial)
    } else if let Some(serial) = &config.connection.serial {
        find_serial_port(&ports, serial)
    } else {
        let (port, matches) = select_serial_port(ports, config, matches.confirm_port)?;

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
                        error!("Failed to save config {:#}", e);
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
    #[cfg(not(target_os = "windows"))]
    let name = fs::canonicalize(name)?;
    #[cfg(not(target_os = "windows"))]
    let name = name.to_string_lossy();

    // The case in device paths matters in BSD!
    #[cfg(any(
        target_os = "freebsd",
        target_os = "dragonfly",
        target_os = "openbsd",
        target_os = "netbsd"
    ))]
    let port_info = ports.iter().find(|port| port.port_name == name);

    // On Windows and other *nix systems, the case is not important.
    #[cfg(not(any(
        target_os = "freebsd",
        target_os = "dragonfly",
        target_os = "openbsd",
        target_os = "netbsd"
    )))]
    let port_info = ports
        .iter()
        .find(|port| port.port_name.eq_ignore_ascii_case(name.as_ref()));

    if let Some(port) = port_info {
        Ok(port.to_owned())
    } else {
        Err(Error::SerialNotFound(name.to_string()))
    }
}

/// Serialport's auto-detect doesn't provide any port information when using MUSL
/// Linux we can do some manual parsing of sysfs to get the relevant bits
/// without udev
#[cfg(all(target_os = "linux", target_env = "musl"))]
fn detect_usb_serial_ports(_list_all_ports: bool) -> Result<Vec<SerialPortInfo>> {
    use std::{
        fs::{read_link, read_to_string},
        path::{Path, PathBuf},
    };

    use serialport::UsbPortInfo;

    let ports = available_ports().into_diagnostic()?;
    let ports = ports
        .into_iter()
        .filter_map(|port_info| {
            // With musl, the paths we get are `/sys/class/tty/*` or `/dev/*`
            // In case of `/dev/*` we transform them into `/sys/class/tty/*`
            let path = match AsRef::<Path>::as_ref(&port_info.port_name).strip_prefix("/dev/") {
                Ok(rem) => PathBuf::from("/sys/class/tty/").join(rem),
                Err(_) => PathBuf::from(&port_info.port_name),
            };

            // This will give something like:
            // `/sys/devices/pci0000:00/0000:00:07.1/0000:0c:00.3/usb5/5-3/5-3.1/5-3.1:1.0/ttyUSB0/tty/ttyUSB0`
            let mut parent_dev = path.canonicalize().ok()?;

            // Walk up 3 dirs to get to the device hosting the tty:
            // `/sys/devices/pci0000:00/0000:00:07.1/0000:0c:00.3/usb5/5-3/5-3.1/5-3.1:1.0`
            parent_dev.pop();
            parent_dev.pop();
            parent_dev.pop();

            // Check that the device is using the usb subsystem
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

/// Returns a vector with available USB serial ports.
#[cfg(not(all(target_os = "linux", target_env = "musl")))]
fn detect_usb_serial_ports(list_all_ports: bool) -> Result<Vec<SerialPortInfo>> {
    let ports = available_ports().into_diagnostic()?;
    let ports = ports
        .into_iter()
        .filter(|port_info| {
            if list_all_ports {
                matches!(
                    &port_info.port_type,
                    SerialPortType::UsbPort(..) |
                    // Allow PciPort. The user may want to use it.
                    // The port might have been misdetected by the system as PCI.
                    SerialPortType::PciPort |
                    // Good luck.
                    SerialPortType::Unknown
                )
            } else {
                matches!(&port_info.port_type, SerialPortType::UsbPort(..))
            }
        })
        .collect::<Vec<_>>();

    Ok(ports)
}

/// USB UART adapters which are known to be on common development boards
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

/// Ask the user to select a serial port from a list of detected serial ports.
fn select_serial_port(
    mut ports: Vec<SerialPortInfo>,
    config: &Config,
    force_confirm_port: bool,
) -> Result<(SerialPortInfo, bool), Error> {
    // Does this port match a known one?
    let matches = |port: &SerialPortInfo| match &port.port_type {
        SerialPortType::UsbPort(info) => config
            .usb_device
            .iter()
            .chain(KNOWN_DEVICES.iter())
            .any(|dev| dev.matches(info)),
        _ => false,
    };

    if let [port] = ports
        .iter()
        .filter(|&p| matches(p))
        .collect::<Vec<_>>()
        .as_slice()
    {
        // There is a unique recognized device.
        if !force_confirm_port {
            return Ok(((*port).to_owned(), true));
        }
    }

    if ports.len() > 1 {
        // Multiple serial ports detected.
        info!("Detected {} serial ports", ports.len());
        info!("Ports which match a known common dev board are highlighted");
        info!("Please select a port");

        ports.sort_by_key(|a| !matches(a));

        let port_names = ports
            .iter()
            .map(|port_info| {
                let formatted = if matches(port_info) {
                    port_info.port_name.as_str().bold()
                } else {
                    port_info.port_name.as_str().reset()
                };
                match &port_info.port_type {
                    SerialPortType::UsbPort(info) => {
                        if let Some(product) = &info.product {
                            format!("{} - {}", formatted, product)
                        } else {
                            formatted.to_string()
                        }
                    }
                    _ => formatted.to_string(),
                }
            })
            .collect::<Vec<_>>();

        // https://github.com/console-rs/dialoguer/issues/77
        ctrlc::set_handler(move || {
            let term = dialoguer::console::Term::stdout();
            let _ = term.show_cursor();
        })
        .expect("Error setting Ctrl-C handler");

        let index = Select::with_theme(&ColorfulTheme::default())
            .items(&port_names)
            .default(0)
            .interact_opt()?
            .ok_or(Error::Cancelled)?;

        match ports.get(index) {
            Some(port_info) => Ok((port_info.to_owned(), matches(port_info))),
            None => Err(Error::SerialNotFound(
                port_names.get(index).unwrap().to_string(),
            )),
        }
    } else if let [port] = ports.as_slice() {
        // A single port, but we didn't recognize it. Prompt for confirmation.
        let port_name = port.port_name.clone();
        let product = match &port.port_type {
            SerialPortType::UsbPort(info) => info.product.as_ref(),
            _ => None,
        };
        if confirm_port(&port_name, product)? {
            Ok((port.to_owned(), false))
        } else {
            Err(Error::SerialNotFound(port_name))
        }
    } else {
        // No serial ports detected
        Err(Error::NoSerial)
    }
}

/// Ask the user to confirm the use of a serial port.
fn confirm_port(port_name: &str, product: Option<&String>) -> Result<bool, Error> {
    Confirm::with_theme(&ColorfulTheme::default())
        .with_prompt({
            if let Some(product) = product {
                format!("Use serial port '{}' - {}?", port_name, product)
            } else {
                format!("Use serial port '{}'?", port_name)
            }
        })
        .interact_opt()?
        .ok_or(Error::Cancelled)
}

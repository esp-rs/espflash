#[cfg(not(target_os = "windows"))]
use std::fs;
use std::{thread::sleep, time::Duration};

use crossterm::style::Stylize;
use dialoguer::{Confirm, Select, theme::ColorfulTheme};
use log::{debug, error, info};
use miette::{IntoDiagnostic, Result};
use serialport::{SerialPortInfo, SerialPortType, UsbPortInfo, available_ports};

use crate::{
    Error,
    cli::{
        ConnectArgs,
        config::{Config, PortConfig, UsbDevice},
    },
};

/// Return the information of a serial port taking into account the different
/// ways of choosing a port.
pub fn serial_port_info(matches: &ConnectArgs, config: &Config) -> Result<SerialPortInfo, Error> {
    // A serial port should be specified either as a command-line argument or in
    // a configuration file. In the case that both have been provided the
    // command-line argument takes precedence.
    //
    // Users may optionally specify the device's VID and PID in the
    // configuration file. If no VID/PID has been provided, the user will
    // always be prompted to select a serial port. If some VID and PID were
    // provided then the user will also be prompted to select a port, unless
    // there is only one found whose VID and PID match the configured
    // values.
    //
    // The call to canonicalize() was originally added to resolve
    // https://github.com/esp-rs/espflash/issues/177, however, canonicalize
    // doesn't work (on Windows) with "dummy" device paths like `COM4`. That's
    // the reason we need to handle Windows/Posix differently.

    if let Some(serial) = matches
        .port
        .as_ref()
        .or(config.port_config.connection.serial.as_ref())
    {
        let mut waiting = false;
        loop {
            let ports = detect_usb_serial_ports(true).unwrap_or_default();
            match find_serial_port(&ports, serial) {
                Err(e) if matches.wait => {
                    if !waiting {
                        info!("Waiting for serial port {serial} to appear...");
                        waiting = true;
                    }
                    debug!("Serial port {serial} is not available yet: {e}");
                    sleep(WAIT_POLL_INTERVAL);
                }
                result => return result,
            }
        }
    } else {
        // With USB devices configured, wait for one of them rather than for
        // any port, so an unrelated adapter that is already connected isn't
        // picked.
        let usb_devices = &config.port_config.usb_device;
        let ready = |ports: &[SerialPortInfo]| {
            if usb_devices.is_empty() {
                !ports.is_empty()
            } else {
                ports.iter().any(|port| match &port.port_type {
                    SerialPortType::UsbPort(info) => {
                        usb_devices.iter().any(|dev| dev.matches(info))
                    }
                    _ => false,
                })
            }
        };

        let mut ports = detect_usb_serial_ports(matches.list_all_ports).unwrap_or_default();
        if matches.wait && !ready(&ports) {
            if usb_devices.is_empty() {
                info!("Waiting for a serial port to appear...");
            } else {
                info!("Waiting for a configured USB device to appear...");
            }
            while !ready(&ports) {
                sleep(WAIT_POLL_INTERVAL);
                ports = detect_usb_serial_ports(matches.list_all_ports).unwrap_or_default();
            }
            // Adapters with several interfaces (e.g. FT2232H) enumerate them a
            // few milliseconds apart, so let the rest show up before selecting.
            sleep(WAIT_POLL_INTERVAL);
            ports = detect_usb_serial_ports(matches.list_all_ports).unwrap_or_default();
        }
        if ports.len() > 1 && matches.non_interactive {
            return Err(Error::SerialNotSelected);
        }
        let (port, matches) = select_serial_port(ports, &config.port_config, matches.confirm_port)?;
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
                        config.port_config.usb_device.push(UsbDevice {
                            vid: usb_info.vid,
                            pid: usb_info.pid,
                        })
                    }) {
                        error!("Failed to save config {e:#}");
                    }
                }
            }
            _ => {}
        }

        Ok(port)
    }
}

/// How often `--wait` checks for the serial port.
pub(super) const WAIT_POLL_INTERVAL: Duration = Duration::from_millis(500);

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

/// Returns a vector with available USB serial ports.
pub(super) fn detect_usb_serial_ports(list_all_ports: bool) -> Result<Vec<SerialPortInfo>> {
    let ports = available_ports().into_diagnostic()?;
    let ports = ports
        .into_iter()
        .filter(|port_info| {
            // On macOS, each USB serial device is exposed twice: once as a
            // "/dev/tty.*" callin device and once as a "/dev/cu.*" callout
            // device. The latter is what users typically want because opening
            // a tty.* device will block until a DCD signal is asserted.  To
            // avoid confusing users with duplicate entries we hide the tty.*
            // variants unless the caller explicitly asked to list *all* ports.
            // See https://web.archive.org/web/20120602152224/http://lists.berlios.de/pipermail/gpsd-dev/2005-April/001288.html
            #[cfg(target_os = "macos")]
            if !list_all_ports && port_info.port_name.starts_with("/dev/tty.") {
                return false;
            }

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
    UsbDevice {
        vid: 0x303a,
        pid: 0x1001,
    }, // Presented by ESP32S3FH4R2, e.g. as part of Waveshare ESP32-S3-Matrix
];

/// USB vendors whose name we display instead of the one reported by the OS.
///
/// On Windows the reported manufacturer is the vendor of the installed driver
/// package (e.g. "Microsoft" or "(Undefined Vendor)"), not of the device.
const KNOWN_VENDORS: &[(u16, &str)] = &[
    (0x0403, "FTDI"),
    (0x10c4, "Silicon Labs"),
    (0x1a86, "WCH"),
    (0x303a, "Espressif"),
];

/// Manufacturer name to display for a USB serial port.
pub(super) fn usb_manufacturer(info: &UsbPortInfo) -> Option<&str> {
    KNOWN_VENDORS
        .iter()
        .find(|(vid, _)| *vid == info.vid)
        .map(|(_, name)| *name)
        .or(info.manufacturer.as_deref())
}

/// Product name to display for a USB serial port.
///
/// On Windows the reported product is the device's friendly name, which ends
/// with the port name, e.g. "USB Serial Device (COM3)".
pub(super) fn usb_product<'a>(port_name: &str, info: &'a UsbPortInfo) -> Option<&'a str> {
    let product = info.product.as_deref()?;
    let product = product
        .strip_suffix(&format!(" ({port_name})"))
        .unwrap_or(product);
    Some(product)
}

pub(super) fn known_ports_filter(port: &SerialPortInfo, config: &PortConfig) -> bool {
    // Does this port match a known one?
    match &port.port_type {
        SerialPortType::UsbPort(info) => config
            .usb_device
            .iter()
            .chain(KNOWN_DEVICES.iter())
            .any(|dev| dev.matches(info)),
        _ => false,
    }
}

/// Ask the user to select a serial port from a list of detected serial ports.
fn select_serial_port(
    mut ports: Vec<SerialPortInfo>,
    config: &PortConfig,
    force_confirm_port: bool,
) -> Result<(SerialPortInfo, bool), Error> {
    if let [port] = ports
        .iter()
        .filter(|&p| known_ports_filter(p, config))
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

        ports.sort_by_key(|a| !known_ports_filter(a, config));

        let port_names = ports
            .iter()
            .map(|port_info| {
                let formatted = if known_ports_filter(port_info, config) {
                    port_info.port_name.as_str().bold()
                } else {
                    port_info.port_name.as_str().reset()
                };
                match &port_info.port_type {
                    SerialPortType::UsbPort(info) => {
                        if let Some(product) = usb_product(&port_info.port_name, info) {
                            format!("{formatted} - {product}")
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
            Some(port_info) => Ok((port_info.to_owned(), known_ports_filter(port_info, config))),
            None => Err(Error::SerialNotFound(
                port_names.get(index).unwrap().to_string(),
            )),
        }
    } else if let [port] = ports.as_slice() {
        // A single port, but we didn't recognize it. Prompt for confirmation.
        let port_name = port.port_name.clone();
        let product = match &port.port_type {
            SerialPortType::UsbPort(info) => usb_product(&port_name, info),
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
fn confirm_port(port_name: &str, product: Option<&str>) -> Result<bool, Error> {
    Confirm::with_theme(&ColorfulTheme::default())
        .with_prompt({
            if let Some(product) = product {
                format!("Use serial port '{port_name}' - {product}?")
            } else {
                format!("Use serial port '{port_name}'?")
            }
        })
        .interact_opt()?
        .ok_or(Error::Cancelled)
}

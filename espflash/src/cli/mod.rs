//! Types and functions for the command-line interface
//!
//! The contents of this module are intended for use with the [cargo-espflash]
//! and [espflash] command-line applications, and are likely not of much use
//! otherwise.
//!
//! Important note: The cli module DOES NOT provide SemVer guarantees,
//! feel free to opt-out by disabling the default `cli` feature.
//!
//! [cargo-espflash]: https://crates.io/crates/cargo-espflash
//! [espflash]: https://crates.io/crates/espflash

#![allow(missing_docs)]

use std::{
    collections::HashMap,
    fs::{self, File},
    io::{Read, Write},
    num::ParseIntError,
    path::{Path, PathBuf},
};

use clap::{Args, ValueEnum};
use clap_complete::Shell;
use comfy_table::{Attribute, Cell, Color, Table, modifiers, presets::UTF8_FULL};
use config::PortConfig;
use esp_idf_part::{DataType, Partition, PartitionTable};
use indicatif::{HumanCount, ProgressBar, style::ProgressStyle};
use log::{debug, info, warn};
use miette::{IntoDiagnostic, Result, WrapErr};
use serde::{Deserialize, Serialize};
use serialport::{FlowControl, SerialPortInfo, SerialPortType, UsbPortInfo};

use self::{
    config::Config,
    monitor::{LogFormat, check_monitor_args, monitor},
};
use crate::{
    connection::{
        Connection,
        reset::{ResetAfterOperation, ResetBeforeOperation},
    },
    error::{Error, MissingPartition, MissingPartitionTable},
    flasher::{
        FLASH_SECTOR_SIZE, FlashData, FlashFrequency, FlashMode, FlashSettings, FlashSize, Flasher,
    },
    image_format::{ImageFormat, ImageFormatKind, Metadata, idf::IdfBootloaderFormat},
    target::{Chip, ProgressCallbacks, XtalFrequency},
};

pub mod config;
pub mod monitor;

mod serial;

/// Establish a connection with a target device
#[derive(Debug, Args, Clone)]
#[non_exhaustive]
pub struct ConnectArgs {
    /// Reset operation to perform after connecting to the target
    #[arg(short = 'a', long, default_value = "hard-reset")]
    pub after: ResetAfterOperation,
    /// Baud rate at which to communicate with target device
    #[arg(short = 'B', long, env = "ESPFLASH_BAUD")]
    pub baud: Option<u32>,
    /// Reset operation to perform before connecting to the target
    #[arg(short = 'b', long, default_value = "default-reset")]
    pub before: ResetBeforeOperation,
    /// Target device
    #[arg(short = 'c', long)]
    pub chip: Option<Chip>,
    /// Require confirmation before auto-connecting to a recognized device.
    #[arg(long)]
    pub confirm_port: bool,
    /// List all available ports.
    #[arg(long)]
    pub list_all_ports: bool,
    /// Do not use the RAM stub for loading
    #[arg(long)]
    pub no_stub: bool,
    /// Serial port connected to target device
    #[arg(short = 'p', long, env = "ESPFLASH_PORT")]
    pub port: Option<String>,
    /// Avoids asking the user for interactions like selecting/resetting the
    /// device
    #[arg(long)]
    pub non_interactive: bool,
}

/// Generate completions for the given shell
#[derive(Debug, Args)]
#[non_exhaustive]
pub struct CompletionsArgs {
    /// Shell to generate completions for.
    pub shell: Shell,
}

/// Erase entire flash of target device
#[derive(Debug, Args)]
#[non_exhaustive]
pub struct EraseFlashArgs {
    /// Connection configuration
    #[clap(flatten)]
    pub connect_args: ConnectArgs,
}

/// Erase specified region of flash
#[derive(Debug, Args)]
#[non_exhaustive]
pub struct EraseRegionArgs {
    /// Connection configuration
    #[clap(flatten)]
    pub connect_args: ConnectArgs,
    /// Start address
    ///
    /// Must be multiple of 4096(0x1000)
    #[arg(value_parser = parse_u32)]
    pub address: u32,
    /// Size of the region to erase
    ///
    /// Must be multiple of 4096(0x1000)
    #[arg(value_parser = parse_u32)]
    pub size: u32,
}

/// Configure communication with the target device's flash
#[derive(Debug, Args)]
#[non_exhaustive]
pub struct FlashConfigArgs {
    /// Flash frequency
    #[arg(short = 'f', long, value_name = "FREQ", value_enum)]
    pub flash_freq: Option<FlashFrequency>,
    /// Flash mode to use
    #[arg(short = 'm', long, value_name = "MODE", value_enum)]
    pub flash_mode: Option<FlashMode>,
    /// Flash size of the target
    #[arg(short = 's', long, value_name = "SIZE", value_enum)]
    pub flash_size: Option<FlashSize>,
}

/// Flash an application to a target device
#[derive(Debug, Args)]
#[non_exhaustive]
#[group(skip)]
pub struct FlashArgs {
    /// Open a serial monitor after flashing
    #[arg(short = 'M', long)]
    pub monitor: bool,
    /// Monitor configuration
    #[clap(flatten)]
    pub monitor_args: MonitorConfigArgs,
    /// Load the application to RAM instead of Flash
    #[arg(long)]
    pub ram: bool,
    /// Don't verify the flash contents after flashing
    #[arg(long)]
    pub no_verify: bool,
    /// Don't skip flashing of parts with matching checksum
    #[arg(long)]
    pub no_skip: bool,
    /// Image related arguments
    #[clap(flatten)]
    pub image: ImageArgs,
    /// Erase partitions by label
    ///
    /// Only valid when using the `esp-idf` format.
    #[arg(long, value_name = "LABELS", value_delimiter = ',')]
    pub erase_parts: Option<Vec<String>>,
    /// Erase specified data partitions
    ///
    /// Only valid when using the `esp-idf` format.
    #[arg(long, value_name = "PARTS", value_enum, value_delimiter = ',')]
    pub erase_data_parts: Option<Vec<DataType>>,
}

/// Operations for ESP-IDF partition tables
#[derive(Debug, Args)]
#[non_exhaustive]
pub struct PartitionTableArgs {
    /// Optional output file name, if unset will output to stdout
    #[arg(short = 'o', long, value_name = "FILE")]
    output: Option<PathBuf>,
    /// Input partition table
    #[arg(value_name = "FILE")]
    partition_table: PathBuf,
    /// Convert CSV partition table to binary representation
    #[arg(long, conflicts_with = "to_csv")]
    to_binary: bool,
    /// Convert binary partition table to CSV representation
    #[arg(long, conflicts_with = "to_binary")]
    to_csv: bool,
}

/// Reads the content of flash memory and saves it to a file
#[derive(Debug, Args)]
#[non_exhaustive]
pub struct ReadFlashArgs {
    /// Address to start reading from
    #[arg(value_parser = parse_u32)]
    pub address: u32,
    /// Size of each individual packet of data
    ///
    /// Defaults to 0x1000 (FLASH_SECTOR_SIZE)
    #[arg(long, default_value = "0x1000", value_parser = parse_u32)]
    pub block_size: u32,
    /// Connection configuration
    #[clap(flatten)]
    connect_args: ConnectArgs,
    /// Size of the region to read
    #[arg(value_parser = parse_u32)]
    pub size: u32,
    /// File name to save the read data to
    pub file: PathBuf,
    /// Maximum number of un-acked packets
    #[arg(long, default_value = "64", value_parser = parse_u32)]
    pub max_in_flight: u32,
}

/// Save the image to disk instead of flashing to device.
#[derive(Debug, Args)]
#[non_exhaustive]
#[group(skip)]
pub struct SaveImageArgs {
    /// Chip to create an image for.
    #[arg(long, value_enum)]
    pub chip: Chip,
    /// File name to save the generated image to.
    pub file: PathBuf,
    /// Boolean flag to merge binaries into single binary.
    #[arg(long)]
    pub merge: bool,
    /// Don't pad the image to the flash size.
    #[arg(long, requires = "merge")]
    pub skip_padding: bool,
    /// Crystal frequency of the target
    #[arg(long, short = 'x')]
    pub xtal_freq: Option<XtalFrequency>,
    #[clap(flatten)]
    /// Image arguments.
    pub image: ImageArgs,
}

/// Image arguments needed for image generation.
#[derive(Debug, Args)]
#[non_exhaustive]
#[group(skip)]
pub struct ImageArgs {
    /// Minimum chip revision supported by image, in format: major.minor
    #[arg(long, default_value = "0.0", value_parser = parse_chip_rev)]
    pub min_chip_rev: u16,
    /// MMU page size.
    #[arg(long, value_name = "MMU_PAGE_SIZE", value_parser = parse_u32)]
    pub mmu_page_size: Option<u32>,
    /// Skip checking whether the app descriptor is present in the image.
    #[arg(long = "ignore_app_descriptor", default_value_t = true, action = clap::ArgAction::SetFalse)]
    pub check_app_descriptor: bool,
}

/// ESP-IDF image format arguments
#[derive(Debug, Args, Clone, Deserialize, Serialize, Default)]
#[non_exhaustive]
#[group(skip)]
pub struct IdfFormatArgs {
    /// Path to a binary ESP-IDF bootloader file
    #[arg(long, value_name = "FILE")]
    pub bootloader: Option<PathBuf>,
    /// Path to a CSV file containing partition table
    #[arg(long, value_name = "FILE")]
    pub partition_table: Option<PathBuf>,
    /// Partition table offset
    #[arg(long, value_name = "OFFSET", value_parser = parse_u32)]
    pub partition_table_offset: Option<u32>,
    /// Label of target app partition
    #[arg(long, value_name = "LABEL")]
    pub target_app_partition: Option<String>,
}

/// Arguments for connection and monitoring
#[derive(Debug, Args)]
#[non_exhaustive]
pub struct MonitorArgs {
    /// Connection configuration
    #[clap(flatten)]
    connect_args: ConnectArgs,
    /// Monitoring arguments
    #[clap(flatten)]
    monitor_args: MonitorConfigArgs,
}

/// Open the serial monitor without flashing
#[derive(Debug, Args)]
#[non_exhaustive]
pub struct MonitorConfigArgs {
    /// Baud rate at which to monitor the target device
    #[arg(short = 'r', long, env = "MONITOR_BAUD", default_value = "115_200", value_parser = parse_u32)]
    pub monitor_baud: u32,
    /// ELF image to load the symbols from
    #[arg(long, value_name = "FILE")]
    pub elf: Option<PathBuf>,
    /// Avoids restarting the device before monitoring
    ///
    /// Only valid when `non_interactive` is also set.
    #[arg(long)]
    no_reset: bool,
    /// The encoding of the target's serial output.
    #[arg(long, short = 'L')]
    log_format: Option<LogFormat>,
    /// The format of the printed defmt messages.
    ///
    /// You can also use one of two presets: oneline (default) and full.
    ///
    /// See <https://defmt.ferrous-systems.com/custom-log-output>
    #[arg(long, short = 'F')]
    output_format: Option<String>,
    /// External log processors to use (comma separated executables)
    #[arg(long)]
    processors: Option<String>,
    /// Disable address resolution for a smaller log output
    #[arg(long)]
    pub no_addresses: bool,
}

/// Arguments for MD5 checksum calculation
#[derive(Debug, Args)]
#[non_exhaustive]
pub struct ChecksumMd5Args {
    /// Start address
    #[clap(value_parser=parse_u32)]
    address: u32,
    /// Size of the region to check
    #[clap(value_parser=parse_u32)]
    size: u32,
    /// Connection configuration
    #[clap(flatten)]
    connect_args: ConnectArgs,
}

/// List the available serial ports.
#[derive(Debug, Args)]
#[non_exhaustive]
pub struct ListPortsArgs {
    /// List all available serial ports, instead of just those likely to be
    /// development boards. Includes non-usb ports such as PCI devices.
    #[arg(short = 'a', long)]
    pub list_all_ports: bool,

    /// Only print the name of the ports and nothing else. Useful for scripting.
    #[arg(short, long)]
    pub name_only: bool,
}

/// Writes a binary file to a specific address in the chip's flash
#[derive(Debug, Args)]
#[non_exhaustive]
pub struct WriteBinArgs {
    /// Address at which to write the binary file
    #[arg(value_parser = parse_u32)]
    pub address: u32,
    /// File containing the binary data to write
    pub file: String,
    /// Connection configuration
    #[clap(flatten)]
    connect_args: ConnectArgs,
    /// Open a serial monitor after writing
    #[arg(short = 'M', long)]
    pub monitor: bool,
    /// Serial monitor configuration
    #[clap(flatten)]
    pub monitor_args: MonitorConfigArgs,
}

/// Parses a bootloader file from a path
pub fn parse_bootloader(path: &Path) -> Result<Vec<u8>, Error> {
    // If the '--bootloader' option is provided, load the binary file at the
    // specified path.
    fs::canonicalize(path)
        .and_then(fs::read)
        .map_err(|e| Error::FileOpenError(path.display().to_string(), e))
}

/// Parses an integer, in base-10 or hexadecimal format, into a [u32]
pub fn parse_u32(input: &str) -> Result<u32, ParseIntError> {
    let input: &str = &input.replace('_', "");
    let (s, radix) = if input.len() > 2 && matches!(&input[0..2], "0x" | "0X") {
        (&input[2..], 16)
    } else {
        (input, 10)
    };

    u32::from_str_radix(s, radix)
}

/// Select a serial port and establish a connection with a target device
pub fn connect(
    args: &ConnectArgs,
    config: &Config,
    no_verify: bool,
    no_skip: bool,
) -> Result<Flasher> {
    if args.before == ResetBeforeOperation::NoReset
        || args.before == ResetBeforeOperation::NoResetNoSync
    {
        warn!(
            "Pre-connection option '{:#?}' was selected. Connection may fail if the chip is not in bootloader or flasher stub mode.",
            args.before
        );
    }

    let port_info = serial::serial_port_info(args, config)?;

    // Attempt to open the serial port and set its initial baud rate.
    info!("Serial port: '{}'", port_info.port_name);
    info!("Connecting...");

    let serial_port = serialport::new(&port_info.port_name, 115_200)
        .flow_control(FlowControl::None)
        .open_native()
        .map_err(Error::from)
        .wrap_err_with(|| format!("Failed to open serial port {}", port_info.port_name))?;

    // NOTE: since `serial_port_info` filters out all PCI Port and Bluetooth
    //       serial ports, we can just pretend these types don't exist here.
    let port_info = match port_info.port_type {
        SerialPortType::UsbPort(info) => info,
        SerialPortType::PciPort | SerialPortType::Unknown => {
            debug!("Matched `SerialPortType::PciPort or ::Unknown`");
            UsbPortInfo {
                vid: 0,
                pid: 0,
                serial_number: None,
                manufacturer: None,
                product: None,
            }
        }
        _ => unreachable!(),
    };

    let connection = Connection::new(
        *Box::new(serial_port),
        port_info,
        args.after,
        args.before,
        args.baud
            .or(config.project_config.baudrate)
            .unwrap_or(115_200),
    );
    Ok(Flasher::connect(
        connection,
        !args.no_stub,
        !no_verify,
        !no_skip,
        args.chip,
        args.baud.or(config.project_config.baudrate),
    )?)
}

/// Connect to a target device and print information about its chip
pub fn board_info(args: &ConnectArgs, config: &Config) -> Result<()> {
    let mut flasher = connect(args, config, true, true)?;
    print_board_info(&mut flasher)?;

    let chip = flasher.chip();
    if chip != Chip::Esp32 {
        let security_info = flasher.security_info()?;
        println!("{security_info}");
    } else {
        println!("Security features: None");
    }

    flasher.connection().reset_after(!args.no_stub, chip)?;

    Ok(())
}

/// Connect to a target device and calculate the checksum of the given region
pub fn checksum_md5(args: &ChecksumMd5Args, config: &Config) -> Result<()> {
    let mut flasher = connect(&args.connect_args, config, true, true)?;

    let checksum = flasher.checksum_md5(args.address, args.size)?;
    println!("0x{checksum:x}");

    let chip = flasher.chip();
    flasher
        .connection()
        .reset_after(!args.connect_args.no_stub, chip)?;

    Ok(())
}

/// List the available serial ports.
pub fn list_ports(args: &ListPortsArgs, config: &PortConfig) -> Result<()> {
    let mut ports: Vec<SerialPortInfo> = serial::detect_usb_serial_ports(true)?
        .into_iter()
        .filter(|p| args.list_all_ports || serial::known_ports_filter(p, config))
        .collect();
    if ports.is_empty() {
        if !args.name_only {
            println!(
                "No {}serial ports found.",
                if args.list_all_ports { "" } else { "known " }
            );
        }
    } else {
        // We want display columns so we determine a width for each field
        let name_width = ports.iter().map(|p| p.port_name.len()).max().unwrap_or(13) + 2;
        let manufacturer_width = ports
            .iter()
            .filter_map(|p| match &p.port_type {
                SerialPortType::UsbPort(p) => p.manufacturer.as_ref().map(|m| m.len()),
                _ => None,
            })
            .max()
            .unwrap_or(15)
            + 2;

        ports.sort_by(|p1, p2| {
            p1.port_name
                .to_lowercase()
                .cmp(&p2.port_name.to_lowercase())
        });
        for port in ports {
            if args.name_only {
                println!("{}", port.port_name)
            } else {
                match port.port_type {
                    SerialPortType::BluetoothPort => {
                        println!(
                            "{0: <name_width$}Bluetooth serial port",
                            port.port_name,
                            name_width = name_width + 11
                        )
                    }
                    SerialPortType::UsbPort(p) => {
                        println!(
                            "{0: <name_width$}{3:04X}:{4:04X}  {1: <manufacturer_width$}{2}",
                            port.port_name,
                            p.manufacturer.unwrap_or_default(),
                            p.product.unwrap_or_default(),
                            p.pid,
                            p.vid,
                            name_width = name_width,
                            manufacturer_width = manufacturer_width,
                        )
                    }
                    SerialPortType::PciPort => {
                        println!(
                            "{0: <name_width$}PCI serial port",
                            port.port_name,
                            name_width = name_width + 11
                        )
                    }
                    SerialPortType::Unknown => {
                        println!(
                            "{0: <name_width$}Unknown type of port",
                            port.port_name,
                            name_width = name_width + 11
                        )
                    }
                }
            }
        }
    }
    Ok(())
}

/// Generate shell completions for the given shell
pub fn completions(args: &CompletionsArgs, app: &mut clap::Command, bin_name: &str) -> Result<()> {
    clap_complete::generate(args.shell, app, bin_name, &mut std::io::stdout());

    Ok(())
}

/// Parses chip revision from string to major * 100 + minor format
pub fn parse_chip_rev(chip_rev: &str) -> Result<u16> {
    let mut split = chip_rev.split('.');

    let parse_or_error = |value: Option<&str>| {
        value
            .ok_or_else(|| Error::ParseChipRevError {
                chip_rev: chip_rev.to_string(),
            })
            .and_then(|v| {
                v.parse::<u16>().map_err(|_| Error::ParseChipRevError {
                    chip_rev: chip_rev.to_string(),
                })
            })
            .into_diagnostic()
    };

    let major = parse_or_error(split.next())?;
    let minor = parse_or_error(split.next())?;

    if split.next().is_some() {
        return Err(Error::ParseChipRevError {
            chip_rev: chip_rev.to_string(),
        })
        .into_diagnostic();
    }

    Ok(major * 100 + minor)
}

/// Print information about a chip
pub fn print_board_info(flasher: &mut Flasher) -> Result<()> {
    let info = flasher.device_info()?;
    print!("Chip type:         {}", info.chip);

    if let Some((major, minor)) = info.revision {
        println!(" (revision v{major}.{minor})");
    } else {
        println!();
    }

    println!("Crystal frequency: {}", info.crystal_frequency);
    println!("Flash size:        {}", info.flash_size);
    println!("Features:          {}", info.features.join(", "));

    if let Some(mac) = info.mac_address {
        println!("MAC address:       {mac}");
    }

    Ok(())
}

/// Open a serial monitor
pub fn serial_monitor(args: MonitorArgs, config: &Config) -> Result<()> {
    let mut flasher = connect(&args.connect_args, config, true, true)?;
    let pid = flasher.connection().usb_pid();

    let elf = if let Some(elf_path) = args.monitor_args.elf.clone() {
        let path = fs::canonicalize(elf_path).into_diagnostic()?;
        let data = fs::read(path).into_diagnostic()?;

        Some(data)
    } else {
        None
    };

    let chip = flasher.chip();

    ensure_chip_compatibility(chip, elf.as_deref())?;

    let mut monitor_args = args.monitor_args;

    // The 26MHz ESP32-C2's need to be treated as a special case.
    if chip == Chip::Esp32c2
        && chip.xtal_frequency(flasher.connection())? == XtalFrequency::_26Mhz
        && monitor_args.monitor_baud == 115_200
    {
        // 115_200 * 26 MHz / 40 MHz = 74_880
        monitor_args.monitor_baud = 74_880;
    }

    monitor(
        flasher.into(),
        elf.as_deref(),
        pid,
        monitor_args,
        args.connect_args.non_interactive,
    )
}

/// Convert the provided firmware image from ELF to binary
pub fn save_elf_as_image<'a>(
    image_path: PathBuf,
    flash_size: Option<FlashSize>,
    merge: bool,
    skip_padding: bool,
    image_format: ImageFormat<'a>,
) -> Result<()> {
    let metadata = image_format.metadata();
    if metadata.contains_key("app_size") && metadata.contains_key("part_size") {
        let app_size = metadata["app_size"].parse::<u32>().unwrap();
        let part_size = metadata["part_size"].parse::<u32>().unwrap();

        display_image_size(app_size, Some(part_size));
    }

    if merge {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .truncate(true)
            .create(true)
            .open(image_path)
            .into_diagnostic()?;

        for segment in image_format.flash_segments() {
            let padding_bytes = vec![
                0xffu8;
                segment.addr as usize
                    - file.metadata().into_diagnostic()?.len() as usize
            ];
            file.write_all(&padding_bytes).into_diagnostic()?;
            file.write_all(&segment.data).into_diagnostic()?;
        }

        if !skip_padding {
            // Take flash_size as input parameter, if None, use default value of 4Mb
            let padding_bytes = vec![
                0xffu8;
                flash_size.unwrap_or_default().size() as usize
                    - file.metadata().into_diagnostic()?.len() as usize
            ];
            file.write_all(&padding_bytes).into_diagnostic()?;
        }
    } else {
        match image_format.ota_segments().as_slice() {
            [single] => fs::write(&image_path, &single.data).into_diagnostic()?,
            parts => {
                for part in parts {
                    let part_path = format!("{:#x}_{}", part.addr, image_path.display());
                    fs::write(part_path, &part.data).into_diagnostic()?
                }
            }
        }
    }

    info!("Image successfully saved!");

    Ok(())
}

/// Displays the image or app size
pub(crate) fn display_image_size(app_size: u32, part_size: Option<u32>) {
    if let Some(part_size) = part_size {
        let percent = app_size as f32 / part_size as f32 * 100.0;
        println!(
            "App/part. size:    {}/{} bytes, {:.2}%",
            HumanCount(app_size as u64),
            HumanCount(part_size as u64),
            percent
        );
    } else {
        println!("App size:          {} bytes", HumanCount(app_size as u64));
    }
}

/// Progress callback implementations for use in `cargo-espflash` and `espflash`
#[derive(Debug, Default)]
pub struct EspflashProgress {
    pb: Option<ProgressBar>,
    verifying: bool,
}

impl ProgressCallbacks for EspflashProgress {
    /// Initialize the progress bar
    fn init(&mut self, addr: u32, len: usize) {
        let pb = ProgressBar::new(len as u64)
            .with_message(format!("{addr:<#8X}"))
            .with_style(
                ProgressStyle::default_bar()
                    .template("[{elapsed_precise}] [{bar:40}] {pos:>7}/{len:7} {msg}")
                    .unwrap()
                    .progress_chars("=> "),
            );

        self.pb = Some(pb);
        self.verifying = false;
    }

    /// Update the progress bar
    fn update(&mut self, current: usize) {
        if let Some(pb) = &self.pb {
            pb.set_position(current as u64);
        }
    }

    /// Tell user we're verifying the flashed data
    fn verifying(&mut self) {
        if let Some(pb) = &self.pb {
            self.verifying = true;
            let last_msg = pb.message();

            pb.set_message(format!("{last_msg} Verifying..."));
        }
    }

    /// End the progress bar
    fn finish(&mut self, skipped: bool) {
        if let Some(pb) = &self.pb {
            use crossterm::style::Stylize;
            let last_msg = pb.message();

            if skipped {
                let skipped = "Skipped! (checksum matches)".cyan();
                pb.finish_with_message(format!("{last_msg} {skipped}"));
            } else if self.verifying {
                let ok = "OK!".green();
                pb.finish_with_message(format!("{last_msg} {ok}"));
            } else {
                pb.finish();
            }
        }
        self.verifying = false;
    }
}

/// Erase the entire flash memory of a target device
pub fn erase_flash(args: EraseFlashArgs, config: &Config) -> Result<()> {
    if args.connect_args.no_stub {
        return Err(Error::StubRequired.into());
    }

    let mut flasher = connect(&args.connect_args, config, true, true)?;
    info!("Erasing Flash...");

    let chip = flasher.chip();

    flasher.erase_flash()?;
    flasher
        .connection()
        .reset_after(!args.connect_args.no_stub, chip)?;

    info!("Flash has been erased!");

    Ok(())
}

/// Erase a specified region of flash memory
pub fn erase_region(args: EraseRegionArgs, config: &Config) -> Result<()> {
    if args.connect_args.no_stub {
        return Err(Error::StubRequired).into_diagnostic();
    }

    if args.address % FLASH_SECTOR_SIZE as u32 != 0 || args.size % FLASH_SECTOR_SIZE as u32 != 0 {
        return Err(Error::InvalidEraseRegionArgument {
            address: args.address,
            size: args.size,
        })
        .into_diagnostic();
    }

    let mut flasher = connect(&args.connect_args, config, true, true)?;
    let chip = flasher.chip();

    info!(
        "Erasing region at 0x{:08x} ({} bytes)",
        args.address, args.size
    );

    flasher.erase_region(args.address, args.size)?;
    flasher
        .connection()
        .reset_after(!args.connect_args.no_stub, chip)?;

    Ok(())
}

/// Write an ELF image to a target device's flash
pub fn flash_image<'a>(flasher: &mut Flasher, image_format: ImageFormat<'a>) -> Result<()> {
    flasher.load_image_to_flash(&mut EspflashProgress::default(), image_format)?;
    info!("Flashing has completed!");

    Ok(())
}

/// Erase one or more partitions by label or [DataType]
pub fn erase_partitions(
    flasher: &mut Flasher,
    partition_table: Option<PartitionTable>,
    erase_parts: Option<Vec<String>>,
    erase_data_parts: Option<Vec<DataType>>,
) -> Result<()> {
    let partition_table = match &partition_table {
        Some(partition_table) => partition_table,
        None => return Err(MissingPartitionTable.into()),
    };

    // Using a hashmap to deduplicate entries
    let mut parts_to_erase = None;

    // Look for any partitions with specific labels
    if let Some(part_labels) = erase_parts {
        for label in part_labels {
            let part = partition_table
                .find(label.as_str())
                .ok_or_else(|| MissingPartition::from(label))?;

            parts_to_erase
                .get_or_insert(HashMap::new())
                .insert(part.offset(), part);
        }
    }

    // Look for any data partitions with specific data subtype
    // There might be multiple partition of the same subtype, e.g. when using
    // multiple FAT partitions
    if let Some(partition_types) = erase_data_parts {
        for ty in partition_types {
            for part in partition_table.partitions() {
                if part.ty() == esp_idf_part::Type::Data
                    && part.subtype() == esp_idf_part::SubType::Data(ty)
                {
                    parts_to_erase
                        .get_or_insert(HashMap::new())
                        .insert(part.offset(), part);
                }
            }
        }
    }

    if let Some(parts) = parts_to_erase {
        parts
            .iter()
            .try_for_each(|(_, p)| erase_partition(flasher, p))?;
    }

    Ok(())
}

/// Erase a single partition
fn erase_partition(flasher: &mut Flasher, part: &Partition) -> Result<()> {
    log::info!("Erasing {} ({:?})...", part.name(), part.subtype());

    let offset = part.offset();
    let size = part.size();

    flasher.erase_region(offset, size).into_diagnostic()
}

/// Read flash content and write it to a file
pub fn read_flash(args: ReadFlashArgs, config: &Config) -> Result<()> {
    let mut flasher = connect(&args.connect_args, config, false, false)?;
    print_board_info(&mut flasher)?;

    if args.connect_args.no_stub {
        flasher.read_flash_rom(
            args.address,
            args.size,
            args.block_size,
            args.max_in_flight,
            args.file,
        )?;
    } else {
        flasher.read_flash(
            args.address,
            args.size,
            args.block_size,
            args.max_in_flight,
            args.file,
        )?;
    }

    let chip = flasher.chip();
    flasher
        .connection()
        .reset_after(!args.connect_args.no_stub, chip)?;

    Ok(())
}

/// Convert and display CSV and binary partition tables
pub fn partition_table(args: PartitionTableArgs) -> Result<()> {
    if args.to_binary {
        let table = parse_partition_table(&args.partition_table)?;

        // Use either stdout or a file if provided for the output.
        let mut writer: Box<dyn Write> = if let Some(output) = args.output {
            Box::new(fs::File::create(output).into_diagnostic()?)
        } else {
            Box::new(std::io::stdout())
        };

        writer
            .write_all(&table.to_bin().into_diagnostic()?)
            .into_diagnostic()?;
    } else if args.to_csv {
        let input = fs::read(&args.partition_table).into_diagnostic()?;
        let table = PartitionTable::try_from_bytes(input).into_diagnostic()?;

        // Use either stdout or a file if provided for the output.
        let mut writer: Box<dyn Write> = if let Some(output) = args.output {
            Box::new(fs::File::create(output).into_diagnostic()?)
        } else {
            Box::new(std::io::stdout())
        };

        writer
            .write_all(table.to_csv().into_diagnostic()?.as_bytes())
            .into_diagnostic()?;
    } else {
        let input = fs::read(&args.partition_table).into_diagnostic()?;
        let table = PartitionTable::try_from(input).into_diagnostic()?;

        pretty_print(table);
    }

    Ok(())
}

/// Parse a [PartitionTable] from the provided path
pub fn parse_partition_table(path: &Path) -> Result<PartitionTable, Error> {
    let data = fs::read(path).map_err(|e| Error::FileOpenError(path.display().to_string(), e))?;

    Ok(PartitionTable::try_from(data)?)
}

/// Pretty print a partition table
fn pretty_print(table: PartitionTable) {
    let mut pretty = Table::new();

    pretty
        .load_preset(UTF8_FULL)
        .apply_modifier(modifiers::UTF8_ROUND_CORNERS)
        .set_header(vec![
            Cell::new("Name")
                .fg(Color::Green)
                .add_attribute(Attribute::Bold),
            Cell::new("Type")
                .fg(Color::Cyan)
                .add_attribute(Attribute::Bold),
            Cell::new("SubType")
                .fg(Color::Magenta)
                .add_attribute(Attribute::Bold),
            Cell::new("Offset")
                .fg(Color::Red)
                .add_attribute(Attribute::Bold),
            Cell::new("Size")
                .fg(Color::Yellow)
                .add_attribute(Attribute::Bold),
            Cell::new("Encrypted")
                .fg(Color::DarkCyan)
                .add_attribute(Attribute::Bold),
        ]);

    for p in table.partitions() {
        let flags = p
            .flags()
            .iter_names()
            .map(|(name, _)| name.to_lowercase())
            .collect::<Vec<_>>()
            .join(":");

        pretty.add_row(vec![
            Cell::new(p.name()).fg(Color::Green),
            Cell::new(p.ty().to_string()).fg(Color::Cyan),
            Cell::new(p.subtype().to_string()).fg(Color::Magenta),
            Cell::new(format!("{:#x}", p.offset())).fg(Color::Red),
            Cell::new(format!("{:#x} ({}KiB)", p.size(), p.size() / 1024)).fg(Color::Yellow),
            Cell::new(flags).fg(Color::DarkCyan),
        ]);
    }

    println!("{pretty}");
}

/// Make an image format from the given arguments
pub fn make_image_format<'a>(
    elf_data: &'a [u8],
    flash_data: &FlashData,
    image_format_kind: ImageFormatKind,
    config: &Config,
    idf_format_args: Option<IdfFormatArgs>,
    build_ctx_bootloader: Option<PathBuf>,
    build_ctx_partition_table: Option<PathBuf>,
) -> Result<ImageFormat<'a>, Error> {
    let image_format = match image_format_kind {
        ImageFormatKind::EspIdf => {
            let mut args = idf_format_args.unwrap_or_default();
            // Set bootloader path with precedence
            if args.bootloader.is_none() {
                args.bootloader = config
                    .project_config
                    .idf_format_args
                    .bootloader
                    .clone()
                    .or(build_ctx_bootloader);
            }

            // Set partition table path with precedence
            if args.partition_table.is_none() {
                args.partition_table = config
                    .project_config
                    .idf_format_args
                    .partition_table
                    .clone()
                    .or(build_ctx_partition_table);
            }
            IdfBootloaderFormat::new(
                elf_data,
                flash_data,
                args.partition_table.as_deref(),
                args.bootloader.as_deref(),
                args.partition_table_offset,
                args.target_app_partition.as_deref(),
            )?
        }
    };

    Ok(image_format.into())
}

/// Make flash data from the given arguments
pub fn make_flash_data(
    image_args: ImageArgs,
    flash_config_args: &FlashConfigArgs,
    config: &Config,
    chip: Chip,
    xtal_freq: XtalFrequency,
) -> FlashData {
    // Create flash settings with precedence
    let mode = flash_config_args
        .flash_mode
        .or(config.project_config.flash.mode);
    let size = flash_config_args
        .flash_size
        .or(config.project_config.flash.size)
        .or_else(|| Some(FlashSize::default()));
    let freq = flash_config_args
        .flash_freq
        .or(config.project_config.flash.freq);

    let flash_settings = FlashSettings::new(mode, size, freq);

    FlashData::new(
        flash_settings,
        image_args.min_chip_rev,
        image_args.mmu_page_size,
        chip,
        xtal_freq,
    )
}

/// Write a binary to the flash memory of a target device
pub fn write_bin(args: WriteBinArgs, config: &Config) -> Result<()> {
    // Check monitor arguments
    check_monitor_args(
        &args.monitor,
        &args.monitor_args,
        args.connect_args.non_interactive,
    )?;

    // Load the file to be flashed
    let mut f = File::open(&args.file).into_diagnostic()?;

    let size = f.metadata().into_diagnostic()?.len();
    let mut buffer = Vec::with_capacity(size.try_into().into_diagnostic()?);
    f.read_to_end(&mut buffer).into_diagnostic()?;

    let mut flasher = connect(&args.connect_args, config, false, false)?;
    print_board_info(&mut flasher)?;

    let chip = flasher.chip();
    let target_xtal_freq = chip.xtal_frequency(flasher.connection())?;

    flasher.write_bin_to_flash(args.address, &buffer, &mut EspflashProgress::default())?;

    if args.monitor {
        let pid = flasher.connection().usb_pid();
        let mut monitor_args = args.monitor_args;
        if chip == Chip::Esp32c2
            && target_xtal_freq == XtalFrequency::_26Mhz
            && monitor_args.monitor_baud == 115_200
        {
            monitor_args.monitor_baud = 74_880;
        }
        monitor(
            flasher.into(),
            None,
            pid,
            monitor_args,
            args.connect_args.non_interactive,
        )?;
    }

    Ok(())
}

/// Reset the target device.
pub fn reset(args: ConnectArgs, config: &Config) -> Result<()> {
    let mut args = args.clone();
    args.no_stub = true;
    let mut flasher = connect(&args, config, true, true)?;
    info!("Resetting target device");
    flasher.connection().reset()?;

    Ok(())
}

/// Hold the target device in reset.
pub fn hold_in_reset(args: ConnectArgs, config: &Config) -> Result<()> {
    connect(&args, config, true, true)?;
    info!("Holding target device in reset");

    Ok(())
}

/// Ensures the chip is compatible with the ELF file.
pub fn ensure_chip_compatibility(chip: Chip, elf: Option<&[u8]>) -> Result<()> {
    let metadata = Metadata::from_bytes(elf);
    let Some(elf_chip) = metadata.chip_name() else {
        // No chip name in the ELF, assume compatible
        return Ok(());
    };

    match Chip::from_str(elf_chip, false) {
        Ok(elf_chip) if chip == elf_chip => Ok(()),
        _ => Err(Error::FirmwareChipMismatch {
            elf: elf_chip.to_string(),
            detected: chip,
        })
        .into_diagnostic(),
    }
}

/// Check if the given arguments are valid for the ESP-IDF format
pub fn check_idf_args(
    format: ImageFormatKind,
    erase_parts: &Option<Vec<String>>,
    erase_data_parts: &Option<Vec<DataType>>,
) -> Result<()> {
    if format != ImageFormatKind::EspIdf && (erase_parts.is_some() || erase_data_parts.is_some()) {
        return Err(miette::miette!(
            "`erase-parts` and `erase-data` parts are only supported when using the `esp-idf` format."
        ));
    }

    Ok(())
}

mod test {
    use clap::Parser;

    use super::*;

    #[derive(Parser)]
    struct TestParser {
        #[clap(flatten)]
        args: IdfFormatArgs,
    }

    #[test]
    fn test_parse_hex_partition_table_offset() {
        let command = "command --partition-table-offset 0x8000";
        let iter = command.split_whitespace();
        let parser = TestParser::parse_from(iter);
        assert_eq!(parser.args.partition_table_offset, Some(0x8000));
    }

    #[test]
    fn test_parse_u32() {
        // Hex
        assert_eq!(parse_u32("0x1"), Ok(0x1));
        assert_eq!(parse_u32("0X1234"), Ok(0x1234));
        assert_eq!(parse_u32("0xaBcD"), Ok(0xabcd));
        // Decimal
        assert_eq!(parse_u32("1234"), Ok(1234));
        assert_eq!(parse_u32("0"), Ok(0));
        // Underscores
        assert_eq!(parse_u32("12_34"), Ok(1234));
        assert_eq!(parse_u32("0X12_34"), Ok(0x1234));
        // Errors
        assert!(parse_u32("").is_err());
        assert!(parse_u32("0x").is_err());
        assert!(parse_u32("0xg").is_err());
        assert!(parse_u32("-123").is_err());
        assert!(parse_u32("12.34").is_err());
    }
}

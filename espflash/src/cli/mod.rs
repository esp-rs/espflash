//! Types and functions for the command-line interface
//!
//! The contents of this module are intended for use with the [cargo-espflash]
//! and [espflash] command-line applications, and are likely not of much use
//! otherwise.
//!
//! [cargo-espflash]: https://crates.io/crates/cargo-espflash
//! [espflash]: https://crates.io/crates/espflash

use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};

use clap::Args;
use comfy_table::{modifiers, presets::UTF8_FULL, Attribute, Cell, Color, Table};
use esp_idf_part::{DataType, PartitionTable, SubType, Type};
use log::{debug, info};
use miette::{IntoDiagnostic, Result, WrapErr};
use serialport::{SerialPortType, UsbPortInfo};
use strum::VariantNames;

use self::{config::Config, monitor::monitor, serial::get_serial_port_info};
use crate::{
    elf::ElfFirmwareImage,
    error::{MissingPartitionTable, NoOtadataError},
    flasher::{FlashFrequency, FlashMode, FlashSize, Flasher},
    image_format::ImageFormatKind,
    interface::Interface,
    targets::Chip,
};

pub mod config;
pub mod monitor;

mod serial;

// Since as of `clap@4.0.x` the `possible_values` attribute is no longer
// present, we must use the more convoluted `value_parser` attribute instead.
// Since this is a bit tedious, we'll use a helper macro to abstract away all
// the cruft. It's important to note that this macro assumes the
// `strum::EnumVariantNames` trait has been implemented for the provided type,
// and that the provided type is in scope when calling this macro.
//
// See this comment for details:
// https://github.com/clap-rs/clap/discussions/4264#discussioncomment-3737696
#[doc(hidden)]
#[macro_export]
macro_rules! clap_enum_variants {
    ($e: ty) => {{
        use clap::builder::TypedValueParser;
        clap::builder::PossibleValuesParser::new(<$e>::VARIANTS).map(|s| s.parse::<$e>().unwrap())
    }};
}

pub use clap_enum_variants;

/// Establish a connection with a target device
#[derive(Debug, Args)]
pub struct ConnectArgs {
    /// Baud rate at which to communicate with target device
    #[arg(short = 'b', long)]
    pub baud: Option<u32>,
    /// Serial port connected to target device
    #[arg(short = 'p', long)]
    pub port: Option<String>,
    /// DTR pin to use for the internal UART hardware. Uses BCM numbering.
    #[cfg(feature = "raspberry")]
    #[cfg_attr(feature = "raspberry", clap(long))]
    pub dtr: Option<u8>,
    /// RTS pin to use for the internal UART hardware. Uses BCM numbering.
    #[cfg(feature = "raspberry")]
    #[cfg_attr(feature = "raspberry", clap(long))]
    pub rts: Option<u8>,
    /// Use RAM stub for loading
    #[arg(long)]
    pub use_stub: bool,
}

/// Configure communication with the target device's flash
#[derive(Debug, Args)]
pub struct FlashConfigArgs {
    /// Flash frequency
    #[arg(short = 'f', long, value_name = "FREQ", value_parser = clap_enum_variants!(FlashFrequency))]
    pub flash_freq: Option<FlashFrequency>,
    /// Flash mode to use
    #[arg(short = 'm', long, value_name = "MODE", value_parser = clap_enum_variants!(FlashMode))]
    pub flash_mode: Option<FlashMode>,
    /// Flash size of the target
    #[arg(short = 's', long, value_name = "SIZE", value_parser = clap_enum_variants!(FlashSize))]
    pub flash_size: Option<FlashSize>,
}

/// Flash an application to a target device
#[derive(Debug, Args)]
#[group(skip)]
pub struct FlashArgs {
    /// Path to a binary (.bin) bootloader file
    #[arg(long, value_name = "FILE")]
    pub bootloader: Option<PathBuf>,
    /// Erase the OTA data partition
    /// This is useful when using multiple OTA partitions and still wanting to
    /// be able to reflash via cargo-espflash or espflash
    #[arg(long)]
    pub erase_otadata: bool,
    /// Image format to flash
    #[arg(long, value_parser = clap_enum_variants!(ImageFormatKind))]
    pub format: Option<ImageFormatKind>,
    /// Open a serial monitor after flashing
    #[arg(long)]
    pub monitor: bool,
    /// Baud rate at which to read console output
    #[arg(long, requires = "monitor", value_name = "BAUD")]
    pub monitor_baud: Option<u32>,
    /// Path to a CSV file containing partition table
    #[arg(long, value_name = "FILE")]
    pub partition_table: Option<PathBuf>,
    /// Load the application to RAM instead of Flash
    #[arg(long)]
    pub ram: bool,
}

/// Operations for partitions tables
#[derive(Debug, Args)]
pub struct PartitionTableArgs {
    /// Optional output file name, if unset will output to stdout
    #[arg(short = 'o', long, value_name = "FILE")]
    output: Option<PathBuf>,
    /// Input partition table
    #[arg(value_name = "FILE")]
    partition_table: PathBuf,
    /// Convert CSV parition table to binary representation
    #[arg(long, conflicts_with = "to_csv")]
    to_binary: bool,
    /// Convert binary partition table to CSV representation
    #[arg(long, conflicts_with = "to_binary")]
    to_csv: bool,
}

/// Save the image to disk instead of flashing to device
#[derive(Debug, Args)]
#[group(skip)]
pub struct SaveImageArgs {
    /// Custom bootloader for merging
    #[arg(long, value_name = "FILE")]
    pub bootloader: Option<PathBuf>,
    /// Chip to create an image for
    #[arg(long, value_parser = clap_enum_variants!(Chip))]
    pub chip: Chip,
    /// File name to save the generated image to
    pub file: PathBuf,
    /// Boolean flag to merge binaries into single binary
    #[arg(long)]
    pub merge: bool,
    /// Custom partition table for merging
    #[arg(long, short = 'T', requires = "merge", value_name = "FILE")]
    pub partition_table: Option<PathBuf>,
    /// Don't pad the image to the flash size
    #[arg(long, short = 'P', requires = "merge")]
    pub skip_padding: bool,
}

/// Select a serial port and establish a connection with a target device
pub fn connect(args: &ConnectArgs, config: &Config) -> Result<Flasher> {
    let port_info = get_serial_port_info(args, config)?;

    // Attempt to open the serial port and set its initial baud rate.
    info!("Serial port: '{}'", port_info.port_name);
    info!("Connecting...");

    let interface = Interface::new(&port_info, args, config)
        .wrap_err_with(|| format!("Failed to open serial port {}", port_info.port_name))?;

    // NOTE: since `get_serial_port_info` filters out all PCI Port and Bluetooth
    //       serial ports, we can just pretend these types don't exist here.
    let port_info = match port_info.port_type {
        SerialPortType::UsbPort(info) => info,
        SerialPortType::Unknown => {
            debug!("Matched `SerialPortType::Unknown`");
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

    Ok(Flasher::connect(
        interface,
        port_info,
        args.baud,
        args.use_stub,
    )?)
}

/// Connect to a target device and print information about its chip
pub fn board_info(args: ConnectArgs, config: &Config) -> Result<()> {
    let mut flasher = connect(&args, config)?;
    flasher.board_info()?;

    Ok(())
}

/// Open a serial monitor
pub fn serial_monitor(args: ConnectArgs, config: &Config) -> Result<()> {
    let flasher = connect(&args, config)?;
    let pid = flasher.get_usb_pid()?;

    monitor(
        flasher.into_interface(),
        None,
        pid,
        args.baud.unwrap_or(115_200),
    )
    .into_diagnostic()?;

    Ok(())
}

/// Convert the provided firmware image from ELF to binary
pub fn save_elf_as_image(
    chip: Chip,
    elf_data: &[u8],
    image_path: PathBuf,
    image_format: Option<ImageFormatKind>,
    flash_mode: Option<FlashMode>,
    flash_size: Option<FlashSize>,
    flash_freq: Option<FlashFrequency>,
    merge: bool,
    bootloader_path: Option<PathBuf>,
    partition_table_path: Option<PathBuf>,
    skip_padding: bool,
) -> Result<()> {
    let image = ElfFirmwareImage::try_from(elf_data)?;

    if merge {
        // merge_bin is TRUE
        // merge bootloader, partition table and app binaries
        // basic functionality, only merge 3 binaries

        // If the '-B' option is provided, load the bootloader binary file at the
        // specified path.
        let bootloader = if let Some(bootloader_path) = bootloader_path {
            let path = fs::canonicalize(bootloader_path).into_diagnostic()?;
            let data = fs::read(path).into_diagnostic()?;

            Some(data)
        } else {
            None
        };

        // If the '-T' option is provided, load the partition table from
        // the CSV or binary file at the specified path.
        let partition_table = if let Some(partition_table_path) = partition_table_path {
            let path = fs::canonicalize(partition_table_path).into_diagnostic()?;
            let data = fs::read(path)
                .into_diagnostic()
                .wrap_err("Failed to open partition table")?;

            let table = PartitionTable::try_from(data).into_diagnostic()?;

            Some(table)
        } else {
            None
        };

        // To get a chip revision, the connection is needed
        // For simplicity, the revision None is used
        let image = chip.into_target().get_flash_image(
            &image,
            bootloader,
            partition_table,
            image_format,
            None,
            flash_mode,
            flash_size,
            flash_freq,
        )?;

        let mut file = fs::OpenOptions::new()
            .write(true)
            .truncate(true)
            .create(true)
            .open(image_path)
            .into_diagnostic()?;

        for segment in image.flash_segments() {
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
                flash_size.unwrap_or(FlashSize::Flash4Mb).size() as usize
                    - file.metadata().into_diagnostic()?.len() as usize
            ];
            file.write_all(&padding_bytes).into_diagnostic()?;
        }
    } else {
        let flash_image = chip.into_target().get_flash_image(
            &image,
            None,
            None,
            image_format,
            None,
            flash_mode,
            flash_size,
            flash_freq,
        )?;
        let parts: Vec<_> = flash_image.ota_segments().collect();

        match parts.as_slice() {
            [single] => fs::write(&image_path, &single.data).into_diagnostic()?,
            parts => {
                for part in parts {
                    let part_path = format!("{:#x}_{}", part.addr, image_path.display());
                    fs::write(part_path, &part.data).into_diagnostic()?
                }
            }
        }
    }

    Ok(())
}

/// Write an ELF image to a target device's flash
pub fn flash_elf_image(
    flasher: &mut Flasher,
    elf_data: &[u8],
    bootloader: Option<&Path>,
    partition_table: Option<&Path>,
    image_format: Option<ImageFormatKind>,
    flash_mode: Option<FlashMode>,
    flash_size: Option<FlashSize>,
    flash_freq: Option<FlashFrequency>,
    erase_otadata: bool,
) -> Result<()> {
    // If the '--bootloader' option is provided, load the binary file at the
    // specified path.
    let bootloader = if let Some(path) = bootloader {
        let path = fs::canonicalize(path).into_diagnostic()?;
        let data = fs::read(path).into_diagnostic()?;

        Some(data)
    } else {
        None
    };

    // If the '--partition-table' option is provided, load the partition table from
    // the CSV or binary file at the specified path.
    let partition_table = if let Some(path) = partition_table {
        let path = fs::canonicalize(path).into_diagnostic()?;

        let data = fs::read(path)
            .into_diagnostic()
            .wrap_err("Failed to open partition table")?;
        let table = PartitionTable::try_from(data).into_diagnostic()?;

        Some(table)
    } else {
        None
    };

    if erase_otadata {
        let partition_table = match &partition_table {
            Some(partition_table) => partition_table,
            None => return Err((MissingPartitionTable {}).into()),
        };

        let otadata =
            match partition_table.find_by_subtype(Type::Data, SubType::Data(DataType::Ota)) {
                Some(otadata) => otadata,
                None => return Err((NoOtadataError {}).into()),
            };

        let offset = otadata.offset();
        let size = otadata.size();

        flasher.erase_region(offset, size)?;
    }

    // Load the ELF data, optionally using the provider bootloader/partition
    // table/image format, to the device's flash memory.
    flasher.load_elf_to_flash_with_format(
        elf_data,
        bootloader,
        partition_table,
        image_format,
        flash_mode,
        flash_size,
        flash_freq,
    )?;
    info!("Flashing has completed!");

    Ok(())
}

/// Convert and display CSV and binary partition tables
pub fn partition_table(args: PartitionTableArgs) -> Result<()> {
    if args.to_binary {
        let input = fs::read_to_string(&args.partition_table).into_diagnostic()?;
        let table = PartitionTable::try_from_str(input).into_diagnostic()?;

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
        pretty.add_row(vec![
            Cell::new(&p.name()).fg(Color::Green),
            Cell::new(&p.ty().to_string()).fg(Color::Cyan),
            Cell::new(&p.subtype().to_string()).fg(Color::Magenta),
            Cell::new(&format!("{:#x}", p.offset())).fg(Color::Red),
            Cell::new(&format!("{:#x} ({}KiB)", p.size(), p.size() / 1024)).fg(Color::Yellow),
            Cell::new(&p.encrypted()).fg(Color::DarkCyan),
        ]);
    }

    println!("{pretty}");
}

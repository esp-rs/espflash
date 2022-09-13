//! CLI utilities shared between espflash and cargo-espflash
//!
//! No stability guaranties apply

use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};

use clap::Args;
use config::Config;
use miette::{IntoDiagnostic, Result, WrapErr};
use serialport::{SerialPortType, UsbPortInfo};
use strum::VariantNames;

use crate::{
    cli::{monitor::monitor, serial::get_serial_port_info},
    elf::ElfFirmwareImage,
    error::NoOtadataError,
    flasher::{FlashFrequency, FlashMode, FlashSize},
    image_format::ImageFormatType,
    interface::Interface,
    partition_table, Chip, Flasher, ImageFormatId, InvalidPartitionTable, MissingPartitionTable,
    PartitionTable,
};

pub mod config;
pub mod monitor;

mod serial;

#[derive(Debug, Args)]
pub struct ConnectArgs {
    /// Baud rate at which to communicate with target device
    #[clap(short = 'b', long)]
    pub baud: Option<u32>,
    /// Baud rate at which to read console output
    #[clap(long)]
    pub monitor_baud: Option<u32>,
    /// Serial port connected to target device
    #[clap(short = 'p', long)]
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
    #[clap(long)]
    pub use_stub: bool,
}

#[derive(Debug, Args)]
pub struct FlashConfigArgs {
    /// Flash frequency
    #[clap(short = 'f', long, possible_values = FlashFrequency::VARIANTS, value_name = "FREQ")]
    pub flash_freq: Option<FlashFrequency>,
    /// Flash mode to use
    #[clap(short = 'm', long, possible_values = FlashMode::VARIANTS, value_name = "MODE")]
    pub flash_mode: Option<FlashMode>,
    /// Flash size of the target
    #[clap(short = 's', long, possible_values = FlashSize::VARIANTS, value_name = "SIZE")]
    pub flash_size: Option<FlashSize>,
}

#[derive(Debug, Args)]
pub struct FlashArgs {
    /// Path to a binary (.bin) bootloader file
    #[clap(long)]
    pub bootloader: Option<PathBuf>,
    /// Erase the OTA data partition
    /// This is useful when using multiple OTA partitions and still wanting to
    /// be able to reflash via cargo-espflash or espflash
    #[clap(long)]
    pub erase_otadata: bool,
    /// Image format to flash
    #[clap(long, possible_values = ImageFormatType::VARIANTS)]
    pub format: Option<String>,
    /// Open a serial monitor after flashing
    #[clap(long)]
    pub monitor: bool,
    /// Path to a CSV file containing partition table
    #[clap(long)]
    pub partition_table: Option<PathBuf>,
    /// Load the application to RAM instead of Flash
    #[clap(long)]
    pub ram: bool,
}

/// Operations for partitions tables
#[derive(Debug, Args)]
pub struct PartitionTableArgs {
    /// Optional output file name, if unset will output to stdout
    #[clap(short = 'o', long)]
    output: Option<PathBuf>,
    /// Input partition table
    partition_table: PathBuf,
    /// Convert CSV parition table to binary representation
    #[clap(long, conflicts_with = "to-csv")]
    to_binary: bool,
    /// Convert binary partition table to CSV representation
    #[clap(long, conflicts_with = "to-binary")]
    to_csv: bool,
}

/// Save the image to disk instead of flashing to device
#[derive(Debug, Args)]
pub struct SaveImageArgs {
    /// Custom bootloader for merging
    #[clap(long)]
    pub bootloader: Option<PathBuf>,
    /// Chip to create an image for
    #[clap(long, possible_values = Chip::VARIANTS)]
    pub chip: Chip,
    /// File name to save the generated image to
    pub file: PathBuf,
    /// Boolean flag to merge binaries into single binary
    #[clap(long)]
    pub merge: bool,
    /// Custom partition table for merging
    #[clap(long, short = 'T', requires = "merge")]
    pub partition_table: Option<PathBuf>,
    /// Don't pad the image to the flash size
    #[clap(long, short = 'P', requires = "merge")]
    pub skip_padding: bool,
}

pub fn connect(args: &ConnectArgs, config: &Config) -> Result<Flasher> {
    let port_info = get_serial_port_info(args, config)?;

    // Attempt to open the serial port and set its initial baud rate.
    println!("Serial port: {}", port_info.port_name);
    println!("Connecting...\n");

    let interface = Interface::new(&port_info, args, config)
        .wrap_err_with(|| format!("Failed to open serial port {}", port_info.port_name))?;

    // NOTE: since `get_serial_port_info` filters out all non-USB serial ports, we
    //       can just pretend the remaining types don't exist here.
    let port_info = match port_info.port_type {
        SerialPortType::UsbPort(info) => info,
        SerialPortType::Unknown => {
            println!("Matched SerialPortType::Unknown");
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

pub fn board_info(args: ConnectArgs, config: &Config) -> Result<()> {
    let mut flasher = connect(&args, config)?;
    flasher.board_info()?;

    Ok(())
}

pub fn serial_monitor(args: ConnectArgs, config: &Config) -> Result<()> {
    let flasher = connect(&args, config)?;
    let pid = flasher.get_usb_pid()?;

    monitor(
        flasher.into_interface(),
        None,
        pid,
        args.monitor_baud.unwrap_or(115_200),
    )
    .into_diagnostic()?;

    Ok(())
}

pub fn save_elf_as_image(
    chip: Chip,
    elf_data: &[u8],
    image_path: PathBuf,
    image_format: Option<ImageFormatId>,
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

            let table =
                PartitionTable::try_from(data).wrap_err("Failed to parse partition table")?;

            Some(table)
        } else {
            None
        };

        // To get a chip revision, the connection is needed
        // For simplicity, the revision None is used
        let image = chip.get_flash_image(
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
        let flash_image = chip.get_flash_image(
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

pub fn flash_elf_image(
    flasher: &mut Flasher,
    elf_data: &[u8],
    bootloader: Option<&Path>,
    partition_table: Option<&Path>,
    image_format: Option<ImageFormatId>,
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
        let table = PartitionTable::try_from(data).wrap_err("Failed to parse partition table")?;

        Some(table)
    } else {
        None
    };

    if erase_otadata {
        let partition_table = match &partition_table {
            Some(partition_table) => partition_table,
            None => return Err((MissingPartitionTable {}).into()),
        };

        let otadata = match partition_table.find_by_subtype(
            partition_table::Type::CoreType(partition_table::CoreType::Data),
            partition_table::SubType::Data(partition_table::DataType::Ota),
        ) {
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
    println!("\nFlashing has completed!");

    Ok(())
}

pub fn partition_table(args: PartitionTableArgs) -> Result<()> {
    if args.to_binary {
        let input = fs::read(&args.partition_table).into_diagnostic()?;
        let part_table = PartitionTable::try_from_str(String::from_utf8(input).into_diagnostic()?)
            .into_diagnostic()?;

        // Use either stdout or a file if provided for the output.
        let mut writer: Box<dyn Write> = if let Some(output) = args.output {
            Box::new(fs::File::create(output).into_diagnostic()?)
        } else {
            Box::new(std::io::stdout())
        };
        part_table.save_bin(&mut writer).into_diagnostic()?;
    } else if args.to_csv {
        let input = fs::read(&args.partition_table).into_diagnostic()?;
        let part_table = PartitionTable::try_from_bytes(input).into_diagnostic()?;

        // Use either stdout or a file if provided for the output.
        let mut writer: Box<dyn Write> = if let Some(output) = args.output {
            Box::new(fs::File::create(output).into_diagnostic()?)
        } else {
            Box::new(std::io::stdout())
        };
        part_table.save_csv(&mut writer).into_diagnostic()?;
    } else {
        let input = fs::read(&args.partition_table).into_diagnostic()?;

        // Try getting the partition table from either the csv or the binary
        // representation and fail otherwise.
        let part_table = if let Ok(part_table) = PartitionTable::try_from(input).into_diagnostic() {
            part_table
        } else {
            return Err((InvalidPartitionTable {}).into());
        };

        part_table.pretty_print();
    }

    Ok(())
}

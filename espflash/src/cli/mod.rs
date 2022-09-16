//! CLI utilities shared between espflash and cargo-espflash
//!
//! No stability guaranties apply

use std::{
    fs::{self, File},
    io::{Read, Write},
    num::ParseIntError,
    path::{Path, PathBuf},
    time::Duration,
};

use clap::Parser;
use config::Config;
use miette::{IntoDiagnostic, Result, WrapErr};
use serialport::{FlowControl, SerialPortType, UsbPortInfo};
use strum::VariantNames;
use update_informer::{registry, Check};

use crate::{
    cli::monitor::monitor,
    cli::serial::get_serial_port_info,
    elf::{ElfFirmwareImage, FlashFrequency, FlashMode},
    error::{Error, NoOtadataError},
    flasher::FlashSize,
    partition_table, Chip, Flasher, ImageFormatId, InvalidPartitionTable, MissingPartitionTable,
    PartitionTable,
};

pub mod config;
pub mod monitor;

mod serial;

#[derive(Clone, Debug, Parser)]
pub struct ConnectOpts {
    /// Serial port connected to target device
    pub serial: Option<String>,

    /// Baud rate at which to flash target device
    #[clap(long)]
    pub speed: Option<u32>,

    /// Baud rate at which to read console output
    #[clap(long)]
    pub monitor_speed: Option<u32>,

    /// Use RAM stub for loading
    #[clap(long)]
    pub use_stub: bool,
}

#[derive(Clone, Debug, Parser)]
pub struct FlashOpts {
    /// Load the application to RAM instead of Flash
    #[clap(long)]
    pub ram: bool,
    /// Path to a binary (.bin) bootloader file
    #[clap(long)]
    pub bootloader: Option<PathBuf>,
    /// Path to a CSV file containing partition table
    #[clap(long)]
    pub partition_table: Option<PathBuf>,
    /// Open a serial monitor after flashing
    #[clap(long)]
    pub monitor: bool,
    /// Erase the OTADATA partition
    /// This is useful when using multiple OTA partitions and still wanting to be able to reflash via espflash
    #[clap(long)]
    pub erase_otadata: bool,
}

#[derive(Debug, Clone, Parser)]
pub struct FlashConfigOpts {
    /// Flash mode to use
    #[clap(short = 'm', long, possible_values = FlashMode::VARIANTS, value_name = "MODE")]
    pub flash_mode: Option<FlashMode>,
    /// Flash size of the target
    #[clap(short = 's', long, possible_values = FlashSize::VARIANTS, value_name = "SIZE")]
    pub flash_size: Option<FlashSize>,
    /// Flash frequency
    #[clap(short = 'f', long, possible_values = FlashFrequency::VARIANTS, value_name = "FREQUENCY")]
    pub flash_freq: Option<FlashFrequency>,
}

#[derive(Clone, Debug, Parser)]
pub struct PartitionTableOpts {
    /// Convert CSV parition table to binary representation
    #[clap(long, required_unless_present_any = ["info", "to-csv"])]
    to_binary: bool,
    /// Convert binary partition table to CSV representation
    #[clap(long, required_unless_present_any = ["info", "to-binary"])]
    to_csv: bool,
    /// Show information on partition table
    #[clap(short, long, required_unless_present_any = ["to-binary", "to-csv"])]
    info: bool,
    /// Input partition table
    partition_table: PathBuf,
    /// Optional output file name, if unset will output to stdout
    #[clap(short, long)]
    output: Option<PathBuf>,
}

#[derive(Clone, Debug, Parser)]
pub struct WriteBinToFlashOpts {
    /// Address at which to write the binary file
    #[clap(value_parser = parse_u32)]
    addr: u32,

    /// File containing the binary data to write
    bin_file: String,

    #[clap(flatten)]
    connect_opts: ConnectOpts,
}

fn parse_u32(input: &str) -> Result<u32, ParseIntError> {
    parse_int::parse(input)
}

pub fn connect(opts: &ConnectOpts, config: &Config) -> Result<Flasher> {
    let port_info = get_serial_port_info(opts, config)?;

    // Attempt to open the serial port and set its initial baud rate.
    println!("Serial port: {}", port_info.port_name);
    println!("Connecting...\n");
    let serial = serialport::new(&port_info.port_name, 115_200)
        .flow_control(FlowControl::None)
        .open()
        .map_err(Error::from)
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
        serial,
        port_info,
        opts.speed,
        opts.use_stub,
    )?)
}

pub fn board_info(opts: ConnectOpts, config: Config) -> Result<()> {
    let mut flasher = connect(&opts, &config)?;
    flasher.board_info()?;

    Ok(())
}

pub fn serial_monitor(opts: ConnectOpts, config: Config) -> Result<()> {
    let flasher = connect(&opts, &config)?;
    let pid = flasher.get_usb_pid()?;

    monitor(
        flasher.into_serial(),
        None,
        pid,
        opts.monitor_speed.unwrap_or(115200),
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

pub fn partition_table(opts: PartitionTableOpts) -> Result<()> {
    if opts.to_binary {
        let input = fs::read(&opts.partition_table).into_diagnostic()?;
        let part_table = PartitionTable::try_from_str(String::from_utf8(input).into_diagnostic()?)
            .into_diagnostic()?;

        // Use either stdout or a file if provided for the output.
        let mut writer: Box<dyn Write> = if let Some(output) = opts.output {
            Box::new(fs::File::create(output).into_diagnostic()?)
        } else {
            Box::new(std::io::stdout())
        };
        part_table.save_bin(&mut writer).into_diagnostic()?;
    } else if opts.to_csv {
        let input = fs::read(&opts.partition_table).into_diagnostic()?;
        let part_table = PartitionTable::try_from_bytes(input).into_diagnostic()?;

        // Use either stdout or a file if provided for the output.
        let mut writer: Box<dyn Write> = if let Some(output) = opts.output {
            Box::new(fs::File::create(output).into_diagnostic()?)
        } else {
            Box::new(std::io::stdout())
        };
        part_table.save_csv(&mut writer).into_diagnostic()?;
    } else if opts.info {
        let input = fs::read(&opts.partition_table).into_diagnostic()?;

        // Try getting the partition table from either the csv or the binary representation and
        // fail otherwise.
        let part_table = if let Ok(part_table) = PartitionTable::try_from(input).into_diagnostic() {
            part_table
        } else {
            return Err((InvalidPartitionTable {}).into());
        };

        part_table.pretty_print();
    }

    Ok(())
}

pub fn write_bin_to_flash(opts: WriteBinToFlashOpts) -> Result<()> {
    let config = Config::load()?;
    let mut flasher = connect(&opts.connect_opts, &config)?;
    flasher.board_info()?;

    let mut f = File::open(&opts.bin_file).into_diagnostic()?;
    let size = f.metadata().into_diagnostic()?.len();
    let mut buffer = Vec::with_capacity(size.try_into().into_diagnostic()?);
    f.read_to_end(&mut buffer).into_diagnostic()?;

    flasher.write_bin_to_flash(opts.addr, &buffer)?;

    Ok(())
}

pub fn check_for_updates(name: &str, version: &str) {
    const NO_INTERVAL: Duration = Duration::from_secs(0);

    let informer = update_informer::new(registry::Crates, name, version).interval(NO_INTERVAL);

    if let Some(version) = informer.check_version().ok().flatten() {
        println!("New version of {name} is available: {version}\n");
    }
}

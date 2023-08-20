//! Types and functions for the command-line interface
//!
//! The contents of this module are intended for use with the [cargo-espflash]
//! and [espflash] command-line applications, and are likely not of much use
//! otherwise.
//!
//! [cargo-espflash]: https://crates.io/crates/cargo-espflash
//! [espflash]: https://crates.io/crates/espflash

use std::{
    collections::HashMap,
    fs,
    io::Write,
    path::{Path, PathBuf},
};

use clap::Args;
use clap_complete::Shell;
use comfy_table::{modifiers, presets::UTF8_FULL, Attribute, Cell, Color, Table};
use esp_idf_part::{DataType, Partition, PartitionTable};
use indicatif::{style::ProgressStyle, HumanCount, ProgressBar};
use log::{debug, info};
use miette::{IntoDiagnostic, Result, WrapErr};
use serialport::{SerialPortType, UsbPortInfo};

use self::{config::Config, monitor::monitor, serial::get_serial_port_info};
use crate::{
    elf::ElfFirmwareImage,
    error::{MissingPartition, MissingPartitionTable},
    flasher::{FlashFrequency, FlashMode, FlashSize, Flasher, ProgressCallbacks},
    image_format::ImageFormatKind,
    interface::Interface,
    targets::Chip,
};

pub mod config;
pub mod monitor;

mod serial;

/// Establish a connection with a target device
#[derive(Debug, Args)]
pub struct ConnectArgs {
    /// Baud rate at which to communicate with target device
    #[arg(short = 'b', long, env = "ESPFLASH_BAUD")]
    pub baud: Option<u32>,
    /// Serial port connected to target device
    #[arg(short = 'p', long, env = "ESPFLASH_PORT")]
    pub port: Option<String>,
    /// DTR pin to use for the internal UART hardware. Uses BCM numbering.
    #[cfg(feature = "raspberry")]
    #[cfg_attr(feature = "raspberry", clap(long))]
    pub dtr: Option<u8>,
    /// RTS pin to use for the internal UART hardware. Uses BCM numbering.
    #[cfg(feature = "raspberry")]
    #[cfg_attr(feature = "raspberry", clap(long))]
    pub rts: Option<u8>,
    /// Do not use the RAM stub for loading
    #[arg(long)]
    pub no_stub: bool,
}

/// Generate completions for the given shell
#[derive(Debug, Args)]
pub struct CompletionsArgs {
    /// Shell to generate completions for.
    pub shell: Shell,
}

/// Configure communication with the target device's flash
#[derive(Debug, Args)]
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
#[group(skip)]
pub struct FlashArgs {
    /// Path to a binary (.bin) bootloader file
    #[arg(long, value_name = "FILE")]
    pub bootloader: Option<PathBuf>,
    /// Erase partitions by label
    #[arg(long, value_name = "LABELS", value_delimiter = ',')]
    pub erase_parts: Option<Vec<String>>,
    /// Erase specified data partitions
    #[arg(long, value_name = "PARTS", value_enum, value_delimiter = ',')]
    pub erase_data_parts: Option<Vec<DataType>>,
    /// Image format to flash
    #[arg(long, value_enum)]
    pub format: Option<ImageFormatKind>,
    /// Open a serial monitor after flashing
    #[arg(short = 'M', long)]
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
    /// Convert CSV partition table to binary representation
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
    #[arg(long, value_enum)]
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

/// Open the serial monitor without flashing
#[derive(Debug, Args)]
pub struct MonitorArgs {
    /// Optional file name of the ELF image to load the symbols from
    #[arg(short = 'e', long, value_name = "FILE")]
    elf: Option<PathBuf>,
    /// Connection configuration
    #[clap(flatten)]
    connect_args: ConnectArgs,
}

/// Erase entire flash of target device
#[derive(Debug, Args)]
pub struct EraseFlashArgs {
    /// Connection configuration
    #[clap(flatten)]
    pub connect_args: ConnectArgs,
}

/// Erase named partitions based on provided partition table
#[derive(Debug, Args)]
pub struct ErasePartsArgs {
    /// Connection configuration
    #[clap(flatten)]
    pub connect_args: ConnectArgs,

    #[arg(value_name = "LABELS", value_delimiter = ',')]
    pub erase_parts: Vec<String>,

    /// Input partition table
    #[arg(long, value_name = "FILE")]
    pub partition_table: PathBuf,
}

/// Select a serial port and establish a connection with a target device
pub fn connect(args: &ConnectArgs, config: &Config) -> Result<Flasher> {
    let port_info = get_serial_port_info(args, config)?;

    // Attempt to open the serial port and set its initial baud rate.
    info!("Serial port: '{}'", port_info.port_name);
    info!("Connecting...");

    #[cfg(feature = "raspberry")]
    let (dtr, rts) = (
        args.dtr.or(config.connection.dtr),
        args.rts.or(config.connection.rts),
    );
    #[cfg(not(feature = "raspberry"))]
    let (dtr, rts) = (None, None);

    let interface = Interface::new(&port_info, dtr, rts)
        .wrap_err_with(|| format!("Failed to open serial port {}", port_info.port_name))?;

    // NOTE: since `get_serial_port_info` filters out all PCI Port and Bluetooth
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

    Ok(Flasher::connect(
        interface,
        port_info,
        args.baud,
        !args.no_stub,
    )?)
}

/// Connect to a target device and print information about its chip
pub fn board_info(args: &ConnectArgs, config: &Config) -> Result<()> {
    let mut flasher = connect(args, config)?;
    print_board_info(&mut flasher)?;

    Ok(())
}

/// Generate shell completions for the given shell
pub fn completions(args: &CompletionsArgs, app: &mut clap::Command, bin_name: &str) -> Result<()> {
    clap_complete::generate(args.shell, app, bin_name, &mut std::io::stdout());

    Ok(())
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
    println!("Crystal frequency: {}MHz", info.crystal_frequency);
    println!("Flash size:        {}", info.flash_size);
    println!("Features:          {}", info.features.join(", "));
    println!("MAC address:       {}", info.mac_address);

    Ok(())
}

/// Open a serial monitor
pub fn serial_monitor(args: MonitorArgs, config: &Config) -> Result<()> {
    let mut flasher = connect(&args.connect_args, config)?;
    let pid = flasher.get_usb_pid()?;

    let elf = if let Some(elf_path) = args.elf {
        let path = fs::canonicalize(elf_path).into_diagnostic()?;
        let data = fs::read(path).into_diagnostic()?;

        Some(data)
    } else {
        None
    };

    let chip = flasher.chip();
    let target = chip.into_target();

    // The 26MHz ESP32-C2's need to be treated as a special case.
    let default_baud = if chip == Chip::Esp32c2
        && args.connect_args.no_stub
        && target.crystal_freq(flasher.connection())? == 26
    {
        74_880
    } else {
        115_200
    };

    monitor(
        flasher.into_interface(),
        elf.as_deref(),
        pid,
        args.connect_args.baud.unwrap_or(default_baud),
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

        display_image_size(image.app_size(), image.part_size());

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
                flash_size.unwrap_or_default().size() as usize
                    - file.metadata().into_diagnostic()?.len() as usize
            ];
            file.write_all(&padding_bytes).into_diagnostic()?;
        }
    } else {
        let image = chip.into_target().get_flash_image(
            &image,
            None,
            None,
            image_format,
            None,
            flash_mode,
            flash_size,
            flash_freq,
        )?;

        display_image_size(image.app_size(), image.part_size());

        let parts = image.ota_segments().collect::<Vec<_>>();
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
#[derive(Default)]
pub struct EspflashProgress {
    pb: Option<ProgressBar>,
}

impl ProgressCallbacks for EspflashProgress {
    /// Initialize the progress bar
    fn init(&mut self, addr: u32, len: usize) {
        let pb = ProgressBar::new(len as u64)
            .with_message(format!("{addr:#X}"))
            .with_style(
                ProgressStyle::default_bar()
                    .template("[{elapsed_precise}] [{bar:40}] {pos:>7}/{len:7} {msg}")
                    .unwrap()
                    .progress_chars("=> "),
            );

        self.pb = Some(pb);
    }

    /// Update the progress bar
    fn update(&mut self, current: usize) {
        if let Some(ref pb) = self.pb {
            pb.set_position(current as u64);
        }
    }

    /// End the progress bar
    fn finish(&mut self) {
        if let Some(ref pb) = self.pb {
            pb.finish();
        }
    }
}

/// Write an ELF image to a target device's flash
pub fn flash_elf_image(
    flasher: &mut Flasher,
    elf_data: &[u8],
    bootloader: Option<&Path>,
    partition_table: Option<PartitionTable>,
    image_format: Option<ImageFormatKind>,
    flash_mode: Option<FlashMode>,
    flash_size: Option<FlashSize>,
    flash_freq: Option<FlashFrequency>,
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
        Some(&mut EspflashProgress::default()),
    )?;
    info!("Flashing has completed!");

    Ok(())
}

/// Parse a [PartitionTable] from the provided path
pub fn parse_partition_table(path: &Path) -> Result<PartitionTable> {
    let data = fs::read(path)
        .into_diagnostic()
        .wrap_err("Failed to open partition table")?;

    PartitionTable::try_from(data).into_diagnostic()
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
        pretty.add_row(vec![
            Cell::new(p.name()).fg(Color::Green),
            Cell::new(p.ty().to_string()).fg(Color::Cyan),
            Cell::new(p.subtype().to_string()).fg(Color::Magenta),
            Cell::new(format!("{:#x}", p.offset())).fg(Color::Red),
            Cell::new(format!("{:#x} ({}KiB)", p.size(), p.size() / 1024)).fg(Color::Yellow),
            Cell::new(p.encrypted()).fg(Color::DarkCyan),
        ]);
    }

    println!("{pretty}");
}

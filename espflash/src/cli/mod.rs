//! CLI utilities shared between espflash and cargo-espflash
//!
//! No stability guaranties apply

use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};

use clap::Parser;
use config::Config;
use miette::{IntoDiagnostic, Result, WrapErr};
use serialport::{FlowControl, SerialPortType};
use strum::VariantNames;

use crate::{
    cli::serial::get_serial_port_info,
    elf::{FirmwareImageBuilder, FlashFrequency, FlashMode},
    error::Error,
    flasher::FlashSize,
    Chip, Flasher, ImageFormatId, PartitionTable,
};

pub mod config;
pub mod monitor;

mod serial;

#[derive(Parser)]
pub struct ConnectOpts {
    /// Serial port connected to target device
    pub serial: Option<String>,
    /// Baud rate at which to flash target device
    #[clap(long)]
    pub speed: Option<u32>,
}

#[derive(Parser)]
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
}

#[derive(Parser)]
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
        _ => unreachable!(),
    };

    Ok(Flasher::connect(serial, port_info, opts.speed)?)
}

pub fn board_info(opts: ConnectOpts, config: Config) -> Result<()> {
    let mut flasher = connect(&opts, &config)?;
    flasher.board_info()?;

    Ok(())
}

pub fn save_elf_as_image(
    chip: Chip,
    elf_data: &[u8],
    path: PathBuf,
    image_format: Option<ImageFormatId>,
    flash_mode: Option<FlashMode>,
    flash_size: Option<FlashSize>,
    flash_freq: Option<FlashFrequency>,
    merge: bool,
    bootloader_path: Option<PathBuf>,
    partition_table_path: Option<PathBuf>,
) -> Result<()> {
    let image = FirmwareImageBuilder::new(elf_data)
        .flash_mode(flash_mode)
        .flash_size(flash_size)
        .flash_freq(flash_freq)
        .build()?;

    let flash_image = chip.get_flash_image(&image, None, None, image_format, None)?;
    let parts: Vec<_> = flash_image.ota_segments().collect();

    match parts.as_slice() {
        [single] => fs::write(&path, &single.data).into_diagnostic()?,
        parts => {
            for part in parts {
                let part_path = format!("{:#x}_{}", part.addr, path.display());
                fs::write(part_path, &part.data).into_diagnostic()?
            }
        }
    }

    // merge_bin is TRUE
    // merge bootloader, partition table and app binaries
    // basic functionality, only merge 3 binaries
    if merge {
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
        // the CSV at the specified path.
        let partition_table = if let Some(partition_table_path) = partition_table_path {
            let path = fs::canonicalize(partition_table_path).into_diagnostic()?;
            let data = fs::read_to_string(path)
                .into_diagnostic()
                .wrap_err("Failed to open partition table")?;

            let table =
                PartitionTable::try_from_str(data).wrap_err("Failed to parse partition table")?;

            Some(table)
        } else {
            None
        };

        // To get a chip revision, the connection is needed
        // For simplicity, the revision None is used
        let image =
            chip.get_flash_image(&image, bootloader, partition_table, image_format, None)?;

        let merged_bin = format!(
            "merged_{path}",
            path = &path.to_str().unwrap_or("merged_bins.bin")
        );

        if Path::new(&merged_bin).exists() {
            fs::remove_file(&merged_bin).into_diagnostic()?;
        }

        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(merged_bin)
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

        // Take flash_size as input parameter, if None, use default value of 4Mb
        let padding_bytes = vec![
            0xffu8;
            flash_size.unwrap_or(FlashSize::Flash4Mb).size() as usize
                - file.metadata().into_diagnostic()?.len() as usize
        ];
        file.write_all(&padding_bytes).into_diagnostic()?;
    }

    println!("The final merge binary was created successfully.");
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
    // the CSV at the specified path.
    let partition_table = if let Some(path) = partition_table {
        let path = fs::canonicalize(path).into_diagnostic()?;
        let data = fs::read_to_string(path)
            .into_diagnostic()
            .wrap_err("Failed to open partition table")?;

        let table =
            PartitionTable::try_from_str(data).wrap_err("Failed to parse partition table")?;

        Some(table)
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
    )?;
    println!("\nFlashing has completed!");

    Ok(())
}

use std::{
    collections::HashMap,
    fs::{self, File},
    io::Read,
    num::ParseIntError,
    path::PathBuf,
};

use clap::{Args, Parser, Subcommand};
use espflash::{
    cli::{
        self, board_info, clap_enum_variants, config::Config, connect, erase_partition,
        flash_elf_image, monitor::monitor, parse_partition_table, partition_table,
        save_elf_as_image, serial_monitor, ConnectArgs, FlashConfigArgs, PartitionTableArgs,
    },
    error::{MissingPartition, MissingPartitionTable},
    image_format::ImageFormatKind,
    logging::initialize_logger,
    update::check_for_update,
};
use log::{debug, LevelFilter};
use miette::{IntoDiagnostic, Result, WrapErr};
use strum::VariantNames;

#[derive(Debug, Parser)]
#[clap(about, version, propagate_version = true)]
struct Cli {
    #[clap(subcommand)]
    subcommand: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Display information about the connected board and exit without flashing
    BoardInfo(ConnectArgs),
    /// Flash an application to a target device
    Flash(FlashArgs),
    /// Open the serial monitor without flashing
    Monitor(ConnectArgs),
    PartitionTable(PartitionTableArgs),
    SaveImage(SaveImageArgs),
    WriteBin(WriteBinArgs),
}

#[derive(Debug, Args)]
struct FlashArgs {
    /// ELF image to flash
    image: PathBuf,

    #[clap(flatten)]
    connect_args: ConnectArgs,
    #[clap(flatten)]
    pub flash_config_args: FlashConfigArgs,
    #[clap(flatten)]
    flash_args: cli::FlashArgs,
}

#[derive(Debug, Args)]
struct SaveImageArgs {
    /// Image format to flash
    #[arg(long, value_parser = clap_enum_variants!(ImageFormatKind))]
    format: Option<ImageFormatKind>,

    #[clap(flatten)]
    pub flash_config_args: FlashConfigArgs,
    #[clap(flatten)]
    save_image_args: cli::SaveImageArgs,

    /// ELF image to flash
    image: PathBuf,
}

/// Writes a binary file to a specific address in the chip's flash
#[derive(Debug, Args)]
struct WriteBinArgs {
    /// Address at which to write the binary file
    #[arg(value_parser = parse_uint32)]
    pub addr: u32,
    /// File containing the binary data to write
    pub bin_file: String,

    #[clap(flatten)]
    connect_args: ConnectArgs,
}

fn parse_uint32(input: &str) -> Result<u32, ParseIntError> {
    parse_int::parse(input)
}

fn main() -> Result<()> {
    miette::set_panic_hook();
    initialize_logger(LevelFilter::Info);

    // Attempt to parse any provided comand-line arguments, or print the help
    // message and terminate if the invocation is not correct.
    let args = Cli::parse().subcommand;
    debug!("{:#?}", args);

    // Only check for updates once the command-line arguments have been processed,
    // to avoid printing any update notifications when the help message is
    // displayed.
    check_for_update(env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"));

    // Load any user configuraiton, if present.
    let config = Config::load().unwrap();

    // Execute the correct action based on the provided subcommand and its
    // associated arguments.
    match args {
        Commands::BoardInfo(args) => board_info(args, &config),
        Commands::Flash(args) => flash(args, &config),
        Commands::Monitor(args) => serial_monitor(args, &config),
        Commands::PartitionTable(args) => partition_table(args),
        Commands::SaveImage(args) => save_image(args),
        Commands::WriteBin(args) => write_bin(args, &config),
    }
}

fn flash(args: FlashArgs, config: &Config) -> Result<()> {
    let mut flasher = connect(&args.connect_args, config)?;
    flasher.board_info()?;

    // Read the ELF data from the build path and load it to the target.
    let elf_data = fs::read(&args.image).into_diagnostic()?;

    if args.flash_args.ram {
        flasher.load_elf_to_ram(&elf_data)?;
    } else {
        let bootloader = args.flash_args.bootloader.as_deref();
        let partition_table = match args.flash_args.partition_table.as_deref() {
            Some(path) => Some(parse_partition_table(path)?),
            None => None,
        };

        if args.flash_args.erase_parts.is_some() || args.flash_args.erase_data_parts.is_some() {
            let partition_table = match &partition_table {
                Some(partition_table) => partition_table,
                None => return Err((MissingPartitionTable {}).into()),
            };

            // Using a hashmap to deduplicate entries
            let mut parts_to_erase = None;

            // Look for any part with specific label
            if let Some(part_labels) = args.flash_args.erase_parts {
                for label in part_labels {
                    let part = partition_table
                        .find(label.as_str())
                        .ok_or(MissingPartition::from(label))?;
                    parts_to_erase
                        .get_or_insert(HashMap::new())
                        .insert(part.offset(), part);
                }
            }
            // Look for any data partitions with specific data subtype
            // There might be multiple partition of the same subtype, e.g. when using multiple FAT partitions
            if let Some(partition_types) = args.flash_args.erase_data_parts {
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
                    .try_for_each(|(_, p)| erase_partition(&mut flasher, p))?;
            }
        }

        flash_elf_image(
            &mut flasher,
            &elf_data,
            bootloader,
            partition_table,
            args.flash_args.format,
            args.flash_config_args.flash_mode,
            args.flash_config_args.flash_size,
            args.flash_config_args.flash_freq,
        )?;
    }

    if args.flash_args.monitor {
        let pid = flasher.get_usb_pid()?;

        monitor(
            flasher.into_interface(),
            Some(&elf_data),
            pid,
            args.flash_args.monitor_baud.unwrap_or(115_200),
        )
        .into_diagnostic()?;
    }

    Ok(())
}

fn save_image(args: SaveImageArgs) -> Result<()> {
    let elf_data = fs::read(&args.image)
        .into_diagnostic()
        .wrap_err_with(|| format!("Failed to open image {}", args.image.display()))?;

    // Since we have no `Flasher` instance and as such cannot print the board
    // information, we will print whatever information we _do_ have.
    println!("Chip type:         {}", args.save_image_args.chip);
    if let Some(format) = args.format {
        println!("Image format:      {:?}", format);
    }
    println!("Merge:             {}", args.save_image_args.merge);
    println!("Skip padding:      {}", args.save_image_args.skip_padding);

    save_elf_as_image(
        args.save_image_args.chip,
        &elf_data,
        args.save_image_args.file,
        args.format,
        args.flash_config_args.flash_mode,
        args.flash_config_args.flash_size,
        args.flash_config_args.flash_freq,
        args.save_image_args.merge,
        args.save_image_args.bootloader,
        args.save_image_args.partition_table,
        args.save_image_args.skip_padding,
    )?;

    Ok(())
}

fn write_bin(args: WriteBinArgs, config: &Config) -> Result<()> {
    let mut flasher = connect(&args.connect_args, config)?;
    flasher.board_info()?;

    let mut f = File::open(&args.bin_file).into_diagnostic()?;
    let size = f.metadata().into_diagnostic()?.len();
    let mut buffer = Vec::with_capacity(size.try_into().into_diagnostic()?);
    f.read_to_end(&mut buffer).into_diagnostic()?;

    flasher.write_bin_to_flash(args.addr, &buffer)?;

    Ok(())
}

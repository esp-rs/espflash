use std::{
    fs::{self, File},
    io::Read,
    num::ParseIntError,
    path::PathBuf,
};

use clap::{Args, Parser, Subcommand};
use espflash::{
    cli::{
        self, board_info, config::Config, connect, erase_partitions, flash_elf_image,
        monitor::monitor, parse_partition_table, partition_table, print_board_info,
        save_elf_as_image, serial_monitor, ConnectArgs, EspflashProgress, FlashConfigArgs,
        MonitorArgs, PartitionTableArgs,
    },
    image_format::ImageFormatKind,
    logging::initialize_logger,
    targets::Chip,
    update::check_for_update,
};
use log::{debug, LevelFilter};
use miette::{IntoDiagnostic, Result, WrapErr};

#[derive(Debug, Parser)]
#[clap(about, version, propagate_version = true)]
struct Cli {
    #[clap(subcommand)]
    subcommand: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    BoardInfo(ConnectArgs),
    Flash(FlashArgs),
    Monitor(MonitorArgs),
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
    #[arg(long, value_enum)]
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
    let config = Config::load()?;

    // Execute the correct action based on the provided subcommand and its
    // associated arguments.
    match args {
        Commands::BoardInfo(args) => board_info(&args, &config),
        Commands::Flash(args) => flash(args, &config),
        Commands::Monitor(args) => serial_monitor(args, &config),
        Commands::PartitionTable(args) => partition_table(args),
        Commands::SaveImage(args) => save_image(args),
        Commands::WriteBin(args) => write_bin(args, &config),
    }
}

fn flash(args: FlashArgs, config: &Config) -> Result<()> {
    let mut flasher = connect(&args.connect_args, config)?;
    print_board_info(&mut flasher)?;

    let chip = flasher.chip();
    let target = chip.into_target();
    let target_xtal_freq = target.crystal_freq(flasher.connection())?;

    // Read the ELF data from the build path and load it to the target.
    let elf_data = fs::read(&args.image).into_diagnostic()?;

    if args.flash_args.ram {
        flasher.load_elf_to_ram(&elf_data, Some(&mut EspflashProgress::default()))?;
    } else {
        let bootloader = args.flash_args.bootloader.as_deref();
        let partition_table = args.flash_args.partition_table.as_deref();

        if let Some(path) = bootloader {
            println!("Bootloader:        {}", path.display());
        }
        if let Some(path) = partition_table {
            println!("Partition table:   {}", path.display());
        }

        let partition_table = match partition_table {
            Some(path) => Some(parse_partition_table(path)?),
            None => None,
        };

        if args.flash_args.erase_parts.is_some() || args.flash_args.erase_data_parts.is_some() {
            erase_partitions(
                &mut flasher,
                partition_table.clone(),
                args.flash_args.erase_parts,
                args.flash_args.erase_data_parts,
            )?;
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

        // The 26MHz ESP32-C2's need to be treated as a special case.
        let default_baud =
            if chip == Chip::Esp32c2 && args.connect_args.no_stub && target_xtal_freq == 26 {
                74_880
            } else {
                115_200
            };

        monitor(
            flasher.into_interface(),
            Some(&elf_data),
            pid,
            args.flash_args.monitor_baud.unwrap_or(default_baud),
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
    if let Some(path) = &args.save_image_args.bootloader {
        println!("Bootloader:        {}", path.display());
    }
    if let Some(path) = &args.save_image_args.partition_table {
        println!("Partition table:   {}", path.display());
    }

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
    print_board_info(&mut flasher)?;

    let mut f = File::open(&args.bin_file).into_diagnostic()?;
    let size = f.metadata().into_diagnostic()?.len();
    let mut buffer = Vec::with_capacity(size.try_into().into_diagnostic()?);
    f.read_to_end(&mut buffer).into_diagnostic()?;

    flasher.write_bin_to_flash(args.addr, &buffer, Some(&mut EspflashProgress::default()))?;

    Ok(())
}

use std::{
    fs::{self, File},
    io::Read,
    num::ParseIntError,
    path::PathBuf,
    str::FromStr,
};

use clap::{Args, Parser, Subcommand};
use espflash::{
    cli::{
        board_info, connect, flash_elf_image, monitor::monitor, partition_table, save_elf_as_image,
        serial_monitor, ConnectArgs, FlashArgs as BaseFlashArgs, FlashConfigArgs,
        PartitionTableArgs, SaveImageArgs as BaseSaveImageArgs,
    },
    image_format::{ImageFormatId, ImageFormatType},
    logging::initialize_logger,
    update::check_for_update,
    Config,
};
use log::{debug, LevelFilter};
use miette::{IntoDiagnostic, Result, WrapErr};
use strum::VariantNames;

#[derive(Debug, Parser)]
#[clap(about, propagate_version = true, version)]
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
    flash_args: BaseFlashArgs,
}

#[derive(Debug, Args)]
struct SaveImageArgs {
    /// Image format to flash
    #[clap(long, possible_values = ImageFormatType::VARIANTS)]
    format: Option<String>,

    #[clap(flatten)]
    pub flash_config_args: FlashConfigArgs,
    #[clap(flatten)]
    save_image_args: BaseSaveImageArgs,

    /// ELF image to flash
    image: PathBuf,
}

/// Writes a binary file to a specific address in the chip's flash
#[derive(Debug, Args)]
struct WriteBinArgs {
    /// Address at which to write the binary file
    #[clap(value_parser = parse_uint32)]
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

fn flash(mut args: FlashArgs, config: &Config) -> Result<()> {
    // The `erase_otadata` argument requires `use_stub`, which is implicitly
    // enabled here.
    if args.flash_args.erase_otadata {
        args.connect_args.use_stub = true;
    }

    let mut flasher = connect(&args.connect_args, config)?;
    flasher.board_info()?;

    // Read the ELF data from the build path and load it to the target.
    let elf_data = fs::read(&args.image).into_diagnostic()?;

    if args.flash_args.ram {
        flasher.load_elf_to_ram(&elf_data)?;
    } else {
        let bootloader = args.flash_args.bootloader.as_deref();
        let partition_table = args.flash_args.partition_table.as_deref();

        let image_format = args
            .flash_args
            .format
            .as_deref()
            .map(ImageFormatId::from_str)
            .transpose()?;

        flash_elf_image(
            &mut flasher,
            &elf_data,
            bootloader,
            partition_table,
            image_format,
            args.flash_config_args.flash_mode,
            args.flash_config_args.flash_size,
            args.flash_config_args.flash_freq,
            args.flash_args.erase_otadata,
        )?;
    }

    if args.flash_args.monitor {
        let pid = flasher.get_usb_pid()?;

        monitor(
            flasher.into_interface(),
            Some(&elf_data),
            pid,
            args.connect_args.monitor_baud.unwrap_or(115_200),
        )
        .into_diagnostic()?;
    }

    Ok(())
}

fn save_image(args: SaveImageArgs) -> Result<()> {
    let elf_data = fs::read(&args.image)
        .into_diagnostic()
        .wrap_err_with(|| format!("Failed to open image {}", args.image.display()))?;

    let image_format = args
        .format
        .as_deref()
        .map(ImageFormatId::from_str)
        .transpose()?;

    save_elf_as_image(
        args.save_image_args.chip,
        &elf_data,
        args.save_image_args.file,
        image_format,
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

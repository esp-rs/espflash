use std::{
    fs::{self, File},
    io::Read,
    num::ParseIntError,
    path::PathBuf,
};

use clap::{Args, CommandFactory, Parser, Subcommand};
use espflash::{
    cli::{
        self, board_info, completions, config::Config, connect, erase_partitions, flash_elf_image,
        monitor::monitor, parse_partition_table, partition_table, print_board_info,
        save_elf_as_image, serial_monitor, CompletionsArgs, ConnectArgs, EraseFlashArgs,
        EspflashProgress, FlashConfigArgs, MonitorArgs, PartitionTableArgs,
    },
    image_format::ImageFormatKind,
    logging::initialize_logger,
    targets::Chip,
    update::check_for_update,
};
use log::{debug, info, LevelFilter};
use miette::{IntoDiagnostic, Result, WrapErr};

#[derive(Debug, Parser)]
#[command(about, max_term_width = 100, propagate_version = true, version)]
pub struct Cli {
    #[command(subcommand)]
    subcommand: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Print information about a connected target device
    ///
    /// Automatically detects and prints the chip type, crystal frequency, flash
    /// size, chip features, and MAC address of a connected target device.
    BoardInfo(ConnectArgs),
    /// Generate completions for the given shell
    ///
    /// The completions are printed to stdout, and can be redirected as needed.
    /// The directory in which completion scripts are stored differs
    /// depending on which shell is being used; consult your shell's
    /// documentation to determine the appropriate path.
    Completions(CompletionsArgs),
    /// Erase Flash entirely
    EraseFlash(EraseFlashArgs),
    /// Flash an application in ELF format to a connected target device
    ///
    /// Given a path to an ELF file, first convert it into the appropriate
    /// binary application image format as required by the ESP32 devices. Once
    /// we have a valid application image, we can write the bootloader,
    /// partition table, and application image to the connected target device.
    ///
    /// Please refer to the ESP-IDF documentation for more information on the
    /// binary image format:
    ///
    /// https://docs.espressif.com/projects/esp-idf/en/latest/esp32/api-reference/system/app_image_format.html
    Flash(FlashArgs),
    /// Open the serial monitor without flashing the connected target device
    Monitor(MonitorArgs),
    /// Convert partition tables between CSV and binary format
    ///
    /// Uses the ESP-IDF format for partition tables; please refer to the
    /// ESP-IDF documentation for more information on this format:
    ///
    /// https://docs.espressif.com/projects/esp-idf/en/latest/esp32/api-guides/partition-tables.html
    ///
    /// Allows for conversion between formats via the '--to-csv' and
    /// '--to-binary' options, plus the ability to print a partition table
    /// in tabular format.
    PartitionTable(PartitionTableArgs),
    /// Generate a binary application image and save it to a local disk
    ///
    /// If the '--merge' option is used, then the bootloader, partition table,
    /// and all application segments will be merged into a single binary file.
    /// Otherwise, each segment will be saved as individual binaries, prefixed
    /// with their intended addresses in flash.
    SaveImage(SaveImageArgs),
    /// Write a binary file to a specific address in a target device's flash
    WriteBin(WriteBinArgs),
}

#[derive(Debug, Args)]
struct FlashArgs {
    /// Connection configuration
    #[clap(flatten)]
    connect_args: ConnectArgs,
    /// Flashing configuration
    #[clap(flatten)]
    pub flash_config_args: FlashConfigArgs,
    /// Flashing arguments
    #[clap(flatten)]
    flash_args: cli::FlashArgs,
    /// ELF image to flash
    image: PathBuf,
}

#[derive(Debug, Args)]
struct SaveImageArgs {
    /// ELF image to flash
    image: PathBuf,
    /// Image format to flash
    #[arg(long, value_enum)]
    format: Option<ImageFormatKind>,
    /// Flashing configuration
    #[clap(flatten)]
    pub flash_config_args: FlashConfigArgs,
    /// Sage image arguments
    #[clap(flatten)]
    save_image_args: cli::SaveImageArgs,
}

/// Writes a binary file to a specific address in the chip's flash
#[derive(Debug, Args)]
struct WriteBinArgs {
    /// Address at which to write the binary file
    #[arg(value_parser = parse_uint32)]
    pub addr: u32,
    /// File containing the binary data to write
    pub bin_file: String,
    /// Connection configuration
    #[clap(flatten)]
    connect_args: ConnectArgs,
}

/// Parses a string as a 32-bit unsigned integer.
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
        Commands::Completions(args) => completions(&args, &mut Cli::command(), "espflash"),
        Commands::EraseFlash(args) => erase_flash(args, &config),
        Commands::Flash(args) => flash(args, &config),
        Commands::Monitor(args) => serial_monitor(args, &config),
        Commands::PartitionTable(args) => partition_table(args),
        Commands::SaveImage(args) => save_image(args),
        Commands::WriteBin(args) => write_bin(args, &config),
    }
}

fn erase_flash(args: EraseFlashArgs, config: &Config) -> Result<()> {
    info!("Erasing Flash...");
    let mut flash = connect(&args.connect_args, config)?;
    flash.erase_flash()?;

    Ok(())
}

fn flash(args: FlashArgs, config: &Config) -> Result<()> {
    let mut flasher = connect(&args.connect_args, config)?;

    // If the user has provided a flash size via a command-line argument, we'll
    // override the detected (or default) value with this.
    if let Some(flash_size) = args.flash_config_args.flash_size {
        flasher.set_flash_size(flash_size);
    }

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

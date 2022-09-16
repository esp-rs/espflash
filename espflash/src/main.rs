use std::{fs, mem::swap, path::PathBuf, str::FromStr};

use clap::{IntoApp, Parser};
use espflash::{
    cli::{
        board_info, check_for_updates, connect, flash_elf_image, monitor::monitor, partition_table,
        save_elf_as_image, serial_monitor, write_bin_to_flash, ConnectOpts, FlashConfigOpts,
        FlashOpts, PartitionTableOpts, WriteBinToFlashOpts,
    },
    Chip, Config, ImageFormatId,
};
use log::debug;
use miette::{IntoDiagnostic, Result, WrapErr};
use strum::VariantNames;
use tracing_subscriber::{filter::LevelFilter, EnvFilter};

#[derive(Debug, Parser)]
#[clap(version, propagate_version = true)]
struct Opts {
    /// Image format to flash
    #[clap(long, possible_values = &["bootloader", "direct-boot"])]
    pub format: Option<String>,

    #[clap(flatten)]
    pub flash_config_opts: FlashConfigOpts,

    #[clap(flatten)]
    flash_opts: FlashOpts,

    #[clap(flatten)]
    connect_opts: ConnectOpts,

    /// ELF image to flash
    image: Option<String>,

    #[clap(subcommand)]
    subcommand: Option<SubCommand>,

    /// Log level
    #[clap(long, default_value = "info", env)]
    log_level: LevelFilter,
}

#[derive(Debug, Parser)]
pub enum SubCommand {
    /// Display information about the connected board and exit without flashing
    BoardInfo(ConnectOpts),
    /// Save the image to disk instead of flashing to device
    SaveImage(SaveImageOpts),
    /// Open the serial monitor without flashing
    SerialMonitor(ConnectOpts),
    /// Operations for partitions tables
    PartitionTable(PartitionTableOpts),
    /// Writes a binary file to a specific address in the chip's flash
    WriteBinToFlash(WriteBinToFlashOpts),
}

#[derive(Debug, Parser)]
pub struct SaveImageOpts {
    #[clap(flatten)]
    pub flash_config_opts: FlashConfigOpts,
    /// Image format to flash
    #[clap(long, possible_values = &["bootloader", "direct-boot"])]
    format: Option<String>,
    /// Chip to create an image for
    #[clap(possible_values = Chip::VARIANTS)]
    chip: Chip,
    /// ELF image to flash
    image: PathBuf,
    /// File name to save the generated image to
    file: PathBuf,
    /// Boolean flag, if set, bootloader, partition table and application
    /// binaries will be merged into single binary
    #[clap(long, short = 'M')]
    pub merge: bool,
    /// Custom bootloader for merging
    #[clap(long, short = 'B')]
    pub bootloader: Option<PathBuf>,
    /// Custom partition table for merging
    #[clap(long, short = 'T')]
    pub partition_table: Option<PathBuf>,
    /// Don't pad the image to the flash size
    #[clap(long, short = 'P')]
    pub skip_padding: bool,
}

fn main() -> Result<()> {
    miette::set_panic_hook();

    check_for_updates(env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"));

    // Read options and configuration
    let mut opts = Opts::parse();
    let config = Config::load()?;

    debug!("options: {:?}", opts);

    if opts.flash_opts.erase_otadata {
        opts.connect_opts.use_stub = true;
    }

    // Setup logging
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive(opts.log_level.into()))
        .init();

    if opts.subcommand.is_none() {
        // If neither the IMAGE nor SERIAL arguments have been provided, print the
        // help message and exit.
        if opts.image.is_none() && opts.connect_opts.serial.is_none() {
            Opts::command().print_help().ok();
            return Ok(());
        }

        // If only a single argument is passed, it *should* always be the ELF file. In
        // the case that the serial port was not provided as a command-line argument, we
        // will either load the value specified in the configuration file or do port
        // auto-detection instead.
        if opts.image.is_none() && opts.connect_opts.serial.is_some() {
            swap(&mut opts.image, &mut opts.connect_opts.serial);
        }
    }

    if let Some(subcommand) = opts.subcommand {
        use SubCommand::*;

        match subcommand {
            BoardInfo(opts) => board_info(opts, config),
            SaveImage(opts) => save_image(opts),
            SerialMonitor(opts) => serial_monitor(opts, config),
            PartitionTable(opts) => partition_table(opts),
            WriteBinToFlash(opts) => write_bin_to_flash(opts),
        }
    } else {
        flash(opts, config)
    }
}

fn flash(opts: Opts, config: Config) -> Result<()> {
    let mut flasher = connect(&opts.connect_opts, &config)?;
    flasher.board_info()?;

    let elf = if let Some(elf) = opts.image {
        elf
    } else {
        Opts::command().print_help().ok();
        return Ok(());
    };

    // Read the ELF data from the build path and load it to the target.
    let elf_data = fs::read(&elf).into_diagnostic()?;

    if opts.flash_opts.ram {
        flasher.load_elf_to_ram(&elf_data)?;
    } else {
        let bootloader = opts.flash_opts.bootloader.as_deref();
        let partition_table = opts.flash_opts.partition_table.as_deref();

        let image_format = opts
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
            opts.flash_config_opts.flash_mode,
            opts.flash_config_opts.flash_size,
            opts.flash_config_opts.flash_freq,
            opts.flash_opts.erase_otadata,
        )?;
    }

    if opts.flash_opts.monitor {
        let pid = flasher.get_usb_pid()?;

        monitor(
            flasher.into_serial(),
            Some(&elf_data),
            pid,
            opts.connect_opts.monitor_speed.unwrap_or(115200),
        )
        .into_diagnostic()?;
    }

    Ok(())
}

fn save_image(opts: SaveImageOpts) -> Result<()> {
    let elf_data = fs::read(&opts.image)
        .into_diagnostic()
        .wrap_err_with(|| format!("Failed to open image {}", opts.image.display()))?;

    let image_format = opts
        .format
        .as_deref()
        .map(ImageFormatId::from_str)
        .transpose()?;

    save_elf_as_image(
        opts.chip,
        &elf_data,
        opts.file,
        image_format,
        opts.flash_config_opts.flash_mode,
        opts.flash_config_opts.flash_size,
        opts.flash_config_opts.flash_freq,
        opts.merge,
        opts.bootloader,
        opts.partition_table,
        opts.skip_padding,
    )?;

    Ok(())
}

use std::{
    fs::{self, read, read_to_string},
    mem::swap,
    path::PathBuf,
    str::FromStr,
};

use clap::{AppSettings, IntoApp, Parser};
use espflash::{
    cli::{
        board_info,
        clap::{ConnectOpts, FlashOpts},
        connect,
        monitor::monitor,
        save_elf_as_image,
    },
    Chip, Config, Error, ImageFormatId, PartitionTable,
};
use miette::{IntoDiagnostic, Result, WrapErr};

#[derive(Parser)]
#[clap(version, global_setting = AppSettings::PropagateVersion)]
struct Opts {
    /// Image format to flash
    #[clap(long)]
    pub format: Option<String>,
    #[clap(flatten)]
    flash_opts: FlashOpts,
    #[clap(flatten)]
    connect_opts: ConnectOpts,
    /// ELF image to flash
    image: Option<String>,
    #[clap(subcommand)]
    subcommand: Option<SubCommand>,
}

#[derive(Parser)]
pub enum SubCommand {
    BoardInfo(ConnectOpts),
    SaveImage(SaveImageOpts),
}

/// Save the image to disk instead of flashing to device
#[derive(Parser)]
pub struct SaveImageOpts {
    /// Image format to flash
    #[clap(long)]
    format: Option<String>,
    /// the chip to create an image for
    chip: Chip,
    /// ELF image to flash
    image: PathBuf,
    /// File name to save the generated image to
    file: PathBuf,
}

fn main() -> Result<()> {
    miette::set_panic_hook();

    let mut opts = Opts::parse();
    let config = Config::load()?;

    // If only a single argument is passed, it's always going to be the ELF file. In
    // the case that the serial port was not provided as a command-line argument,
    // we will either load the value specified in the configuration file or do port
    // auto-detection instead.
    if opts.image.is_none() && opts.connect_opts.serial.is_some() {
        swap(&mut opts.image, &mut opts.connect_opts.serial);
    }

    if let Some(subcommand) = opts.subcommand {
        use SubCommand::*;

        match subcommand {
            BoardInfo(matches) => board_info(matches, config),
            SaveImage(matches) => save_image(matches),
        }
    } else {
        flash(opts, config)
    }
}

fn flash(opts: Opts, config: Config) -> Result<()> {
    let ram = opts.flash_opts.ram;
    let bootloader_path = opts.flash_opts.bootloader;
    let partition_table_path = opts.flash_opts.partition_table;
    let image_format_string = opts.format;

    let elf = match opts.image {
        Some(elf) => elf,
        _ => {
            Opts::into_app().print_help().ok();
            return Ok(());
        }
    };

    let mut flasher = connect(&opts.connect_opts, &config)?;

    let input_bytes = read(&elf)
        .into_diagnostic()
        .wrap_err_with(|| format!("Failed to open elf image \"{}\"", elf))?;

    if ram {
        flasher.load_elf_to_ram(&input_bytes)?;
    } else {
        let bootloader = bootloader_path
            .as_deref()
            .map(read)
            .transpose()
            .into_diagnostic()
            .wrap_err_with(|| {
                format!(
                    "Failed to open bootloader image \"{}\"",
                    bootloader_path.unwrap().display()
                )
            })?;
        let image_format = image_format_string
            .as_deref()
            .map(ImageFormatId::from_str)
            .transpose()?;
        let partition_table = partition_table_path
            .as_deref()
            .map(|path| {
                let table = read_to_string(path)?;
                PartitionTable::try_from_str(&table).map_err(Error::from)
            })
            .transpose()
            .into_diagnostic()
            .wrap_err_with(|| {
                format!(
                    "Failed to load partition table \"{}\"",
                    partition_table_path.unwrap().display()
                )
            })?;
        flasher.load_elf_to_flash_with_format(
            &input_bytes,
            bootloader,
            partition_table,
            image_format,
        )?;
    }

    if opts.flash_opts.monitor {
        monitor(flasher.into_serial()).into_diagnostic()?;
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

    save_elf_as_image(opts.chip, &elf_data, opts.file, image_format)?;

    Ok(())
}

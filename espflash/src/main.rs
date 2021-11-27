use std::{
    fs::{self, read, read_to_string},
    mem::swap,
    str::FromStr,
};

use clap::{AppSettings, IntoApp, Parser};
use espflash::{
    cli::{clap::*, connect, monitor::monitor},
    Chip, Config, Error, FirmwareImage, ImageFormatId, PartitionTable,
};
use miette::{IntoDiagnostic, Result, WrapErr};

#[derive(Parser)]
#[clap(global_setting = AppSettings::PropagateVersion)]
#[clap(version = env!("CARGO_PKG_VERSION"))]
struct Opts {
    /// Image format to flash
    #[clap(long)]
    pub format: Option<String>,
    #[clap(flatten)]
    flash_args: FlashArgs,
    #[clap(flatten)]
    connect_args: ConnectArgs,
    /// ELF image to flash
    image: Option<String>,
    #[clap(subcommand)]
    sub_cmd: Option<SubCommand>,
}

#[derive(Parser)]
pub enum SubCommand {
    SaveImage(SaveImageOpts),
    BoardInfo(BoardInfoOpts),
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
    image: String,
    /// File name to save the generated image to
    file: String,
}

fn main() -> Result<()> {
    miette::set_panic_hook();

    let mut opts = Opts::parse();
    let config = Config::load()?;

    // If only a single argument is passed, it's always going to be the ELF file. In
    // the case that the serial port was not provided as a command-line argument,
    // we will either load the value specified in the configuration file or do port
    // auto-detection instead.
    if opts.image.is_none() && opts.connect_args.serial.is_some() {
        swap(&mut opts.image, &mut opts.connect_args.serial);
    }

    match opts.sub_cmd {
        Some(SubCommand::BoardInfo(opts)) => board_info(opts, config),
        Some(SubCommand::SaveImage(opts)) => save_image(opts, config),
        None => flash(opts, config),
    }
}

fn flash(opts: Opts, config: Config) -> Result<()> {
    if opts.flash_args.board_info {
        return board_info(
            BoardInfoOpts {
                connect_args: opts.connect_args,
            },
            config,
        );
    }
    let ram = opts.flash_args.ram;
    let bootloader_path = opts.flash_args.bootloader;
    let partition_table_path = opts.flash_args.partition_table;
    let image_format_string = opts.format;

    let elf = match opts.image {
        Some(elf) => elf,
        _ => {
            Opts::into_app().print_help().ok();
            return Ok(());
        }
    };

    let mut flasher = connect(&opts.connect_args, &config)?;

    let input_bytes = read(&elf)
        .into_diagnostic()
        .wrap_err_with(|| format!("Failed to open elf image \"{}\"", &elf))?;

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
                    bootloader_path.unwrap()
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
                    partition_table_path.unwrap()
                )
            })?;
        flasher.load_elf_to_flash_with_format(
            &input_bytes,
            bootloader,
            partition_table,
            image_format,
        )?;
    }

    if opts.flash_args.monitor {
        monitor(flasher.into_serial()).into_diagnostic()?;
    }

    Ok(())
}

fn save_image(opts: SaveImageOpts, _config: Config) -> Result<()> {
    let chip = opts.chip;
    let elf = opts.image;
    let elf_data = fs::read(&elf)
        .into_diagnostic()
        .wrap_err_with(|| format!("Failed to open image {}", elf))?;

    let image = FirmwareImage::from_data(&elf_data)?;

    let image_format = opts
        .format
        .as_deref()
        .map(ImageFormatId::from_str)
        .transpose()?;

    let flash_image = chip.get_flash_image(&image, None, None, image_format, None)?;
    let parts: Vec<_> = flash_image.ota_segments().collect();

    let out_path = opts.file;

    match parts.as_slice() {
        [single] => fs::write(out_path, &single.data).into_diagnostic()?,
        parts => {
            for part in parts {
                let part_path = format!("{:#x}_{}", part.addr, out_path);
                fs::write(part_path, &part.data).into_diagnostic()?
            }
        }
    }

    Ok(())
}

fn board_info(opts: BoardInfoOpts, config: Config) -> Result<()> {
    let mut flasher = connect(&opts.connect_args, &config)?;
    flasher.board_info()?;
    Ok(())
}

use std::fs::{read, read_to_string};

use clap::{AppSettings, Clap, IntoApp};
use espflash::cli::{clap::*, get_serial_port};
use espflash::{Chip, Config, Error, FirmwareImage, Flasher, ImageFormatId, PartitionTable};
use miette::{IntoDiagnostic, Result, WrapErr};
use serial::{BaudRate, FlowControl, SerialPort};
use std::fs;
use std::mem::swap;
use std::str::FromStr;

#[derive(Clap)]
#[clap(global_setting = AppSettings::ColoredHelp)]
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

#[derive(Clap)]
pub enum SubCommand {
    SaveImage(SaveImageOpts),
    BoardInfo(BoardInfoOpts),
}

/// Save the image to disk instead of flashing to device
#[derive(Clap)]
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
    let config = Config::load();

    // if only a single argument is passed, it's always the elf
    if opts.image.is_none() && config.connection.serial.is_some() {
        swap(&mut opts.image, &mut opts.connect_args.serial);
    }

    match opts.sub_cmd {
        Some(SubCommand::BoardInfo(opts)) => board_info(opts, config),
        Some(SubCommand::SaveImage(opts)) => save_image(opts, config),
        None => flash(opts, config),
    }
}

fn connect(matches: &ConnectArgs, config: &Config) -> Result<Flasher> {
    let port = get_serial_port(matches, config).ok_or(espflash::Error::NoSerial)?;

    // Attempt to open the serial port and set its initial baud rate.
    println!("Serial port: {}", port);
    println!("Connecting...\n");
    let mut serial = serial::open(&port)
        .map_err(espflash::Error::from)
        .wrap_err_with(|| format!("Failed to open serial port {}", port))?;
    serial
        .reconfigure(&|settings| {
            settings.set_flow_control(FlowControl::FlowNone);
            settings.set_baud_rate(BaudRate::Baud115200)?;
            Ok(())
        })
        .into_diagnostic()?;

    // Parse the baud rate if provided as as a command-line argument.
    let speed = if let Some(speed) = matches.speed {
        Some(BaudRate::from_speed(speed))
    } else {
        None
    };

    Ok(Flasher::connect(serial, speed)?)
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

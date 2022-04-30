use std::{fs, io::Write, mem::swap, path::PathBuf, str::FromStr};

use clap::{IntoApp, Parser};
use espflash::{
    cli::{
        board_info, connect, flash_elf_image, monitor::monitor, save_elf_as_image, ConnectOpts,
        FlashConfigOpts, FlashOpts,
    },
    Chip, Config, ImageFormatId, InvalidPartitionTable, PartitionTable,
};
use miette::{IntoDiagnostic, Result, WrapErr};

#[derive(Parser)]
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
}

#[derive(Parser)]
pub enum SubCommand {
    /// Display information about the connected board and exit without flashing
    BoardInfo(ConnectOpts),
    /// Save the image to disk instead of flashing to device
    SaveImage(SaveImageOpts),
    /// Operations for partitions tables
    PartitionTable(PartitionTableOpts),
}

#[derive(Parser)]
pub struct SaveImageOpts {
    #[clap(flatten)]
    pub flash_config_opts: FlashConfigOpts,
    /// Image format to flash
    #[clap(long, possible_values = &["bootloader", "direct-boot"])]
    format: Option<String>,
    /// the chip to create an image for
    chip: Chip,
    /// ELF image to flash
    image: PathBuf,
    /// File name to save the generated image to
    file: PathBuf,
    /// Boolean flag, if set, bootloader, partition table and application binaries will be merged into single binary
    #[clap(long, short = 'M')]
    pub merge: bool,
    /// Custom bootloader for merging
    #[clap(long, short = 'B')]
    pub bootloader: Option<PathBuf>,
    /// Custom partition table for merging
    #[clap(long, short = 'T')]
    pub partition_table: Option<PathBuf>,
}

#[derive(Parser)]
pub struct PartitionTableOpts {
    /// Convert CSV parition table to binary representation
    #[clap(long, required_unless_present_any = ["info", "to-csv"])]
    to_binary: bool,
    /// Convert binary partition table to CSV representation
    #[clap(long, required_unless_present_any = ["info", "to-binary"])]
    to_csv: bool,
    /// Show information on partition table
    #[clap(short, long, required_unless_present_any = ["to-binary", "to-csv"])]
    info: bool,
    /// Input partition table
    partition_table: PathBuf,
    /// Optional output file name, if unset will output to stdout
    #[clap(short, long)]
    output: Option<PathBuf>,
}

fn main() -> Result<()> {
    miette::set_panic_hook();

    let mut opts = Opts::parse();
    let config = Config::load()?;

    if !matches!(
        opts.subcommand,
        Some(SubCommand::BoardInfo(..) | SubCommand::PartitionTable(..)),
    ) {
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
            PartitionTable(opts) => partition_table(opts),
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
        )?;
    }

    if opts.flash_opts.monitor {
        let pid = flasher.get_usb_pid()?;
        monitor(flasher.into_serial(), &elf_data, pid).into_diagnostic()?;
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
    )?;

    Ok(())
}

fn partition_table(opts: PartitionTableOpts) -> Result<()> {
    if opts.to_binary {
        let input = fs::read(&opts.partition_table).into_diagnostic()?;
        let part_table = PartitionTable::try_from_str(String::from_utf8(input).into_diagnostic()?)
            .into_diagnostic()?;

        // Use either stdout or a file if provided for the output.
        let mut writer: Box<dyn Write> = if let Some(output) = opts.output {
            Box::new(fs::File::create(output).into_diagnostic()?)
        } else {
            Box::new(std::io::stdout())
        };
        part_table.save_bin(&mut writer).into_diagnostic()?;
    } else if opts.to_csv {
        let input = fs::read(&opts.partition_table).into_diagnostic()?;
        let part_table = PartitionTable::try_from_bytes(input).into_diagnostic()?;

        // Use either stdout or a file if provided for the output.
        let mut writer: Box<dyn Write> = if let Some(output) = opts.output {
            Box::new(fs::File::create(output).into_diagnostic()?)
        } else {
            Box::new(std::io::stdout())
        };
        part_table.save_csv(&mut writer).into_diagnostic()?;
    } else if opts.info {
        let input = fs::read(&opts.partition_table).into_diagnostic()?;

        // Try getting the partition table from either the csv or the binary representation and
        // fail otherwise.
        let part_table = if let Ok(part_table) =
            PartitionTable::try_from_bytes(input.clone()).into_diagnostic()
        {
            part_table
        } else if let Ok(part_table) =
            PartitionTable::try_from_str(String::from_utf8(input).into_diagnostic()?)
        {
            part_table
        } else {
            return Err((InvalidPartitionTable {}).into());
        };

        part_table.pretty_print();
    }

    Ok(())
}

use std::{
    fs,
    path::PathBuf,
    process::{exit, Command, ExitStatus, Stdio},
};

use cargo_metadata::Message;
use clap::{Args, CommandFactory, Parser, Subcommand};
use espflash::{
    cli::{
        self, board_info, checksum_md5, completions, config::Config, connect, erase_flash,
        erase_partitions, erase_region, flash_elf_image, monitor::monitor, parse_partition_table,
        partition_table, print_board_info, save_elf_as_image, serial_monitor, ChecksumMd5Args,
        CompletionsArgs, ConnectArgs, EraseFlashArgs, EraseRegionArgs, EspflashProgress,
        FlashConfigArgs, MonitorArgs, PartitionTableArgs,
    },
    error::Error as EspflashError,
    flasher::{FlashData, FlashSettings},
    image_format::ImageFormatKind,
    logging::initialize_logger,
    targets::{Chip, XtalFrequency},
    update::check_for_update,
};
use log::{debug, info, LevelFilter};
use miette::{IntoDiagnostic, Result, WrapErr};

use crate::{
    cargo_config::CargoConfig,
    error::{Error, NoTargetError, UnsupportedTargetError},
    package_metadata::PackageMetadata,
};

mod cargo_config;
mod error;
mod package_metadata;

#[derive(Debug, Parser)]
#[clap(
    bin_name = "cargo",
    max_term_width = 100,
    propagate_version = true,
    version
)]
struct Cli {
    #[clap(subcommand)]
    subcommand: CargoSubcommand,
}

#[derive(Debug, Subcommand)]
enum CargoSubcommand {
    #[clap(about)]
    Espflash {
        #[clap(subcommand)]
        subcommand: Commands,
    },
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
    /// Erase specified partitions
    EraseParts(ErasePartsArgs),
    /// Erase specified region
    EraseRegion(EraseRegionArgs),
    /// Flash an application in ELF format to a target device
    ///
    /// First convert the ELF file produced by cargo into the appropriate
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
    /// Calculate the MD5 checksum of the given region
    ChecksumMd5(ChecksumMd5Args),
}

#[derive(Debug, Args)]
#[non_exhaustive]
struct BuildArgs {
    /// Binary to build and flash
    #[arg(long)]
    pub bin: Option<String>,
    /// Example to build and flash
    #[arg(long)]
    pub example: Option<String>,
    /// Comma delimited list of build features
    #[arg(long, use_value_delimiter = true)]
    pub features: Option<Vec<String>>,
    /// Require Cargo.lock and cache are up to date
    #[arg(long)]
    pub frozen: bool,
    /// Require Cargo.lock is up to date
    #[arg(long)]
    pub locked: bool,
    /// Specify a (binary) package within a workspace to be built
    #[arg(long)]
    pub package: Option<String>,
    /// Build the application using the release profile
    #[arg(long)]
    pub release: bool,
    /// Target to build for
    #[arg(long)]
    pub target: Option<String>,
    /// Directory for all generated artifacts
    #[arg(long)]
    pub target_dir: Option<String>,
    /// Unstable (nightly-only) flags to Cargo, see 'cargo -Z help' for details
    #[arg(short = 'Z')]
    pub unstable: Option<Vec<String>>,

    #[clap(flatten)]
    pub flash_config_args: FlashConfigArgs,
}

/// Erase named partitions based on provided partition table
#[derive(Debug, Args)]
#[non_exhaustive]
pub struct ErasePartsArgs {
    /// Connection configuration
    #[clap(flatten)]
    pub connect_args: ConnectArgs,
    /// Labels of the partitions to be erased
    #[arg(value_name = "LABELS", value_delimiter = ',')]
    pub erase_parts: Vec<String>,
    /// Input partition table
    #[arg(long, value_name = "FILE")]
    pub partition_table: Option<PathBuf>,
    /// Specify a (binary) package within a workspace which may provide a partition table
    #[arg(long)]
    pub package: Option<String>,
}

/// Build and flash an application to a target device
#[derive(Debug, Args)]
#[non_exhaustive]
struct FlashArgs {
    #[clap(flatten)]
    build_args: BuildArgs,
    #[clap(flatten)]
    connect_args: ConnectArgs,
    #[clap(flatten)]
    flash_args: cli::FlashArgs,
}

#[derive(Debug, Args)]
#[non_exhaustive]
struct SaveImageArgs {
    /// Image format to flash
    #[arg(long, value_enum)]
    pub format: Option<ImageFormatKind>,
    #[clap(flatten)]
    build_args: BuildArgs,
    #[clap(flatten)]
    save_image_args: cli::SaveImageArgs,
}

fn main() -> Result<()> {
    miette::set_panic_hook();
    initialize_logger(LevelFilter::Info);

    // Attempt to parse any provided comand-line arguments, or print the help
    // message and terminate if the invocation is not correct.
    let CargoSubcommand::Espflash { subcommand: args } = Cli::parse().subcommand;
    debug!("{:#?}", args);

    // Only check for updates once the command-line arguments have been processed,
    // to avoid printing any update notifications when the help message is
    // displayed.
    check_for_update(env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"));

    // Load any user configuration, if present.
    let config = Config::load()?;

    // Execute the correct action based on the provided subcommand and its
    // associated arguments.
    match args {
        Commands::BoardInfo(args) => board_info(&args, &config),
        Commands::Completions(args) => completions(&args, &mut Cli::command(), "cargo"),
        Commands::EraseFlash(args) => erase_flash(args, &config),
        Commands::EraseParts(args) => erase_parts(args, &config),
        Commands::EraseRegion(args) => erase_region(args, &config),
        Commands::Flash(args) => flash(args, &config),
        Commands::Monitor(args) => serial_monitor(args, &config),
        Commands::PartitionTable(args) => partition_table(args),
        Commands::SaveImage(args) => save_image(args),
        Commands::ChecksumMd5(args) => checksum_md5(&args, &config),
    }
}

#[derive(Debug, Clone)]
struct BuildContext {
    pub artifact_path: PathBuf,
    pub bootloader_path: Option<PathBuf>,
    pub partition_table_path: Option<PathBuf>,
}

pub fn erase_parts(args: ErasePartsArgs, config: &Config) -> Result<()> {
    if args.connect_args.no_stub {
        return Err(EspflashError::StubRequiredToEraseFlash).into_diagnostic();
    }

    let metadata_partition_table = PackageMetadata::load(&args.package)
        .ok()
        .and_then(|m| m.partition_table);

    let partition_table = args
        .partition_table
        .as_deref()
        .or(metadata_partition_table.as_deref());

    let mut flash = connect(&args.connect_args, config, false, false)?;
    let partition_table = match partition_table {
        Some(path) => Some(parse_partition_table(path)?),
        None => None,
    };

    info!("Erasing the following partitions: {:?}", args.erase_parts);
    let chip: Chip = flash.chip();
    erase_partitions(&mut flash, partition_table, Some(args.erase_parts), None)?;
    flash
        .connection()
        .reset_after(!args.connect_args.no_stub, chip)?;

    Ok(())
}

fn flash(args: FlashArgs, config: &Config) -> Result<()> {
    let metadata = PackageMetadata::load(&args.build_args.package)?;
    let cargo_config = CargoConfig::load(&metadata.workspace_root, &metadata.package_root);

    let mut flasher = connect(
        &args.connect_args,
        config,
        args.flash_args.no_verify,
        args.flash_args.no_skip,
    )?;
    flasher.verify_minimum_revision(args.flash_args.min_chip_rev)?;

    // If the user has provided a flash size via a command-line argument, we'll
    // override the detected (or default) value with this.
    if let Some(flash_size) = args.build_args.flash_config_args.flash_size {
        flasher.set_flash_size(flash_size);
    }

    let chip = flasher.chip();
    let target = chip.into_target();
    let target_xtal_freq = target.crystal_freq(flasher.connection())?;

    flasher.disable_watchdog()?;

    let build_ctx =
        build(&args.build_args, &cargo_config, chip).wrap_err("Failed to build project")?;

    // Read the ELF data from the build path and load it to the target.
    let elf_data = fs::read(build_ctx.artifact_path).into_diagnostic()?;

    print_board_info(&mut flasher)?;

    if args.flash_args.ram {
        flasher.load_elf_to_ram(&elf_data, Some(&mut EspflashProgress::default()))?;
    } else {
        let bootloader = args
            .flash_args
            .bootloader
            .as_deref()
            .or(metadata.bootloader.as_deref())
            .or(build_ctx.bootloader_path.as_deref());

        let partition_table = args
            .flash_args
            .partition_table
            .as_deref()
            .or(metadata.partition_table.as_deref())
            .or(build_ctx.partition_table_path.as_deref());

        if let Some(path) = &bootloader {
            println!("Bootloader:        {}", path.display());
        }
        if let Some(path) = &partition_table {
            println!("Partition table:   {}", path.display());
        }

        let flash_settings = FlashSettings::new(
            args.build_args.flash_config_args.flash_mode,
            args.build_args.flash_config_args.flash_size,
            args.build_args.flash_config_args.flash_freq,
        );

        let flash_data = FlashData::new(
            bootloader,
            partition_table,
            args.flash_args.partition_table_offset,
            args.flash_args.format.or(metadata.format),
            args.flash_args.target_app_partition,
            flash_settings,
            args.flash_args.min_chip_rev,
        )?;

        if args.flash_args.erase_parts.is_some() || args.flash_args.erase_data_parts.is_some() {
            erase_partitions(
                &mut flasher,
                flash_data.partition_table.clone(),
                args.flash_args.erase_parts,
                args.flash_args.erase_data_parts,
            )?;
        }

        flash_elf_image(&mut flasher, &elf_data, flash_data, target_xtal_freq)?;
    }

    if args.flash_args.monitor {
        let pid = flasher.get_usb_pid()?;

        // The 26MHz ESP32-C2's need to be treated as a special case.
        let default_baud = if chip == Chip::Esp32c2
            && args.connect_args.no_stub
            && target_xtal_freq == XtalFrequency::_26Mhz
        {
            74_880
        } else {
            115_200
        };

        monitor(
            flasher.into_interface(),
            Some(&elf_data),
            pid,
            args.flash_args.monitor_baud.unwrap_or(default_baud),
            args.flash_args.log_format,
        )
    } else {
        Ok(())
    }
}

fn build(
    build_options: &BuildArgs,
    cargo_config: &CargoConfig,
    chip: Chip,
) -> Result<BuildContext> {
    let target = build_options
        .target
        .as_deref()
        .or_else(|| cargo_config.target())
        .ok_or_else(|| NoTargetError::new(Some(chip)))?;

    if !chip.into_target().supports_build_target(target) {
        return Err(UnsupportedTargetError::new(target, chip).into());
    }

    // The 'build-std' unstable cargo feature is required to enable
    // cross-compilation for Xtensa targets. If it has not been set then we
    // cannot build the application, and the cause of the (numerous) build errors
    // may not be immediately clear to the user.
    let cfg_has_build_std = cargo_config.has_build_std();
    let opts_has_build_std = build_options
        .unstable
        .clone()
        .map(|ref v| v.iter().any(|s| s.contains("build-std")))
        .unwrap_or_default();
    let xtensa_target = target.starts_with("xtensa-");

    if xtensa_target && !(cfg_has_build_std || opts_has_build_std) {
        return Err(Error::NoBuildStd.into());
    };

    // Build the list of arguments to pass to 'cargo build'. We will always
    // explicitly state the target, as it must be provided as either a command-line
    // argument or in the cargo config file.
    let mut args = vec!["--target".to_string(), target.to_string()];

    if let Some(target_dir) = &build_options.target_dir {
        args.push("--target-dir".to_string());
        args.push(target_dir.to_string());
    }

    if build_options.release {
        args.push("--release".to_string());
    }

    if build_options.locked {
        args.push("--locked".to_string());
    }

    if build_options.frozen {
        args.push("--frozen".to_string());
    }

    if let Some(example) = &build_options.example {
        args.push("--example".to_string());
        args.push(example.to_string());
    }

    if let Some(bin) = &build_options.bin {
        args.push("--bin".to_string());
        args.push(bin.to_string());
    }

    if let Some(package) = &build_options.package {
        args.push("--package".to_string());
        args.push(package.to_string());
    }

    if let Some(features) = &build_options.features {
        args.push("--features".to_string());
        args.push(features.join(","));
    }

    if let Some(unstable) = &build_options.unstable {
        for item in unstable.iter() {
            args.push("-Z".to_string());
            args.push(item.to_string());
        }
    }

    // Invoke the 'cargo build' command, passing our list of arguments.
    let output = Command::new("cargo")
        .arg("build")
        .args(args)
        .args(["--message-format", "json-diagnostic-rendered-ansi"])
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .into_diagnostic()?
        .wait_with_output()
        .into_diagnostic()?;

    // Parse build output.
    let messages = Message::parse_stream(&output.stdout[..]);

    // Find artifacts.
    let mut target_artifact = None;
    let mut bootloader_path = None;
    let mut partition_table_path = None;

    for message in messages {
        match message.into_diagnostic()? {
            Message::BuildScriptExecuted(script)
                if script.package_id.repr.starts_with("esp-idf-sys") =>
            {
                // If the `esp-idf-sys` package is being used, attempt to use the bootloader and
                // partition table compiled by `embuild` instead.
                let build_path = PathBuf::from(script.out_dir).join("build");

                let bl_path = build_path.join("bootloader").join("bootloader.bin");
                let pt_path = build_path
                    .join("partition_table")
                    .join("partition-table.bin");

                if bl_path.exists() && bl_path.is_file() {
                    bootloader_path = Some(bl_path);
                }

                if pt_path.exists() && pt_path.is_file() {
                    partition_table_path = Some(pt_path);
                }
            }
            Message::CompilerArtifact(artifact) => {
                if artifact.executable.is_some() {
                    if target_artifact.is_some() {
                        return Err(Error::MultipleArtifacts.into());
                    } else {
                        target_artifact = Some(artifact);
                    }
                }
            }
            Message::CompilerMessage(message) => {
                if let Some(rendered) = message.message.rendered {
                    print!("{}", rendered);
                }
            }
            // Ignore all other messages.
            _ => (),
        }
    }

    // Check if the command succeeded, otherwise return an error. Any error messages
    // occurring during the build are shown above, when the compiler messages are
    // rendered.
    if !output.status.success() {
        exit_with_process_status(output.status);
    }

    // If no target artifact was found, we don't have a path to return.
    let target_artifact = target_artifact.ok_or(Error::NoArtifact)?;
    let artifact_path = target_artifact.executable.unwrap().into();

    let build_ctx = BuildContext {
        artifact_path,
        bootloader_path,
        partition_table_path,
    };

    Ok(build_ctx)
}

fn save_image(args: SaveImageArgs) -> Result<()> {
    let metadata = PackageMetadata::load(&args.build_args.package)?;
    let cargo_config = CargoConfig::load(&metadata.workspace_root, &metadata.package_root);

    let build_ctx = build(&args.build_args, &cargo_config, args.save_image_args.chip)?;
    let elf_data = fs::read(build_ctx.artifact_path).into_diagnostic()?;

    let bootloader = args
        .save_image_args
        .bootloader
        .as_deref()
        .or(metadata.bootloader.as_deref())
        .or(build_ctx.bootloader_path.as_deref())
        .map(|p| p.to_path_buf());

    let partition_table = args
        .save_image_args
        .partition_table
        .as_deref()
        .or(metadata.partition_table.as_deref())
        .or(build_ctx.partition_table_path.as_deref())
        .map(|p| p.to_path_buf());

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

    let flash_settings = FlashSettings::new(
        args.build_args.flash_config_args.flash_mode,
        args.build_args.flash_config_args.flash_size,
        args.build_args.flash_config_args.flash_freq,
    );

    let flash_data = FlashData::new(
        bootloader.as_deref(),
        partition_table.as_deref(),
        args.save_image_args.partition_table_offset,
        args.format.or(metadata.format),
        args.save_image_args.target_app_partition,
        flash_settings,
        args.save_image_args.min_chip_rev,
    )?;

    let xtal_freq = args
        .save_image_args
        .xtal_freq
        .unwrap_or(XtalFrequency::default(args.save_image_args.chip));

    save_elf_as_image(
        &elf_data,
        args.save_image_args.chip,
        args.save_image_args.file,
        flash_data,
        args.save_image_args.merge,
        args.save_image_args.skip_padding,
        xtal_freq,
    )?;

    Ok(())
}

#[cfg(unix)]
fn exit_with_process_status(status: ExitStatus) -> ! {
    use std::os::unix::process::ExitStatusExt;
    let code = status.code().or_else(|| status.signal()).unwrap_or(1);

    exit(code)
}

#[cfg(not(unix))]
fn exit_with_process_status(status: ExitStatus) -> ! {
    let code = status.code().unwrap_or(1);

    exit(code)
}

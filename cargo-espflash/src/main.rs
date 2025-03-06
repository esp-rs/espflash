use std::{
    fs,
    path::PathBuf,
    process::{exit, Command, ExitStatus, Stdio},
};

use cargo_metadata::{Message, MetadataCommand};
use clap::{Args, CommandFactory, Parser, Subcommand};
use espflash::{
    cli::{self, config::Config, monitor::monitor, *},
    flasher::FlashSize,
    logging::initialize_logger,
    targets::{Chip, XtalFrequency},
    update::check_for_update,
    Error as EspflashError,
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

        /// Do not check for updates
        #[clap(short = 'S', long, global = true, action)]
        skip_update_check: bool,
    },
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Print information about a connected target device
    ///
    /// Automatically detects and prints the chip type, crystal frequency, flash
    /// size, chip features, and MAC address of a connected target device.
    BoardInfo(ConnectArgs),
    /// Calculate the MD5 checksum of the given region
    ChecksumMd5(ChecksumMd5Args),
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
    /// Hold the target device in reset
    HoldInReset(ConnectArgs),
    /// List serial ports available.
    ///
    /// The default behavior is to only list ports of devices known to be used
    /// on development boards.
    ListPorts(ListPortsArgs),
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
    /// Read SPI flash content
    ReadFlash(ReadFlashArgs),
    /// Reset the target device
    Reset(ConnectArgs),
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
    /// Do not activate the `default` feature
    #[arg(long)]
    pub no_default_features: bool,
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
    /// Specify a (binary) package within a workspace which may provide a
    /// partition table
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
    let cli = Cli::parse();
    let CargoSubcommand::Espflash {
        subcommand: args,
        skip_update_check,
    } = cli.subcommand;
    debug!("{:#?}, {:#?}", args, skip_update_check);

    // Only check for updates once the command-line arguments have been processed,
    // to avoid printing any update notifications when the help message is
    // displayed.
    if !skip_update_check {
        check_for_update(env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"));
    }

    // Load any user configuration, if present.
    let config = Config::load()?;

    // Execute the correct action based on the provided subcommand and its
    // associated arguments.
    match args {
        Commands::BoardInfo(args) => board_info(&args, &config),
        Commands::ChecksumMd5(args) => checksum_md5(&args, &config),
        Commands::Completions(args) => completions(&args, &mut Cli::command(), "cargo"),
        Commands::EraseFlash(args) => erase_flash(args, &config),
        Commands::EraseParts(args) => erase_parts(args, &config),
        Commands::EraseRegion(args) => erase_region(args, &config),
        Commands::Flash(args) => flash(args, &config),
        Commands::HoldInReset(args) => hold_in_reset(args, &config),
        Commands::ListPorts(args) => list_ports(&args, &config),
        Commands::Monitor(args) => serial_monitor(args, &config),
        Commands::PartitionTable(args) => partition_table(args),
        Commands::ReadFlash(args) => read_flash(args, &config),
        Commands::Reset(args) => reset(args, &config),
        Commands::SaveImage(args) => save_image(args, &config),
        Commands::WriteBin(args) => write_bin(args, &config),
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
        return Err(EspflashError::StubRequired).into_diagnostic();
    }

    let partition_table = args
        .partition_table
        .as_deref()
        .or(config.partition_table.as_deref());

    let mut flasher = connect(&args.connect_args, config, false, false)?;
    let partition_table = match partition_table {
        Some(path) => Some(parse_partition_table(path)?),
        None => None,
    };

    info!("Erasing the following partitions: {:?}", args.erase_parts);

    erase_partitions(&mut flasher, partition_table, Some(args.erase_parts), None)?;
    flasher
        .connection()
        .reset_after(!args.connect_args.no_stub)?;

    Ok(())
}

fn reset(args: ConnectArgs, config: &Config) -> Result<()> {
    let mut args = args.clone();
    args.no_stub = true;
    let mut flash = connect(&args, config, true, true)?;
    info!("Resetting target device");
    flash.connection().reset()?;

    Ok(())
}

fn hold_in_reset(args: ConnectArgs, config: &Config) -> Result<()> {
    connect(&args, config, true, true)?;
    info!("Holding target device in reset");

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
    flasher.verify_minimum_revision(args.flash_args.image.min_chip_rev)?;

    // If the user has provided a flash size via a command-line argument or config,
    // we'll override the detected (or default) value with this.
    if let Some(flash_size) = args.build_args.flash_config_args.flash_size {
        flasher.set_flash_size(flash_size);
    } else if let Some(flash_size) = config.flash.size {
        flasher.set_flash_size(flash_size);
    }

    let chip = flasher.chip();
    let target = chip.into_target();
    let target_xtal_freq = target.crystal_freq(flasher.connection())?;

    flasher.disable_watchdog()?;

    let build_ctx =
        build(&args.build_args, &cargo_config, chip).wrap_err("Failed to build project")?;

    // Read the ELF data from the build path and load it to the target.
    let elf_data = fs::read(build_ctx.artifact_path.clone()).into_diagnostic()?;

    print_board_info(&mut flasher)?;

    let mut flash_config = args.build_args.flash_config_args;
    flash_config.flash_size = flash_config
        .flash_size // Use CLI argument if provided
        .or(config.flash.size) // If no CLI argument, try the config file
        .or_else(|| flasher.flash_detect().ok().flatten()) // Try detecting flash size next
        .or_else(|| Some(FlashSize::default())); // Otherwise, use a reasonable default value

    if args.flash_args.ram {
        flasher.load_elf_to_ram(&elf_data, Some(&mut EspflashProgress::default()))?;
    } else {
        let flash_data = make_flash_data(
            args.flash_args.image,
            &flash_config,
            config,
            build_ctx.bootloader_path.as_deref(),
            build_ctx.partition_table_path.as_deref(),
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
        let pid = flasher.usb_pid();
        let mut monitor_args = args.flash_args.monitor_args;

        // The 26MHz ESP32-C2's need to be treated as a special case.
        if chip == Chip::Esp32c2
            && target_xtal_freq == XtalFrequency::_26Mhz
            && monitor_args.monitor_baud == 115_200
        {
            // 115_200 * 26 MHz / 40 MHz = 74_880
            monitor_args.monitor_baud = 74_880;
        }

        monitor_args.elf = Some(build_ctx.artifact_path);

        monitor(flasher.into_serial(), Some(&elf_data), pid, monitor_args)
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

    let mut metadata_cmd = MetadataCommand::new();
    if build_options.no_default_features {
        metadata_cmd.features(cargo_metadata::CargoOpt::NoDefaultFeatures);
    }
    if let Some(features) = &build_options.features {
        metadata_cmd.features(cargo_metadata::CargoOpt::SomeFeatures(features.clone()));
    }
    let metadata = metadata_cmd.exec().into_diagnostic()?;

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

    if build_options.no_default_features {
        args.push("--no-default-features".to_string());
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
            Message::BuildScriptExecuted(script) => {
                // We can't use the `Index` implementation on `Metadata` because `-Zbuild-std`
                // pulls in dependencies not listed in the metadata which then causes the
                // `Index` implementation to panic.
                let Some(package) = metadata.packages.iter().find(|p| p.id == script.package_id)
                else {
                    continue;
                };

                if package.name != "esp-idf-sys" {
                    continue;
                }

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

fn save_image(args: SaveImageArgs, config: &Config) -> Result<()> {
    let metadata = PackageMetadata::load(&args.build_args.package)?;
    let cargo_config = CargoConfig::load(&metadata.workspace_root, &metadata.package_root);

    let build_ctx = build(&args.build_args, &cargo_config, args.save_image_args.chip)?;
    let elf_data = fs::read(build_ctx.artifact_path).into_diagnostic()?;

    // Since we have no `Flasher` instance and as such cannot print the board
    // information, we will print whatever information we _do_ have.
    println!("Chip type:         {}", args.save_image_args.chip);
    println!("Merge:             {}", args.save_image_args.merge);
    println!("Skip padding:      {}", args.save_image_args.skip_padding);

    let mut flash_config = args.build_args.flash_config_args;
    flash_config.flash_size = flash_config
        .flash_size // Use CLI argument if provided
        .or(config.flash.size) // If no CLI argument, try the config file
        .or_else(|| Some(FlashSize::default())); // Otherwise, use a reasonable default value

    let flash_data = make_flash_data(
        args.save_image_args.image,
        &flash_config,
        config,
        build_ctx.bootloader_path.as_deref(),
        build_ctx.partition_table_path.as_deref(),
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

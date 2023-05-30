use std::{
    fs,
    path::PathBuf,
    process::{exit, Command, ExitStatus, Stdio},
};

use cargo_metadata::Message;
use clap::{Args, CommandFactory, Parser, Subcommand};
use espflash::{
    cli::{
        self, board_info, completions, config::Config, connect, erase_partitions, flash_elf_image,
        monitor::monitor, parse_partition_table, partition_table, print_board_info,
        save_elf_as_image, serial_monitor, CompletionsArgs, ConnectArgs, EspflashProgress,
        FlashConfigArgs, MonitorArgs, PartitionTableArgs,
    },
    image_format::ImageFormatKind,
    logging::initialize_logger,
    targets::Chip,
    update::check_for_update,
};
use log::{debug, LevelFilter};
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
#[clap(version, propagate_version = true)]
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
    BoardInfo(ConnectArgs),
    Completions(CompletionsArgs),
    Flash(FlashArgs),
    Monitor(MonitorArgs),
    PartitionTable(PartitionTableArgs),
    SaveImage(SaveImageArgs),
}

#[derive(Debug, Args)]
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

/// Build and flash an application to a target device
#[derive(Debug, Args)]
struct FlashArgs {
    #[clap(flatten)]
    build_args: BuildArgs,
    #[clap(flatten)]
    connect_args: ConnectArgs,
    #[clap(flatten)]
    flash_args: cli::FlashArgs,
}

#[derive(Debug, Args)]
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
        Commands::Flash(args) => flash(args, &config),
        Commands::Monitor(args) => serial_monitor(args, &config),
        Commands::PartitionTable(args) => partition_table(args),
        Commands::SaveImage(args) => save_image(args),
    }
}

#[derive(Debug, Clone)]
struct BuildContext {
    pub artifact_path: PathBuf,
    pub bootloader_path: Option<PathBuf>,
    pub partition_table_path: Option<PathBuf>,
}

fn flash(args: FlashArgs, config: &Config) -> Result<()> {
    let metadata = PackageMetadata::load(&args.build_args.package)?;
    let cargo_config = CargoConfig::load(&metadata.workspace_root, &metadata.package_root);

    let mut flasher = connect(&args.connect_args, config)?;

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
            args.flash_args.format.or(metadata.format),
            args.build_args.flash_config_args.flash_mode,
            args.build_args.flash_config_args.flash_size,
            args.build_args.flash_config_args.flash_freq,
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

    save_elf_as_image(
        args.save_image_args.chip,
        &elf_data,
        args.save_image_args.file,
        args.format.or(metadata.format),
        args.build_args.flash_config_args.flash_mode,
        args.build_args.flash_config_args.flash_size,
        args.build_args.flash_config_args.flash_freq,
        args.save_image_args.merge,
        bootloader,
        partition_table,
        args.save_image_args.skip_padding,
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

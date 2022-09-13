use std::{
    fs,
    path::PathBuf,
    process::{exit, Command, ExitStatus, Stdio},
    str::FromStr,
};

use cargo_metadata::Message;
use clap::{Args, Parser, Subcommand};
use espflash::{
    cli::{
        board_info, connect, flash_elf_image, monitor::monitor, partition_table, save_elf_as_image,
        serial_monitor, ConnectArgs, FlashArgs as BaseFlashArgs, FlashConfigArgs,
        PartitionTableArgs, SaveImageArgs as BaseSaveImageArgs,
    },
    image_format::ImageFormatType,
    logging::initialize_logger,
    update::check_for_update,
    Chip, Config, ImageFormatId,
};
use log::{debug, LevelFilter};
use miette::{IntoDiagnostic, Result, WrapErr};
use strum::VariantNames;

use crate::{
    cargo_config::{parse_cargo_config, CargoConfig},
    error::{Error, NoTargetError, UnsupportedTargetError},
    package_metadata::CargoEspFlashMeta,
};

mod cargo_config;
mod error;
mod package_metadata;

#[derive(Debug, Parser)]
#[clap(bin_name = "cargo", propagate_version = true, version)]
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
    /// Display information about the connected board and exit without flashing
    BoardInfo(ConnectArgs),
    /// Flash an application to a target device
    Flash(FlashArgs),
    /// Open the serial monitor without flashing
    Monitor(ConnectArgs),
    PartitionTable(PartitionTableArgs),
    SaveImage(SaveImageArgs),
}

#[derive(Debug, Args)]
struct BuildArgs {
    /// Binary to build and flash
    #[clap(long)]
    pub bin: Option<String>,
    /// Example to build and flash
    #[clap(long)]
    pub example: Option<String>,
    /// Comma delimited list of build features
    #[clap(long, use_value_delimiter = true)]
    pub features: Option<Vec<String>>,
    /// Image format to flash
    #[clap(long, possible_values = ImageFormatType::VARIANTS)]
    pub format: Option<String>,
    /// Require Cargo.lock and cache are up to date
    #[clap(long)]
    pub frozen: bool,
    /// Require Cargo.lock is up to date
    #[clap(long)]
    pub locked: bool,
    /// Specify a (binary) package within a workspace to be built
    #[clap(long)]
    pub package: Option<String>,
    /// Build the application using the release profile
    #[clap(long)]
    pub release: bool,
    /// Target to build for
    #[clap(long)]
    pub target: Option<String>,
    /// Directory for all generated artifacts
    #[clap(long)]
    pub target_dir: Option<String>,
    /// Unstable (nightly-only) flags to Cargo, see 'cargo -Z help' for details
    #[clap(short = 'Z')]
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
    flash_args: BaseFlashArgs,
}

#[derive(Debug, Args)]
struct SaveImageArgs {
    #[clap(flatten)]
    build_args: BuildArgs,
    #[clap(flatten)]
    save_image_args: BaseSaveImageArgs,
}

fn main() -> Result<()> {
    miette::set_panic_hook();
    initialize_logger(LevelFilter::Debug);

    // Attempt to parse any provided comand-line arguments, or print the help
    // message and terminate if the invocation is not correct.
    let CargoSubcommand::Espflash { subcommand: args } = Cli::parse().subcommand;
    debug!("{:#?}", args);

    // Only check for updates once the command-line arguments have been processed,
    // to avoid printing any update notifications when the help message is
    // displayed.
    check_for_update(env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"));

    // Load any user configuraiton and/or package metadata, if present.
    let config = Config::load().unwrap();
    let cargo_config = parse_cargo_config(".")?;
    let metadata = CargoEspFlashMeta::load("Cargo.toml")?;

    // Execute the correct action based on the provided subcommand and its
    // associated arguments.
    match args {
        Commands::BoardInfo(args) => board_info(args, &config),
        Commands::Flash(args) => flash(args, &config, &cargo_config, &metadata),
        Commands::Monitor(args) => serial_monitor(args, &config),
        Commands::PartitionTable(args) => partition_table(args),
        Commands::SaveImage(args) => save_image(args, &cargo_config, &metadata),
    }
}

#[derive(Debug, Clone)]
struct BuildContext {
    pub artifact_path: PathBuf,
    pub bootloader_path: Option<PathBuf>,
    pub partition_table_path: Option<PathBuf>,
}

fn flash(
    mut args: FlashArgs,
    config: &Config,
    cargo_config: &CargoConfig,
    metadata: &CargoEspFlashMeta,
) -> Result<()> {
    // The `erase_otadata` argument requires `use_stub`, which is implicitly
    // enabled here.
    if args.flash_args.erase_otadata {
        args.connect_args.use_stub = true;
    }

    let mut flasher = connect(&args.connect_args, config)?;

    let build_ctx = build(&args.build_args, cargo_config, flasher.chip())
        .wrap_err("Failed to build project")?;

    // Print the board information once the project has successfully built. We do
    // here rather than upon connection to show the Cargo output prior to the board
    // information, rather than breaking up cargo-espflash's output.
    flasher.board_info()?;

    // Read the ELF data from the build path and load it to the target.
    let elf_data = fs::read(build_ctx.artifact_path).into_diagnostic()?;

    if args.flash_args.ram {
        flasher.load_elf_to_ram(&elf_data)?;
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

        let image_format = args
            .build_args
            .format
            .as_deref()
            .map(ImageFormatId::from_str)
            .transpose()?
            .or(metadata.format);

        flash_elf_image(
            &mut flasher,
            &elf_data,
            bootloader,
            partition_table,
            image_format,
            args.build_args.flash_config_args.flash_mode,
            args.build_args.flash_config_args.flash_size,
            args.build_args.flash_config_args.flash_freq,
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

    if !chip.supports_target(target) {
        return Err(Error::UnsupportedTarget(UnsupportedTargetError::new(target, chip)).into());
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
        .args(&["--message-format", "json-diagnostic-rendered-ansi"])
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

fn save_image(
    args: SaveImageArgs,
    cargo_config: &CargoConfig,
    metadata: &CargoEspFlashMeta,
) -> Result<()> {
    let build_ctx = build(&args.build_args, cargo_config, args.save_image_args.chip)?;
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

    let image_format = args
        .build_args
        .format
        .as_deref()
        .map(ImageFormatId::from_str)
        .transpose()?
        .or(metadata.format);

    save_elf_as_image(
        args.save_image_args.chip,
        &elf_data,
        args.save_image_args.file,
        image_format,
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

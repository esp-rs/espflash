use std::{
    fs,
    path::PathBuf,
    process::{exit, Command, ExitStatus, Stdio},
    str::FromStr,
};

use cargo_metadata::Message;
use clap::Parser;
use espflash::{
    cli::{
        board_info, connect, flash_elf_image, monitor::monitor, partition_table, save_elf_as_image,
        serial_monitor, ConnectOpts, FlashConfigOpts, FlashOpts, PartitionTableOpts,
    },
    Chip, Config, ImageFormatId,
};
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

#[derive(Parser)]
#[clap(bin_name = "cargo", version, propagate_version = true)]
struct Opts {
    #[clap(subcommand)]
    subcommand: CargoSubCommand,
}

#[derive(Parser)]
enum CargoSubCommand {
    Espflash(EspFlashOpts),
}

#[derive(Parser)]
struct EspFlashOpts {
    #[clap(flatten)]
    flash_opts: FlashOpts,
    #[clap(flatten)]
    build_opts: BuildOpts,
    #[clap(flatten)]
    connect_opts: ConnectOpts,
    #[clap(subcommand)]
    subcommand: Option<SubCommand>,
}

#[derive(Parser)]
pub enum SubCommand {
    /// Display information about the connected board and exit without flashing
    BoardInfo(ConnectOpts),
    /// Save the image to disk instead of flashing to device
    SaveImage(SaveImageOpts),
    /// Open the serial monitor without flashing
    SerialMonitor(ConnectOpts),
    /// Operations for partitions tables
    PartitionTable(PartitionTableOpts),
}

#[derive(Parser)]
pub struct BuildOpts {
    /// Build the application using the release profile
    #[clap(long)]
    pub release: bool,
    /// Require Cargo.lock is up to date
    #[clap(long)]
    pub locked: bool,
    /// Require Cargo.lock and cache are up to date
    #[clap(long)]
    pub frozen: bool,
    /// Example to build and flash
    #[clap(long)]
    pub example: Option<String>,
    /// Specify a (binary) package within a workspace to be built
    #[clap(long)]
    pub package: Option<String>,
    /// Comma delimited list of build features
    #[clap(long, use_value_delimiter = true)]
    pub features: Option<Vec<String>>,
    /// Image format to flash
    #[clap(long, possible_values = &["bootloader", "direct-boot"])]
    pub format: Option<String>,
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
    pub flash_config_opts: FlashConfigOpts,
}

#[derive(Parser)]
pub struct SaveImageOpts {
    #[clap(flatten)]
    pub build_opts: BuildOpts,
    /// Chip to create an image for
    #[clap(possible_values = Chip::VARIANTS)]
    pub chip: Chip,
    /// File name to save the generated image to
    pub file: PathBuf,
    /// Boolean flag to merge binaries into single binary
    #[clap(long, short = 'M')]
    pub merge: bool,
    /// Custom bootloader for merging
    #[clap(long, short = 'B')]
    pub bootloader: Option<PathBuf>,
    /// Custom partition table for merging
    #[clap(long, short = 'T')]
    pub partition_table: Option<PathBuf>,
}

fn main() -> Result<()> {
    miette::set_panic_hook();

    let CargoSubCommand::Espflash(opts) = Opts::parse().subcommand;

    let config = Config::load()?;
    let metadata = CargoEspFlashMeta::load("Cargo.toml")?;
    let cargo_config = parse_cargo_config(".")?;

    if let Some(subcommand) = opts.subcommand {
        use SubCommand::*;

        match subcommand {
            BoardInfo(opts) => board_info(opts, config),
            SaveImage(opts) => save_image(opts, metadata, cargo_config),
            SerialMonitor(opts) => serial_monitor(opts, config),
            PartitionTable(opts) => partition_table(opts),
        }
    } else {
        flash(opts, config, metadata, cargo_config)
    }
}

#[derive(Debug, Clone)]
struct BuildContext {
    pub artifact_path: PathBuf,
    pub bootloader_path: Option<PathBuf>,
    pub partition_table_path: Option<PathBuf>,
}

fn flash(
    opts: EspFlashOpts,
    config: Config,
    metadata: CargoEspFlashMeta,
    cargo_config: CargoConfig,
) -> Result<()> {
    let mut flasher = connect(&opts.connect_opts, &config)?;

    let build_ctx = build(&opts.build_opts, &cargo_config, flasher.chip())
        .wrap_err("Failed to build project")?;

    // Print the board information once the project has successfully built. We do
    // here rather than upon connection to show the Cargo output prior to the board
    // information, rather than breaking up cargo-espflash's output.
    flasher.board_info()?;

    // Read the ELF data from the build path and load it to the target.
    let elf_data = fs::read(build_ctx.artifact_path).into_diagnostic()?;

    if opts.flash_opts.ram {
        flasher.load_elf_to_ram(&elf_data)?;
    } else {
        let bootloader = opts
            .flash_opts
            .bootloader
            .as_deref()
            .or(metadata.bootloader.as_deref())
            .or(build_ctx.bootloader_path.as_deref());

        let partition_table = opts
            .flash_opts
            .partition_table
            .as_deref()
            .or(metadata.partition_table.as_deref())
            .or(build_ctx.partition_table_path.as_deref());

        let image_format = opts
            .build_opts
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
            opts.build_opts.flash_config_opts.flash_mode,
            opts.build_opts.flash_config_opts.flash_size,
            opts.build_opts.flash_config_opts.flash_freq,
        )?;
    }

    if opts.flash_opts.monitor {
        let pid = flasher.get_usb_pid()?;
        monitor(flasher.into_serial(), Some(&elf_data), pid).into_diagnostic()?;
    }

    Ok(())
}

fn build(
    build_options: &BuildOpts,
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
    // cross-compilation for xtensa targets.
    // If it has not been set then we cannot build the
    // application.
    if !cargo_config.has_build_std() && target.starts_with("xtensa-") {
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
    opts: SaveImageOpts,
    metadata: CargoEspFlashMeta,
    cargo_config: CargoConfig,
) -> Result<()> {
    let build_ctx = build(&opts.build_opts, &cargo_config, opts.chip)?;
    let elf_data = fs::read(build_ctx.artifact_path).into_diagnostic()?;

    let bootloader = opts
        .bootloader
        .as_deref()
        .or(metadata.bootloader.as_deref())
        .or(build_ctx.bootloader_path.as_deref())
        .map(|p| p.to_path_buf());

    let partition_table = opts
        .partition_table
        .as_deref()
        .or(metadata.partition_table.as_deref())
        .or(build_ctx.partition_table_path.as_deref())
        .map(|p| p.to_path_buf());

    let image_format = opts
        .build_opts
        .format
        .as_deref()
        .map(ImageFormatId::from_str)
        .transpose()?
        .or(metadata.format);

    save_elf_as_image(
        opts.chip,
        &elf_data,
        opts.file,
        image_format,
        opts.build_opts.flash_config_opts.flash_mode,
        opts.build_opts.flash_config_opts.flash_size,
        opts.build_opts.flash_config_opts.flash_freq,
        opts.merge,
        bootloader,
        partition_table,
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

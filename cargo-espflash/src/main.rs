use std::{
    fs,
    path::PathBuf,
    process::{exit, Command, ExitStatus, Stdio},
    str::FromStr,
};

use cargo_metadata::Message;
use clap::{AppSettings, Parser};
use espflash::{
    cli::{clap::*, connect, monitor::monitor},
    Chip, Config, FirmwareImage, ImageFormatId, PartitionTable,
};
use miette::{IntoDiagnostic, Result, WrapErr};

use crate::{
    cargo_config::{parse_cargo_config, CargoConfig},
    error::{Error, NoTargetError, UnsupportedTargetError},
    package_metadata::CargoEspFlashMeta,
};

mod cargo_config;
mod error;
mod package_metadata;

#[derive(Parser)]
#[clap(global_setting = AppSettings::PropagateVersion)]
#[clap(bin_name = "cargo")]
#[clap(version = env!("CARGO_PKG_VERSION"))]
struct Opts {
    #[clap(subcommand)]
    sub_cmd: CargoSubCommand,
}

#[derive(Parser)]
enum CargoSubCommand {
    Espflash(EspFlashOpts),
}

#[derive(Parser)]
struct EspFlashOpts {
    #[clap(flatten)]
    flash_args: FlashArgs,
    #[clap(flatten)]
    build_args: BuildArgs,
    #[clap(flatten)]
    connect_args: ConnectArgs,
    #[clap(subcommand)]
    sub_cmd: Option<SubCommand>,
}

#[derive(Parser)]
pub enum SubCommand {
    SaveImage(SaveImageOpts),
    BoardInfo(BoardInfoOpts),
}

fn main() -> Result<()> {
    miette::set_panic_hook();

    let CargoSubCommand::Espflash(opts) = Opts::parse().sub_cmd;

    let config = Config::load()?;
    let metadata = CargoEspFlashMeta::load("Cargo.toml")?;
    let cargo_config = parse_cargo_config(".")?;

    match opts.sub_cmd {
        Some(SubCommand::BoardInfo(matches)) => board_info(matches, config, metadata, cargo_config),
        Some(SubCommand::SaveImage(matches)) => save_image(matches, config, metadata, cargo_config),
        None => flash(opts, config, metadata, cargo_config),
    }
}

fn flash(
    matches: EspFlashOpts,
    config: Config,
    metadata: CargoEspFlashMeta,
    cargo_config: CargoConfig,
) -> Result<()> {
    // Connect the Flasher to the target device and print the board information
    // upon connection. If the '--board-info' flag has been provided, we have
    // nothing left to do so exit early.
    let mut flasher = connect(&matches.connect_args, &config)?;
    flasher.board_info()?;

    if matches.flash_args.board_info {
        return Ok(());
    }

    let path = build(&matches.build_args, &cargo_config, Some(flasher.chip()))
        .wrap_err("Failed to build project")?;

    // If the '--bootloader' option is provided, load the binary file at the
    // specified path.
    let bootloader = if let Some(path) = matches
        .flash_args
        .bootloader
        .as_deref()
        .or_else(|| metadata.bootloader.as_deref())
    {
        let path = fs::canonicalize(path).into_diagnostic()?;
        let data = fs::read(path).into_diagnostic()?;
        Some(data)
    } else {
        None
    };

    // If the '--partition-table' option is provided, load the partition table from
    // the CSV at the specified path.
    let partition_table = if let Some(path) = matches
        .flash_args
        .partition_table
        .as_deref()
        .or_else(|| metadata.partition_table.as_deref())
    {
        let path = fs::canonicalize(path).into_diagnostic()?;
        let data = fs::read_to_string(path)
            .into_diagnostic()
            .wrap_err("Failed to open partition table")?;
        let table =
            PartitionTable::try_from_str(data).wrap_err("Failed to parse partition table")?;
        Some(table)
    } else {
        None
    };

    let image_format = matches
        .build_args
        .format
        .as_deref()
        .map(ImageFormatId::from_str)
        .transpose()?
        .or(metadata.format);

    // Read the ELF data from the build path and load it to the target.
    let elf_data = fs::read(path).into_diagnostic()?;
    if matches.flash_args.ram {
        flasher.load_elf_to_ram(&elf_data)?;
    } else {
        flasher.load_elf_to_flash_with_format(
            &elf_data,
            bootloader,
            partition_table,
            image_format,
        )?;
    }
    println!("\nFlashing has completed!");

    if matches.flash_args.monitor {
        monitor(flasher.into_serial()).into_diagnostic()?;
    }

    // We're all done!
    Ok(())
}

fn build(
    build_options: &BuildArgs,
    cargo_config: &CargoConfig,
    chip: Option<Chip>,
) -> Result<PathBuf> {
    let target = build_options
        .target
        .as_deref()
        .or_else(|| cargo_config.target())
        .ok_or_else(|| NoTargetError::new(chip))?;

    let chip = if chip.is_some() {
        chip
    } else {
        Chip::from_target(target)
    };

    if let Some(chip) = chip {
        if !chip.supports_target(target) {
            return Err(Error::UnsupportedTarget(UnsupportedTargetError::new(target, chip)).into());
        }
    } else {
        return Err(Error::UnknownTarget(target.to_string()).into());
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
    let mut args = vec!["--target", target];

    if build_options.release {
        args.push("--release");
    }

    if let Some(example) = build_options.example.as_deref() {
        args.push("--example");
        args.push(example);
    }

    if let Some(package) = build_options.package.as_deref() {
        args.push("--package");
        args.push(package);
    }

    if let Some(features) = build_options.features.as_deref() {
        args.push("--features");
        args.extend(features.iter().map(|f| f.as_str()));
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

    for message in messages {
        match message.into_diagnostic()? {
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

    Ok(artifact_path)
}

fn save_image(
    matches: SaveImageOpts,
    _config: Config,
    metadata: CargoEspFlashMeta,
    cargo_config: CargoConfig,
) -> Result<()> {
    let target = matches
        .build_args
        .target
        .as_deref()
        .or_else(|| cargo_config.target())
        .ok_or_else(|| NoTargetError::new(None))
        .into_diagnostic()?;

    let chip = Chip::from_target(target).ok_or_else(|| Error::UnknownTarget(target.into()))?;

    let path = build(&matches.build_args, &cargo_config, Some(chip))?;
    let elf_data = fs::read(path).into_diagnostic()?;

    let image = FirmwareImage::from_data(&elf_data)?;

    let image_format = matches
        .build_args
        .format
        .as_deref()
        .map(ImageFormatId::from_str)
        .transpose()?
        .or(metadata.format);

    let flash_image = chip.get_flash_image(&image, None, None, image_format, None)?;
    let parts: Vec<_> = flash_image.ota_segments().collect();

    let out_path = matches.file;

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

fn board_info(
    matches: BoardInfoOpts,
    config: Config,
    _metadata: CargoEspFlashMeta,
    _cargo_config: CargoConfig,
) -> Result<()> {
    let mut flasher = connect(&matches.connect_args, &config)?;
    flasher.board_info()?;
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

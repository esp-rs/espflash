use std::{
    fs,
    path::PathBuf,
    process::{exit, Command, ExitStatus, Stdio},
    string::ToString,
};

use cargo_metadata::Message;
use clap::{App, Arg, ArgMatches, SubCommand};
use error::Error;
use espflash::{Chip, Config, FirmwareImage, Flasher, ImageFormatId, PartitionTable};
use miette::{IntoDiagnostic, Result, WrapErr};
use monitor::monitor;
use package_metadata::CargoEspFlashMeta;
use serial::{BaudRate, FlowControl, SerialPort};

use crate::cargo_config::CargoConfig;
use crate::error::NoTargetError;
use crate::{cargo_config::parse_cargo_config, error::UnsupportedTargetError};
use std::str::FromStr;

mod cargo_config;
mod error;
mod line_endings;
mod monitor;
mod package_metadata;

fn main() -> Result<()> {
    miette::set_panic_hook();

    let build_args = [
        Arg::with_name("release")
            .long("release")
            .help("Build the application using the release profile"),
        Arg::with_name("example")
            .long("example")
            .takes_value(true)
            .value_name("EXAMPLE")
            .help("Example to build and flash"),
        Arg::with_name("features")
            .long("features")
            .use_delimiter(true)
            .takes_value(true)
            .value_name("FEATURES")
            .help("Comma delimited list of build features"),
        Arg::with_name("format")
            .long("format")
            .takes_value(true)
            .value_name("image format")
            .help("Image format to flash"),
    ];
    let connect_args = [Arg::with_name("serial")
        .takes_value(true)
        .value_name("SERIAL")
        .help("Serial port connected to target device")];

    let mut app = App::new(env!("CARGO_PKG_NAME"))
        .bin_name("cargo")
        .subcommand(
            SubCommand::with_name("espflash")
                .version(env!("CARGO_PKG_VERSION"))
                .about(env!("CARGO_PKG_DESCRIPTION"))
                .arg(
                    Arg::with_name("board_info")
                        .long("board-info")
                        .help("Display the connected board's information (deprecated, use the `board-info` subcommand instead)"),
                )
                .args(&build_args)
                .arg(
                    Arg::with_name("ram")
                        .long("ram")
                        .help("Load the application to RAM instead of Flash"),
                )
                .arg(
                    Arg::with_name("bootloader")
                        .long("bootloader")
                        .takes_value(true)
                        .value_name("PATH")
                        .help("Path to a binary (.bin) bootloader file"),
                )
                .arg(
                    Arg::with_name("partition_table")
                        .long("partition-table")
                        .takes_value(true)
                        .value_name("PATH")
                        .help("Path to a CSV file containing partition table"),
                )
                .arg(
                    Arg::with_name("speed")
                        .long("speed")
                        .takes_value(true)
                        .value_name("SPEED")
                        .help("Baud rate at which to flash target device"),
                )
                .args(&connect_args)
                .arg(
                    Arg::with_name("monitor")
                        .long("monitor")
                        .help("Open a serial monitor after flashing"),
                )
                .subcommand(
                    SubCommand::with_name("save-image")
                        .version(env!("CARGO_PKG_VERSION"))
                        .about("Save the image to disk instead of flashing to device")
                        .arg(
                            Arg::with_name("file")
                                .takes_value(true)
                                .required(true)
                                .value_name("FILE")
                                .help("File name to save the generated image to"),
                        )
                        .args(&build_args),
                )
                .subcommand(
                    SubCommand::with_name("board-info")
                        .version(env!("CARGO_PKG_VERSION"))
                        .about("Display the connected board's information")
                        .args(&connect_args),
                ),
        );

    let matches = app.clone().get_matches();
    let matches = match matches.subcommand_matches("espflash") {
        Some(matches) => matches,
        None => {
            app.print_help().into_diagnostic()?;
            exit(0);
        }
    };

    let config = Config::load();
    let metadata = CargoEspFlashMeta::load("Cargo.toml")?;
    let cargo_config = parse_cargo_config(".")?;

    match matches.subcommand() {
        ("board-info", Some(matches)) => board_info(matches, config, metadata, cargo_config),
        ("save-image", Some(matches)) => save_image(matches, config, metadata, cargo_config),
        _ => flash(matches, config, metadata, cargo_config),
    }
}

fn get_serial_port(matches: &ArgMatches, config: &Config) -> Result<String, Error> {
    // The serial port must be specified, either as a command-line argument or in
    // the cargo configuration file. In the case that both have been provided the
    // command-line argument will take precedence.
    if let Some(serial) = matches.value_of("serial") {
        Ok(serial.to_string())
    } else if let Some(serial) = &config.connection.serial {
        Ok(serial.into())
    } else {
        Err(Error::NoSerial)
    }
}

fn connect(matches: &ArgMatches, config: &Config) -> Result<Flasher> {
    let port = get_serial_port(matches, config)?;

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
    let speed = if let Some(speed) = matches.value_of("speed") {
        let speed = speed.parse::<usize>().into_diagnostic()?;
        Some(BaudRate::from_speed(speed))
    } else {
        None
    };

    // Connect the Flasher to the target device and print the board information
    // upon connection. If the '--board-info' flag has been provided, we have
    // nothing left to do so exit early.
    Ok(Flasher::connect(serial, speed)?)
}

fn flash(
    matches: &ArgMatches,
    config: Config,
    metadata: CargoEspFlashMeta,
    cargo_config: CargoConfig,
) -> Result<()> {
    // Connect the Flasher to the target device and print the board information
    // upon connection. If the '--board-info' flag has been provided, we have
    // nothing left to do so exit early.
    let mut flasher = connect(matches, &config)?;
    flasher.board_info()?;

    if matches.is_present("board_info") {
        return Ok(());
    }

    let build_options = BuildOptions::from_args(matches);

    let path = build(build_options, &cargo_config, Some(flasher.chip()))
        .wrap_err("Failed to build project")?;

    // If the '--bootloader' option is provided, load the binary file at the
    // specified path.
    let bootloader = if let Some(path) = matches
        .value_of("bootloader")
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
        .value_of("partition_table")
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
        .value_of("format")
        .map(ImageFormatId::from_str)
        .transpose()?
        .or(metadata.format);

    // Read the ELF data from the build path and load it to the target.
    let elf_data = fs::read(path).into_diagnostic()?;
    if matches.is_present("ram") {
        flasher.load_elf_to_ram(&elf_data)?;
    } else {
        flasher.load_elf_to_flash(&elf_data, bootloader, partition_table, image_format)?;
    }
    println!("\nFlashing has completed!");

    if matches.is_present("monitor") {
        monitor(flasher.into_serial()).into_diagnostic()?;
    }

    // We're all done!
    Ok(())
}

struct BuildOptions<'a> {
    release: bool,
    example: Option<&'a str>,
    features: Option<&'a str>,
}

impl<'a> BuildOptions<'a> {
    pub fn from_args(args: &'a ArgMatches) -> Self {
        BuildOptions {
            release: args.is_present("release"),
            example: args.value_of("example"),
            features: args.value_of("features"),
        }
    }
}

fn build(
    build_options: BuildOptions,
    cargo_config: &CargoConfig,
    chip: Option<Chip>,
) -> Result<PathBuf> {
    let target = cargo_config
        .target()
        .ok_or_else(|| NoTargetError::new(chip))?;
    if let Some(chip) = chip {
        if !chip.supports_target(target) {
            return Err(Error::UnsupportedTarget(UnsupportedTargetError::new(target, chip)).into());
        }
    }
    // The 'build-std' unstable cargo feature is required to enable
    // cross-compilation for xtensa targets.
    // If it has not been set then we cannot build the
    // application.
    if !cargo_config.has_build_std() && target.starts_with("xtensa-") {
        return Err(Error::NoBuildStd.into());
    };

    // Build the list of arguments to pass to 'cargo build'.
    let mut args = vec![];

    if build_options.release {
        args.push("--release");
    }

    if let Some(example) = build_options.example {
        args.push("--example");
        args.push(example);
    }

    if let Some(features) = build_options.features {
        args.push("--features");
        args.push(features);
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
    matches: &ArgMatches,
    _config: Config,
    metadata: CargoEspFlashMeta,
    cargo_config: CargoConfig,
) -> Result<()> {
    let target = cargo_config
        .target()
        .ok_or_else(|| NoTargetError::new(None))?;
    let chip = Chip::from_target(target).ok_or_else(|| Error::UnknownTarget(target.into()))?;
    let build_options = BuildOptions::from_args(matches);

    let path = build(build_options, &cargo_config, Some(chip))?;
    let elf_data = fs::read(path).into_diagnostic()?;

    let image = FirmwareImage::from_data(&elf_data)?;

    let image_format = matches
        .value_of("format")
        .map(ImageFormatId::from_str)
        .transpose()?
        .or(metadata.format);

    let flash_image = chip.get_flash_image(&image, None, None, image_format)?;
    let parts: Vec<_> = flash_image.ota_segments().collect();

    let out_path = matches.value_of("file").unwrap();

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
    matches: &ArgMatches,
    config: Config,
    _metadata: CargoEspFlashMeta,
    _cargo_config: CargoConfig,
) -> Result<()> {
    let mut flasher = connect(matches, &config)?;
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

use cargo_config::has_build_std;
use cargo_metadata::Message;
use clap::{App, Arg, SubCommand};
use error::Error;
use espflash::{Config, Flasher, PartitionTable};
use miette::{IntoDiagnostic, Result, WrapErr};
use monitor::monitor;
use package_metadata::CargoEspFlashMeta;
use serial::{BaudRate, FlowControl, SerialPort};
use std::{
    fs,
    path::PathBuf,
    process::{exit, Command, ExitStatus, Stdio},
    string::ToString,
};

mod cargo_config;
mod error;
mod line_endings;
mod monitor;
mod package_metadata;

fn main() -> Result<()> {
    miette::set_panic_hook();
    let mut app = App::new(env!("CARGO_PKG_NAME"))
        .bin_name("cargo")
        .subcommand(
            SubCommand::with_name("espflash")
                .version(env!("CARGO_PKG_VERSION"))
                .about(env!("CARGO_PKG_DESCRIPTION"))
                .arg(
                    Arg::with_name("board_info")
                        .long("board-info")
                        .help("Display the connected board's information"),
                )
                .arg(
                    Arg::with_name("ram")
                        .long("ram")
                        .help("Load the application to RAM instead of Flash"),
                )
                .arg(
                    Arg::with_name("release")
                        .long("release")
                        .help("Build the application using the release profile"),
                )
                .arg(
                    Arg::with_name("bootloader")
                        .long("bootloader")
                        .takes_value(true)
                        .value_name("PATH")
                        .help("Path to a binary (.bin) bootloader file"),
                )
                .arg(
                    Arg::with_name("example")
                        .long("example")
                        .takes_value(true)
                        .value_name("EXAMPLE")
                        .help("Example to build and flash"),
                )
                .arg(
                    Arg::with_name("features")
                        .long("features")
                        .use_delimiter(true)
                        .takes_value(true)
                        .value_name("FEATURES")
                        .help("Comma delimited list of build features"),
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
                .arg(
                    Arg::with_name("serial")
                        .takes_value(true)
                        .value_name("SERIAL")
                        .help("Serial port connected to target device"),
                )
                .arg(
                    Arg::with_name("monitor")
                        .long("monitor")
                        .help("Open a serial monitor after flashing"),
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

    // The serial port must be specified, either as a command-line argument or in
    // the cargo configuration file. In the case that both have been provided the
    // command-line argument will take precedence.
    let port = if let Some(serial) = matches.value_of("serial") {
        serial.to_string()
    } else if let Some(serial) = config.connection.serial {
        serial
    } else {
        app.print_help().into_diagnostic()?;
        exit(0);
    };

    // Only build the application if the '--board-info' flag has not been passed.
    let show_board_info = matches.is_present("board_info");
    let path = if !show_board_info {
        let release = matches.is_present("release");
        let example = matches.value_of("example");
        let features = matches.value_of("features");

        let path = build(release, example, features)?;

        Some(path)
    } else {
        None
    };

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

    // Connect the Flasher to the target device. If the '--board-info' flag has been
    // provided, display the board info and terminate the application.
    let mut flasher = Flasher::connect(serial, speed)?;
    if show_board_info {
        board_info(&flasher);
        return Ok(());
    }

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
        let data = fs::read_to_string(path).into_diagnostic()?;
        let table = PartitionTable::try_from_str(data)?;
        Some(table)
    } else {
        None
    };

    // Read the ELF data from the build path and load it to the target.
    let elf_data = fs::read(path.unwrap()).into_diagnostic()?;
    if matches.is_present("ram") {
        flasher.load_elf_to_ram(&elf_data)?;
    } else {
        flasher.load_elf_to_flash(&elf_data, bootloader, partition_table)?;
    }

    if matches.is_present("monitor") {
        monitor(flasher.into_serial()).into_diagnostic()?;
    }

    // We're all done!
    Ok(())
}

fn board_info(flasher: &Flasher) {
    println!("Chip type:  {}", flasher.chip());
    println!("Flash size: {}", flasher.flash_size());
}

fn build(release: bool, example: Option<&str>, features: Option<&str>) -> Result<PathBuf> {
    // The 'build-std' unstable cargo feature is required to enable
    // cross-compilation. If it has not been set then we cannot build the
    // application.
    if !has_build_std(".") {
        return Err(Error::NoBuildStd.into());
    };

    // Build the list of arguments to pass to 'cargo build'.
    let mut args = vec![];

    if release {
        args.push("--release");
    }

    if let Some(example) = example {
        args.push("--example");
        args.push(example);
    }

    if let Some(features) = features {
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
    // occuring during the build are shown above, when the compiler messages are
    // rendered.
    if !output.status.success() {
        exit_with_process_status(output.status);
    }

    // If no target artifact was found, we don't have a path to return.
    let target_artifact = target_artifact.ok_or(Error::NoArtifact)?;

    let artifact_path = target_artifact.executable.unwrap().into();

    Ok(artifact_path)
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

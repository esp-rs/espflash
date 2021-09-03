mod cargo_config;

use std::ffi::OsString;
use std::fs::read;
use std::path::PathBuf;
use std::process::{exit, Command, ExitStatus, Stdio};

use crate::cargo_config::has_build_std;
use anyhow::{anyhow, bail, Context, Result};
use cargo_metadata::Message;
use espflash::{Config, Flasher};
use pico_args::Arguments;
use serial::{BaudRate, SerialPort};

fn main() -> Result<()> {
    let args = parse_args().expect("Unable to parse command-line arguments");
    let config = Config::load();

    if args.help || (args.serial.is_none() && config.connection.serial.is_none()) {
        return usage();
    }

    let port = args
        .serial
        .or(config.connection.serial)
        .context("serial port missing")?;

    let speed = args.speed.map(|v| BaudRate::from_speed(v as usize));

    // Don't build if we are just querying board info
    let path = if !args.board_info {
        build(args.release, &args.example, &args.features)?
    } else {
        PathBuf::new()
    };

    let mut serial = serial::open(&port).context(format!("Failed to open serial port {}", port))?;
    serial.reconfigure(&|settings| {
        settings.set_baud_rate(BaudRate::Baud115200)?;
        Ok(())
    })?;

    let mut flasher = Flasher::connect(serial, speed)?;
    if args.board_info {
        return board_info(&flasher);
    }

    let elf_data = read(&path)?;

    if args.ram {
        flasher.load_elf_to_ram(&elf_data)?;
    } else {
        flasher.load_elf_to_flash(&elf_data)?;
    }

    Ok(())
}

#[derive(Debug)]
struct AppArgs {
    help: bool,
    board_info: bool,
    ram: bool,
    release: bool,
    example: Option<String>,
    features: Option<String>,
    chip: Option<String>,
    speed: Option<u32>,
    serial: Option<String>,
}

#[allow(clippy::unnecessary_wraps)]
fn usage() -> Result<()> {
    let usage = "Usage: cargo espflash \
      [--board-info] \
      [--ram] \
      [--release] \
      [--example EXAMPLE] \
      [--chip {{esp32,esp32c3,esp8266}}] \
      [--speed BAUD] \
      <serial>";

    println!("{}", usage);

    Ok(())
}

#[allow(clippy::unnecessary_wraps)]
fn board_info(flasher: &Flasher) -> Result<()> {
    println!("Chip type:  {:?}", flasher.chip());
    println!("Flash size: {:?}", flasher.flash_size());

    Ok(())
}

fn parse_args() -> Result<AppArgs> {
    // Skip the command and subcommand (ie. 'cargo espflash') and convert the
    // remaining arguments to the expected type.
    let args = std::env::args().skip(2).map(OsString::from).collect();

    let mut args = Arguments::from_vec(args);

    let app_args = AppArgs {
        help: args.contains("--help"),
        board_info: args.contains("--board-info"),
        ram: args.contains("--ram"),
        release: args.contains("--release"),
        example: args.opt_value_from_str("--example")?,
        features: args.opt_value_from_str("--features")?,
        chip: args.opt_value_from_str("--chip")?,
        speed: args.opt_value_from_str("--speed")?,
        serial: args.opt_free_from_str()?,
    };

    Ok(app_args)
}

fn build(release: bool, example: &Option<String>, features: &Option<String>) -> Result<PathBuf> {
    let mut args: Vec<String> = vec![];

    if release {
        args.push("--release".to_string());
    }

    match example {
        Some(example) => {
            args.push("--example".to_string());
            args.push(example.to_string());
        }
        None => {}
    }

    match features {
        Some(features) => {
            args.push("--features".to_string());
            args.push(features.to_string());
        }
        None => {}
    }

    if !has_build_std(".") {
        bail!(
            r#"cargo currently requires the unstable build-std, ensure .cargo/config{{.toml}} has the appropriate options.
        See: https://doc.rust-lang.org/cargo/reference/unstable.html#build-std"#
        );
    };

    let output = Command::new("cargo")
        .arg("build")
        .args(args)
        .args(&["--message-format", "json-diagnostic-rendered-ansi"])
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()?
        .wait_with_output()?;

    // Parse build output.
    let messages = Message::parse_stream(&output.stdout[..]);

    // Find artifacts.
    let mut target_artifact = None;

    for message in messages {
        match message? {
            Message::CompilerArtifact(artifact) => {
                if artifact.executable.is_some() {
                    if target_artifact.is_some() {
                        // We found multiple binary artifacts,
                        // so we don't know which one to use.
                        bail!("Multiple artifacts found, please specify one with --bin");
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
            // Ignore other messages.
            _ => (),
        }
    }

    // Check if the command succeeded, otherwise return an error.
    // Any error messages occuring during the build are shown above,
    // when the compiler messages are rendered.
    if !output.status.success() {
        exit_with_process_status(output.status);
    }

    if let Some(artifact) = target_artifact {
        let artifact_path = PathBuf::from(
            artifact
                .executable
                .ok_or(anyhow!("artifact executable path is missing"))?
                .as_path(),
        );
        Ok(artifact_path)
    } else {
        bail!("Artifact not found");
    }
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

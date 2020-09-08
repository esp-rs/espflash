use std::ffi::OsString;
use std::fs::read;
use std::path::PathBuf;
use std::process::{exit, Command, ExitStatus, Stdio};

use cargo_project::{Artifact, Profile, Project};
use espflash::Flasher;
use main_error::MainError;
use pico_args::Arguments;
use serial::{BaudRate, SerialPort};

fn main() -> Result<(), MainError> {
    let args = parse_args().expect("Unable to parse command-line arguments");

    if args.help || args.chip.is_none() || args.serial.is_none() {
        return usage();
    }

    let chip = args.chip.unwrap().to_lowercase();
    let target = match chip.as_str() {
        "esp32" => "xtensa-esp32-none-elf",
        "esp8266" => "xtensa-esp8266-none-elf",
        _ => return usage(),
    };

    let path = get_artifact_path(target, args.release, &args.example)
        .expect("Could not find the build artifact path");

    let status = build(args.release, args.example);
    if !status.success() {
        exit_with_process_status(status)
    }

    let port = args.serial.unwrap();
    let mut serial = serial::open(&port)?;
    serial.reconfigure(&|settings| {
        settings.set_baud_rate(BaudRate::Baud115200)?;

        Ok(())
    })?;

    let mut flasher = Flasher::connect(serial)?;
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
    ram: bool,
    release: bool,
    example: Option<String>,
    chip: Option<String>,
    serial: Option<String>,
}

fn usage() -> Result<(), MainError> {
    let mut usage = String::from("Usage: cargo espflash ");
    usage += "[--ram] [--release] [--example EXAMPLE] ";
    usage += "--chip {{esp32,esp8266}} <serial>";

    println!("{}", usage);

    Ok(())
}

fn parse_args() -> Result<AppArgs, MainError> {
    // Skip the command and subcommand (ie. 'cargo espflash') and convert the
    // remaining arguments to the expected type.
    let args = std::env::args()
        .skip(2)
        .map(|arg| OsString::from(arg))
        .collect();

    let mut args = Arguments::from_vec(args);

    let app_args = AppArgs {
        help: args.contains("--help"),
        ram: args.contains("--ram"),
        release: args.contains("--release"),
        example: args.opt_value_from_str("--example")?,
        chip: args.opt_value_from_str("--chip")?,
        serial: args.free_from_str()?,
    };

    Ok(app_args)
}

fn get_artifact_path(
    target: &str,
    release: bool,
    example: &Option<String>,
) -> Result<PathBuf, MainError> {
    let project = Project::query(".").unwrap();

    let artifact = match example {
        Some(example) => Artifact::Example(example.as_str()),
        None => Artifact::Bin(project.name()),
    };

    let profile = if release {
        Profile::Release
    } else {
        Profile::Dev
    };

    let host = "x86_64-unknown-linux-gnu";
    let path = project.path(artifact, profile, Some(target), host);

    path.map_err(|e| MainError::from(e))
}

fn build(release: bool, example: Option<String>) -> ExitStatus {
    let mut args: Vec<String> = vec![];

    if release {
        args.push("--release".to_string());
    }

    if example.is_some() {
        args.push("--example".to_string());
        args.push(example.unwrap());
    }

    Command::new("xargo")
        .arg("build")
        .args(args)
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()
        .unwrap()
        .wait()
        .unwrap()
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

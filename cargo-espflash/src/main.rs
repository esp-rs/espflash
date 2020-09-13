use std::ffi::OsString;
use std::fs::read;
use std::path::PathBuf;
use std::process::{exit, Command, ExitStatus, Stdio};

use cargo_project::{Artifact, Profile, Project};
use espflash::{Chip, Flasher};
use main_error::MainError;
use pico_args::Arguments;
use serial::{BaudRate, SerialPort};

fn main() -> Result<(), MainError> {
    let args = parse_args().expect("Unable to parse command-line arguments");

    if args.help || args.serial.is_none() {
        return usage();
    }

    let tool = args
        .build_tool
        .as_ref()
        .map(|build_tool| build_tool.as_str())
        .or_else(|| Some("cargo"));

    let tool = match tool {
        Some("xargo") | Some("cargo") | Some("xbuild") => tool.unwrap(),
        Some(_) => {
            eprintln!("Only 'xargo', 'cargo' and 'xbuild' are valid build types.");
            return Ok(());
        }
        None => return usage(),
    };

    let port = args.serial.unwrap();

    let chip = args
        .chip
        .as_ref()
        .map(|chip| chip.as_str())
        .or_else(|| chip_detect(&port));

    let target = match chip {
        Some("esp32") => "xtensa-esp32-none-elf",
        Some("esp8266") => "xtensa-esp8266-none-elf",
        Some(_) => return usage(),
        None => {
            eprintln!("Unable to detect chip type, ensure your device is connected or manually specify the chip");
            return Ok(());
        }
    };

    let path = get_artifact_path(target, args.release, &args.example)
        .expect("Could not find the build artifact path");

    let status = build(args.release, args.example, tool);
    if !status.success() {
        exit_with_process_status(status)
    }

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
    build_tool: Option<String>,
    serial: Option<String>,
}

fn usage() -> Result<(), MainError> {
    let mut usage = String::from("Usage: cargo espflash ");
    usage += "[--ram] [--release] [--example EXAMPLE] [--build-tool {{cargo,xargo,xbuild}}]";
    usage += "[--chip {{esp32,esp8266}}] <serial>";

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
        build_tool: args.opt_value_from_str("--tool")?,
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

fn build(release: bool, example: Option<String>, tool: &str) -> ExitStatus {
    let mut args: Vec<String> = vec![];

    if release {
        args.push("--release".to_string());
    }

    if example.is_some() {
        args.push("--example".to_string());
        args.push(example.unwrap());
    }

    let mut command = match tool {
        "cargo" | "xbuild" => Command::new("cargo"),
        "xargo" => Command::new("xargo"),
        _ => unreachable!(),
    };

    let command = match tool {
        "xargo" | "cargo" => command.arg("build"),
        "xbuild" => command.arg("xbuild"),
        _ => unreachable!(),
    };

    command
        .args(args)
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()
        .unwrap()
        .wait()
        .unwrap()
}

fn chip_detect(port: &str) -> Option<&'static str> {
    let serial = serial::open(port).ok()?;
    let flasher = Flasher::connect(serial).ok()?;
    Some(match flasher.chip() {
        Chip::Esp8266 => "esp8266",
        Chip::Esp32 => "esp32",
    })
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

use std::fs::read;

use color_eyre::{eyre::WrapErr, Result};
use espflash::{Config, Flasher};
use pico_args::Arguments;
use serial::{BaudRate, SerialPort};

#[allow(clippy::unnecessary_wraps)]
fn help() -> Result<()> {
    println!("Usage: espflash [--board-info] [--ram] <serial> <elf image>");
    Ok(())
}

fn main() -> Result<()> {
    let mut args = Arguments::from_env();
    let config = Config::load();

    if args.contains(["-h", "--help"]) {
        return help();
    }

    let ram = args.contains("--ram");
    let board_info = args.contains("--board-info");

    let mut serial: Option<String> = args.opt_free_from_str()?;
    let mut elf: Option<String> = args.opt_free_from_str()?;

    if elf.is_none() && config.connection.serial.is_some() {
        elf = serial.take();
        serial = config.connection.serial;
    }

    let serial: String = match serial {
        Some(serial) => serial,
        _ => return help(),
    };

    let mut serial =
        serial::open(&serial).wrap_err_with(|| format!("Failed to open serial port {}", serial))?;
    serial.reconfigure(&|settings| {
        settings.set_baud_rate(BaudRate::Baud115200)?;

        Ok(())
    })?;

    let mut flasher = Flasher::connect(serial, None)?;

    if board_info {
        println!("Chip type: {:?}", flasher.chip());
        println!("Flash size: {:?}", flasher.flash_size());

        return Ok(());
    }

    let input: String = match elf {
        Some(input) => input,
        _ => return help(),
    };
    let input_bytes =
        read(&input).wrap_err_with(|| format!("Failed to open elf image \"{}\"", input))?;

    if ram {
        flasher.load_elf_to_ram(&input_bytes)?;
    } else {
        flasher.load_elf_to_flash(&input_bytes)?;
    }

    Ok(())
}

//! CLI utilities shared between espflash and cargo-espflash
//!
//! No stability guaranties apply

use std::{fs, path::PathBuf};

use config::Config;
use miette::{IntoDiagnostic, Result, WrapErr};
use serialport::{FlowControl, SerialPortType};

use self::clap::ConnectOpts;
use crate::{
    cli::serial::get_serial_port_info, error::Error, Chip, FirmwareImage, Flasher, ImageFormatId,
};

pub mod clap;
pub mod config;
pub mod monitor;

mod line_endings;
mod serial;

pub fn connect(opts: &ConnectOpts, config: &Config) -> Result<Flasher> {
    let port_info = get_serial_port_info(opts, config)?;

    // Attempt to open the serial port and set its initial baud rate.
    println!("Serial port: {}", port_info.port_name);
    println!("Connecting...\n");
    let serial = serialport::new(&port_info.port_name, 115_200)
        .flow_control(FlowControl::None)
        .open()
        .map_err(Error::from)
        .wrap_err_with(|| format!("Failed to open serial port {}", port_info.port_name))?;

    // NOTE: since `get_serial_port_info` filters out all non-USB serial ports, we
    //       can just pretend the remaining types don't exist here.
    let port_info = match port_info.port_type {
        SerialPortType::UsbPort(info) => info,
        _ => unreachable!(),
    };

    Ok(Flasher::connect(serial, port_info, opts.speed)?)
}

pub fn board_info(opts: ConnectOpts, config: Config) -> Result<()> {
    let mut flasher = connect(&opts, &config)?;
    flasher.board_info()?;

    Ok(())
}

pub fn save_elf_as_image(
    chip: Chip,
    elf_data: &[u8],
    path: PathBuf,
    image_format: Option<ImageFormatId>,
) -> Result<()> {
    let image = FirmwareImage::from_data(elf_data)?;

    let flash_image = chip.get_flash_image(&image, None, None, image_format, None)?;
    let parts: Vec<_> = flash_image.ota_segments().collect();

    match parts.as_slice() {
        [single] => fs::write(path, &single.data).into_diagnostic()?,
        parts => {
            for part in parts {
                let part_path = format!("{:#x}_{}", part.addr, path.display());
                fs::write(part_path, &part.data).into_diagnostic()?
            }
        }
    }

    Ok(())
}

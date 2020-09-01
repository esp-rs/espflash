mod elf;
mod encoder;
mod error;
mod flasher;

pub use error::Error;
use flasher::Flasher;
use serial::{BaudRate, SerialPort};
use std::env::args;
use std::fs::read;

fn main() -> Result<(), Error> {
    let mut args = args();
    let bin = args.next().unwrap();
    let serial = args
        .next()
        .expect(&format!("usage: {} <serial> <input>", bin));
    let input = args
        .next()
        .expect(&format!("usage: {} <serial> <input>", bin));

    let mut serial = serial::open(&serial).unwrap();
    serial
        .reconfigure(&|settings| {
            settings.set_baud_rate(BaudRate::Baud115200)?;

            Ok(())
        })
        .unwrap();

    let mut flasher = Flasher::new(serial);

    let input_bytes = read(&input).unwrap();

    flasher.load_elf_to_flash(&input_bytes)?;

    Ok(())
}

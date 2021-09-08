mod esp32;
mod esp8266;
mod ram;

use crate::connection::Connection;
use crate::elf::{FirmwareImage, RomSegment};
use crate::error::Error;
use crate::flasher::{checksum, Command, Encoder, CHECKSUM_INIT, FLASH_WRITE_SIZE};
use bytemuck::{bytes_of, Pod, Zeroable};
pub use esp32::Esp32Target;
pub use esp8266::Esp8266Target;
pub use ram::RamTarget;
use std::mem::size_of;

pub trait FlashTarget {
    fn begin(&mut self, connection: &mut Connection, image: &FirmwareImage) -> Result<(), Error>;
    fn write_segment(
        &mut self,
        connection: &mut Connection,
        segment: RomSegment,
    ) -> Result<(), Error>;
    fn finish(&mut self, connection: &mut Connection, reboot: bool) -> Result<(), Error>;
}

#[derive(Zeroable, Pod, Copy, Clone, Debug)]
#[repr(C)]
struct BeginParams {
    size: u32,
    blocks: u32,
    block_size: u32,
    offset: u32,
    encrypted: u32,
}

fn begin_command(
    connection: &mut Connection,
    command: Command,
    size: u32,
    blocks: u32,
    block_size: u32,
    offset: u32,
    supports_encrypted: bool,
) -> Result<(), Error> {
    let params = BeginParams {
        size,
        blocks,
        block_size,
        offset,
        encrypted: 0,
    };

    let bytes = bytes_of(&params);
    let data = if !supports_encrypted {
        // The ESP32 and ESP8266 do not take the `encrypted` field, so truncate the last
        // 4 bytes of the slice where it resides.
        let end = bytes.len() - 4;
        &bytes[0..end]
    } else {
        bytes
    };

    connection.with_timeout(command.timeout_for_size(size), |connection| {
        connection.command(command as u8, data, 0)?;
        Ok(())
    })
}

#[derive(Zeroable, Pod, Copy, Clone, Debug)]
#[repr(C)]
struct BlockParams {
    size: u32,
    sequence: u32,
    dummy1: u32,
    dummy2: u32,
}

fn block_command(
    connection: &mut Connection,
    command: Command,
    data: &[u8],
    padding: usize,
    padding_byte: u8,
    sequence: u32,
) -> Result<(), Error> {
    let params = BlockParams {
        size: (data.len() + padding) as u32,
        sequence,
        dummy1: 0,
        dummy2: 0,
    };

    let length = size_of::<BlockParams>() + data.len() + padding;

    let mut check = checksum(data, CHECKSUM_INIT);

    for _ in 0..padding {
        check = checksum(&[padding_byte], check);
    }

    connection.with_timeout(command.timeout_for_size(data.len() as u32), |connection| {
        connection.command(
            command as u8,
            (length as u16, |encoder: &mut Encoder| {
                encoder.write(bytes_of(&params))?;
                encoder.write(data)?;
                let padding = &[padding_byte; FLASH_WRITE_SIZE][0..padding];
                encoder.write(padding)?;
                Ok(())
            }),
            check as u32,
        )?;
        Ok(())
    })
}

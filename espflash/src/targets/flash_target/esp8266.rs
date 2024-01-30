use crate::{
    command::{Command, CommandType},
    elf::RomSegment,
    error::Error,
    flasher::{get_erase_size, ProgressCallbacks, FLASH_WRITE_SIZE},
};
#[cfg(feature = "serialport")]
use crate::{connection::Connection, targets::FlashTarget};

/// Applications running from an ESP8266's flash
#[derive(Default)]
pub struct Esp8266Target;

impl Esp8266Target {
    pub fn new() -> Self {
        Esp8266Target
    }
}

#[cfg(feature = "serialport")]
impl FlashTarget for Esp8266Target {
    fn begin(&mut self, connection: &mut Connection) -> Result<(), Error> {
        connection.command(Command::FlashBegin {
            size: 0,
            blocks: 0,
            block_size: FLASH_WRITE_SIZE as u32,
            offset: 0,
            supports_encryption: false,
        })?;

        Ok(())
    }

    fn write_segment(
        &mut self,
        connection: &mut Connection,
        segment: RomSegment,
        progress: &mut Option<&mut dyn ProgressCallbacks>,
    ) -> Result<(), Error> {
        let addr = segment.addr;
        let block_count = (segment.data.len() + FLASH_WRITE_SIZE - 1) / FLASH_WRITE_SIZE;

        let erase_size = get_erase_size(addr as usize, segment.data.len()) as u32;

        connection.with_timeout(
            CommandType::FlashBegin.timeout_for_size(erase_size),
            |connection| {
                connection.command(Command::FlashBegin {
                    size: erase_size,
                    blocks: block_count as u32,
                    block_size: FLASH_WRITE_SIZE as u32,
                    offset: addr,
                    supports_encryption: false,
                })
            },
        )?;

        let chunks = segment.data.chunks(FLASH_WRITE_SIZE);
        let num_chunks = chunks.len();

        if let Some(cb) = progress.as_mut() {
            cb.init(addr, num_chunks)
        }

        for (i, block) in chunks.enumerate() {
            connection.command(Command::FlashData {
                sequence: i as u32,
                pad_to: FLASH_WRITE_SIZE,
                pad_byte: 0xff,
                data: block,
            })?;

            if let Some(cb) = progress.as_mut() {
                cb.update(i + 1)
            }
        }

        if let Some(cb) = progress.as_mut() {
            cb.finish()
        }

        Ok(())
    }

    fn finish(&mut self, connection: &mut Connection, reboot: bool) -> Result<(), Error> {
        connection.with_timeout(CommandType::FlashEnd.timeout(), |connection| {
            connection.write_command(Command::FlashEnd { reboot: false })
        })?;

        if reboot {
            connection.reset()?;
        }

        Ok(())
    }
}

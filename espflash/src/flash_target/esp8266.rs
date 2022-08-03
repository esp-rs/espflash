use crate::command::{Command, CommandType};
use crate::connection::Connection;
use crate::elf::RomSegment;
use crate::error::Error;
use crate::flash_target::FlashTarget;
use crate::flasher::{get_erase_size, FLASH_WRITE_SIZE};
use indicatif::{ProgressBar, ProgressStyle};

pub struct Esp8266Target;

impl Esp8266Target {
    pub fn new() -> Self {
        Esp8266Target
    }
}

impl Default for Esp8266Target {
    fn default() -> Self {
        Self::new()
    }
}

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

        let (_, chunk_size) = chunks.size_hint();
        let chunk_size = chunk_size.unwrap_or(0) as u64;
        let pb_chunk = ProgressBar::new(chunk_size);
        pb_chunk.set_style(
            ProgressStyle::default_bar()
                .template("[{elapsed_precise}] {bar:40.cyan/blue} {pos:>7}/{len:7} {msg}")
                .unwrap()
                .progress_chars("#>-"),
        );

        for (i, block) in chunks.enumerate() {
            pb_chunk.set_message(format!("segment 0x{:X} writing chunks", addr));
            connection.command(Command::FlashData {
                sequence: i as u32,
                pad_to: FLASH_WRITE_SIZE,
                pad_byte: 0xff,
                data: block,
            })?;
            pb_chunk.inc(1);
        }

        pb_chunk.finish_with_message(format!("segment 0x{:X}", addr));

        Ok(())
    }

    fn finish(&mut self, connection: &mut Connection, reboot: bool) -> Result<(), Error> {
        connection.with_timeout(CommandType::FlashEnd.timeout(), |connection| {
            connection.write_command(Command::FlashEnd { reboot: false })
        })?;
        if reboot {
            connection.reset()
        } else {
            Ok(())
        }
    }
}

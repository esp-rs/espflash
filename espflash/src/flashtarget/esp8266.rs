use crate::connection::Connection;
use crate::elf::{FirmwareImage, RomSegment};
use crate::error::Error;
use crate::flasher::{get_erase_size, Command, FLASH_WRITE_SIZE};
use crate::flashtarget::{begin_command, block_command, FlashTarget};
use indicatif::{ProgressBar, ProgressStyle};

pub struct Esp8266Target;

impl Esp8266Target {
    pub fn new() -> Self {
        Esp8266Target
    }
}

impl FlashTarget for Esp8266Target {
    fn begin(&mut self, connection: &mut Connection, _image: &FirmwareImage) -> Result<(), Error> {
        begin_command(
            connection,
            Command::FlashBegin,
            0,
            0,
            FLASH_WRITE_SIZE as u32,
            0,
            false,
        )
    }

    fn write_segment(
        &mut self,
        connection: &mut Connection,
        segment: RomSegment,
    ) -> Result<(), Error> {
        let addr = segment.addr;
        let block_count = (segment.data.len() + FLASH_WRITE_SIZE - 1) / FLASH_WRITE_SIZE;

        let erase_size = get_erase_size(addr as usize, segment.data.len()) as u32;

        begin_command(
            connection,
            Command::FlashBegin,
            erase_size,
            block_count as u32,
            FLASH_WRITE_SIZE as u32,
            addr,
            false,
        )?;

        let chunks = segment.data.chunks(FLASH_WRITE_SIZE);

        let (_, chunk_size) = chunks.size_hint();
        let chunk_size = chunk_size.unwrap_or(0) as u64;
        let pb_chunk = ProgressBar::new(chunk_size);
        pb_chunk.set_style(
            ProgressStyle::default_bar()
                .template("[{elapsed_precise}] {bar:40.cyan/blue} {pos:>7}/{len:7} {msg}")
                .progress_chars("#>-"),
        );

        for (i, block) in chunks.enumerate() {
            pb_chunk.set_message(format!("segment 0x{:X} writing chunks", addr));
            let block_padding = FLASH_WRITE_SIZE - block.len();
            block_command(
                connection,
                Command::FlashData,
                block,
                block_padding,
                0xff,
                i as u32,
            )?;
            pb_chunk.inc(1);
        }

        pb_chunk.finish_with_message(format!("segment 0x{:X}", addr));

        Ok(())
    }

    fn finish(&mut self, connection: &mut Connection, reboot: bool) -> Result<(), Error> {
        connection.with_timeout(Command::FlashEnd.timeout(), |connection| {
            connection.write_command(Command::FlashEnd as u8, &[1][..], 0)
        })?;
        if reboot {
            connection.reset()
        } else {
            Ok(())
        }
    }
}

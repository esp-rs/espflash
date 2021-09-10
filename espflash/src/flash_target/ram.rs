use crate::connection::Connection;
use crate::elf::{FirmwareImage, RomSegment};
use crate::error::Error;
use crate::flash_target::{begin_command, block_command, FlashTarget};
use crate::flasher::Command;
use bytemuck::{bytes_of, Pod, Zeroable};

#[derive(Zeroable, Pod, Copy, Clone)]
#[repr(C)]
struct EntryParams {
    no_entry: u32,
    entry: u32,
}

pub struct RamTarget {
    entry: Option<u32>,
}

impl RamTarget {
    pub fn new() -> Self {
        RamTarget { entry: None }
    }
}

impl FlashTarget for RamTarget {
    fn begin(&mut self, _connection: &mut Connection, image: &FirmwareImage) -> Result<(), Error> {
        self.entry = Some(image.entry());
        Ok(())
    }

    fn write_segment(
        &mut self,
        connection: &mut Connection,
        segment: RomSegment,
    ) -> Result<(), Error> {
        const MAX_RAM_BLOCK_SIZE: usize = 0x1800;

        let padding = 4 - segment.data.len() % 4;
        let block_count =
            (segment.data.len() + padding + MAX_RAM_BLOCK_SIZE - 1) / MAX_RAM_BLOCK_SIZE;

        begin_command(
            connection,
            Command::MemBegin,
            segment.data.len() as u32,
            block_count as u32,
            MAX_RAM_BLOCK_SIZE as u32,
            segment.addr,
            false,
        )?;

        for (i, block) in segment.data.chunks(MAX_RAM_BLOCK_SIZE).enumerate() {
            let block_padding = if i == block_count - 1 { padding } else { 0 };
            block_command(
                connection,
                Command::MemData,
                block,
                block_padding,
                0,
                i as u32,
            )?;
        }
        Ok(())
    }

    fn finish(&mut self, connection: &mut Connection, reboot: bool) -> Result<(), Error> {
        if reboot {
            let params = match self.entry {
                Some(entry) if entry > 0 => EntryParams { no_entry: 0, entry },
                _ => EntryParams {
                    no_entry: 1,
                    entry: 0,
                },
            };
            connection.with_timeout(Command::MemEnd.timeout(), |connection| {
                connection.write_command(Command::MemEnd as u8, bytes_of(&params), 0)
            })
        } else {
            Ok(())
        }
    }
}

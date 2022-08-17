use crate::command::{Command, CommandType};
use crate::connection::Connection;
use crate::elf::RomSegment;
use crate::error::Error;
use crate::flash_target::FlashTarget;
use bytemuck::{Pod, Zeroable};

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
    pub fn new(entry: Option<u32>) -> Self {
        RamTarget { entry }
    }
}

impl Default for RamTarget {
    fn default() -> Self {
        Self::new(None)
    }
}

impl FlashTarget for RamTarget {
    fn begin(&mut self, _connection: &mut Connection) -> Result<(), Error> {
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

        connection.command(Command::MemBegin {
            size: segment.data.len() as u32,
            blocks: block_count as u32,
            block_size: MAX_RAM_BLOCK_SIZE as u32,
            offset: segment.addr,
            supports_encryption: false,
        })?;

        for (i, block) in segment.data.chunks(MAX_RAM_BLOCK_SIZE).enumerate() {
            connection.command(Command::MemData {
                sequence: i as u32,
                pad_to: 4,
                pad_byte: 0,
                data: block,
            })?;
        }
        Ok(())
    }

    fn finish(&mut self, connection: &mut Connection, reboot: bool) -> Result<(), Error> {
        if reboot {
            let entry = self.entry.unwrap_or_default();
            connection.with_timeout(CommandType::MemEnd.timeout(), |connection| {
                connection.command(Command::MemEnd {
                    no_entry: entry == 0,
                    entry,
                })
            })?;
        }
        Ok(())
    }
}

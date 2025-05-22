use crate::{Error, image_format::Segment};
#[cfg(feature = "serialport")]
use crate::{
    connection::{
        Connection,
        command::{Command, CommandType},
    },
    flasher::ProgressCallbacks,
    targets::FlashTarget,
};

pub const MAX_RAM_BLOCK_SIZE: usize = 0x1800;

/// Applications running in the target device's RAM
#[derive(Debug)]
pub struct RamTarget {
    entry: Option<u32>,
    block_size: usize,
}

impl RamTarget {
    pub fn new(entry: Option<u32>, block_size: usize) -> Self {
        RamTarget { entry, block_size }
    }
}

impl Default for RamTarget {
    fn default() -> Self {
        Self::new(None, MAX_RAM_BLOCK_SIZE)
    }
}

#[cfg(feature = "serialport")]
impl FlashTarget for RamTarget {
    fn begin(&mut self, _connection: &mut Connection) -> Result<(), Error> {
        Ok(())
    }

    fn write_segment(
        &mut self,
        connection: &mut Connection,
        segment: Segment<'_>,
        progress: &mut Option<&mut dyn ProgressCallbacks>,
    ) -> Result<(), Error> {
        let addr = segment.addr;

        let padding = 4 - segment.data.len() % 4;
        let block_count = (segment.data.len() + padding).div_ceil(self.block_size);

        connection.command(Command::MemBegin {
            size: segment.data.len() as u32,
            blocks: block_count as u32,
            block_size: self.block_size as u32,
            offset: addr,
            supports_encryption: false,
        })?;

        let chunks = segment.data.chunks(self.block_size);
        let num_chunks = chunks.len();

        if let Some(cb) = progress.as_mut() {
            cb.init(addr, num_chunks)
        }

        for (i, block) in chunks.enumerate() {
            connection.command(Command::MemData {
                sequence: i as u32,
                pad_to: 4,
                pad_byte: 0,
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

use crate::{Error, image_format::Segment, target::MAX_RAM_BLOCK_SIZE};
#[cfg(feature = "serialport")]
use crate::{
    command::{Command, CommandType},
    connection::Connection,
    target::FlashTarget,
    target::ProgressCallbacks,
};

/// Applications running in the target device's RAM.
#[derive(Copy, Clone, Debug, Hash, PartialEq, Eq)]
pub struct RamTarget {
    entry: Option<u32>,
    block_size: usize,
}

impl RamTarget {
    /// Create a new RAM target.
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
        progress: &mut dyn ProgressCallbacks,
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

        progress.init(addr, num_chunks);

        for (i, block) in chunks.enumerate() {
            connection.command(Command::MemData {
                sequence: i as u32,
                pad_to: 4,
                pad_byte: 0,
                data: block,
            })?;

            progress.update(i + 1)
        }

        progress.finish(false);

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

use bytemuck::{Pod, Zeroable};

pub(crate) use self::ram::MAX_RAM_BLOCK_SIZE;
pub use self::{esp32::Esp32Target, esp8266::Esp8266Target, ram::RamTarget};
#[cfg(feature = "serialport")]
use crate::connection::Connection;
use crate::{elf::RomSegment, error::Error, flasher::ProgressCallbacks};

mod esp32;
mod esp8266;
mod ram;

#[cfg(feature = "serialport")]
/// Operations for interacting with a flash target
pub trait FlashTarget {
    /// Begin the flashing operation
    fn begin(&mut self, connection: &mut Connection) -> Result<(), Error>;

    /// Write a segment to the target device
    fn write_segment(
        &mut self,
        connection: &mut Connection,
        segment: RomSegment,
        progress: &mut Option<&mut dyn ProgressCallbacks>,
    ) -> Result<(), Error>;

    /// Complete the flashing operation
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

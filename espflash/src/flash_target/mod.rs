mod esp32;
mod esp8266;
mod ram;

use crate::connection::Connection;
use crate::elf::RomSegment;
use crate::error::Error;

use bytemuck::{Pod, Zeroable};
pub use esp32::Esp32Target;
pub use esp8266::Esp8266Target;
pub use ram::RamTarget;

pub(crate) use ram::MAX_RAM_BLOCK_SIZE;

pub trait FlashTarget {
    fn begin(&mut self, connection: &mut Connection) -> Result<(), Error>;
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

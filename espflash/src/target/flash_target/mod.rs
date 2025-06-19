pub(crate) use self::ram::MAX_RAM_BLOCK_SIZE;
pub use self::{esp32::Esp32Target, ram::RamTarget};
use crate::{Error, connection::Connection, flasher::ProgressCallbacks, image_format::Segment};

mod esp32;
mod ram;

/// Operations for interacting with a flash target
pub trait FlashTarget {
    /// Begin the flashing operation
    fn begin(&mut self, connection: &mut Connection) -> Result<(), Error>;

    /// Write a segment to the target device
    fn write_segment(
        &mut self,
        connection: &mut Connection,
        segment: Segment<'_>,
        progress: &mut Option<&mut dyn ProgressCallbacks>,
    ) -> Result<(), Error>;

    /// Complete the flashing operation
    fn finish(&mut self, connection: &mut Connection, reboot: bool) -> Result<(), Error>;
}

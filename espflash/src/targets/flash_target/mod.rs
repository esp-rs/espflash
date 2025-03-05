pub(crate) use self::ram::MAX_RAM_BLOCK_SIZE;
pub use self::{esp32::Esp32Target, ram::RamTarget};
use crate::{connection::Connection, image_format::Segment, Error};

mod esp32;
mod ram;

/// Progress update callbacks
pub trait ProgressCallbacks {
    /// Initialize some progress report
    fn init(&mut self, addr: u32, total: usize);
    /// Update some progress report
    fn update(&mut self, current: usize);
    /// Finish some progress report
    fn finish(&mut self);
}

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

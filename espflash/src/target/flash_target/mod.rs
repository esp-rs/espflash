pub use self::{esp32::Esp32Target, ram::RamTarget};
use crate::{Error, connection::Connection, image_format::Segment};

mod esp32;
mod ram;

/// Operations for interacting with a flash target.
pub trait FlashTarget {
    /// Begin the flashing operation.
    fn begin(&mut self, connection: &mut Connection) -> Result<(), Error>;

    /// Write a segment to the target device.
    fn write_segment(
        &mut self,
        connection: &mut Connection,
        segment: Segment<'_>,
        progress: &mut dyn ProgressCallbacks,
    ) -> Result<(), Error>;

    /// Complete the flashing operation.
    fn finish(&mut self, connection: &mut Connection, reboot: bool) -> Result<(), Error>;
}

/// Progress update callbacks.
pub trait ProgressCallbacks {
    /// Initialize some progress report.
    fn init(&mut self, addr: u32, total: usize);
    /// Update some progress report.
    fn update(&mut self, current: usize);
    /// Finish some progress report.
    fn finish(&mut self, skipped: bool);
}

/// An empty implementation of [ProgressCallbacks] that does nothing.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct DefaultProgressCallback;

impl ProgressCallbacks for DefaultProgressCallback {
    fn init(&mut self, _addr: u32, _total: usize) {}
    fn update(&mut self, _current: usize) {}
    fn finish(&mut self, _skipped: bool) {}
}

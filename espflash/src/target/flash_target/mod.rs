//! Flash target module.
//!
//! This module defines the traits and types used for flashing operations on a
//! target device.
//!
//! This module include an `FlashTarget` trait impl for `Esp32Target` and
//! `RamTarget`, enabling the writing of firmware images to the target device's
//! flash memory or static memory (SRAM). It also provides a `ProgressCallbacks`
//! trait which allows for progress updates during the flashing process.`

mod esp32;
mod ram;
pub use self::{esp32::Esp32Target, ram::RamTarget};
use crate::{Error, connection::Connection, image_format::Segment};

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
    /// Indicate post-flash checksum verification has begun.
    fn verifying(&mut self);
    /// Finish some progress report.
    fn finish(&mut self, skipped: bool);
}

/// An empty implementation of [ProgressCallbacks] that does nothing.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct DefaultProgressCallback;

impl ProgressCallbacks for DefaultProgressCallback {
    fn init(&mut self, _addr: u32, _total: usize) {}
    fn update(&mut self, _current: usize) {}
    fn verifying(&mut self) {}
    fn finish(&mut self, _skipped: bool) {}
}

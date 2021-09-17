use crate::connection::Connection;
use crate::elf::{FirmwareImage, RomSegment};
use crate::error::{ConnectionError, Error};
use crate::flash_target::FlashTarget;

pub struct FailOver {
    first: Box<dyn FlashTarget>,
    second: Box<dyn FlashTarget>,
    first_failed: bool,
}

impl FailOver {
    /// Note, this only works for targets that are close enough together
    pub fn new(first: impl FlashTarget + 'static, second: impl FlashTarget + 'static) -> Self {
        FailOver {
            first: Box::new(first),
            second: Box::new(second),
            first_failed: false,
        }
    }

    pub fn active(&mut self) -> &mut dyn FlashTarget {
        if !self.first_failed {
            self.first.as_mut()
        } else {
            self.second.as_mut()
        }
    }

    pub fn fail_over(&mut self, err: Error) -> Result<(), Error> {
        if self.first_failed {
            Err(err)
        } else {
            self.first_failed = true;
            Ok(())
        }
    }
}

impl FlashTarget for FailOver {
    fn begin(&mut self, connection: &mut Connection, image: &FirmwareImage) -> Result<(), Error> {
        self.active().begin(connection, image)
    }

    fn write_segment(
        &mut self,
        connection: &mut Connection,
        segment: &RomSegment,
    ) -> Result<(), Error> {
        match self.active().write_segment(connection, segment) {
            Err(err @ Error::Flashing(ConnectionError::Timeout))
            | Err(err @ Error::Connection(ConnectionError::Timeout)) => {
                self.fail_over(err)?;
                self.active().write_segment(connection, segment)
            }
            res => res,
        }
    }

    fn finish(&mut self, connection: &mut Connection, reboot: bool) -> Result<(), Error> {
        self.active().finish(connection, reboot)
    }
}

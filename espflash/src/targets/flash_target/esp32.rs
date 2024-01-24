use std::io::Write;

use flate2::{
    write::{ZlibDecoder, ZlibEncoder},
    Compression,
};
use log::debug;
use md5::{Digest, Md5};

use crate::{
    command::{Command, CommandType},
    connection::{Connection, USB_SERIAL_JTAG_PID},
    elf::RomSegment,
    error::Error,
    flasher::{ProgressCallbacks, SpiAttachParams, FLASH_SECTOR_SIZE},
    targets::Chip,
    targets::FlashTarget,
};

/// Applications running from an ESP32's (or variant's) flash
pub struct Esp32Target {
    chip: Chip,
    spi_attach_params: SpiAttachParams,
    use_stub: bool,
    verify: bool,
    skip: bool,
    need_deflate_end: bool,
}

impl Esp32Target {
    pub fn new(
        chip: Chip,
        spi_attach_params: SpiAttachParams,
        use_stub: bool,
        verify: bool,
        skip: bool,
    ) -> Self {
        Esp32Target {
            chip,
            spi_attach_params,
            use_stub,
            verify,
            skip,
            need_deflate_end: false,
        }
    }
}

impl FlashTarget for Esp32Target {
    fn begin(&mut self, connection: &mut Connection) -> Result<(), Error> {
        connection.with_timeout(CommandType::SpiAttach.timeout(), |connection| {
            let command = if self.use_stub {
                Command::SpiAttachStub {
                    spi_params: self.spi_attach_params,
                }
            } else {
                Command::SpiAttach {
                    spi_params: self.spi_attach_params,
                }
            };

            connection.command(command)
        })?;

        // The stub usually disables these watchdog timers, however if we're not using the stub
        // we need to disable them before flashing begins
        // TODO: the stub doesn't appear to disable the watchdog on ESP32-S3, so we explicitly
        //       disable the watchdog here.
        if connection.get_usb_pid()? == USB_SERIAL_JTAG_PID {
            match self.chip {
                Chip::Esp32c3 => {
                    connection.command(Command::WriteReg {
                        address: 0x6000_80a8,
                        value: 0x50D8_3AA1,
                        mask: None,
                    })?; // WP disable
                    connection.command(Command::WriteReg {
                        address: 0x6000_8090,
                        value: 0x0,
                        mask: None,
                    })?; // turn off RTC WDT
                    connection.command(Command::WriteReg {
                        address: 0x6000_80a8,
                        value: 0x0,
                        mask: None,
                    })?; // WP enable
                }
                Chip::Esp32s3 => {
                    connection.command(Command::WriteReg {
                        address: 0x6000_80B0,
                        value: 0x50D8_3AA1,
                        mask: None,
                    })?; // WP disable
                    connection.command(Command::WriteReg {
                        address: 0x6000_8098,
                        value: 0x0,
                        mask: None,
                    })?; // turn off RTC WDT
                    connection.command(Command::WriteReg {
                        address: 0x6000_80B0,
                        value: 0x0,
                        mask: None,
                    })?; // WP enable
                }
                Chip::Esp32c6 => {
                    connection.command(Command::WriteReg {
                        address: 0x600B_1C18,
                        value: 0x50D8_3AA1,
                        mask: None,
                    })?; // WP disable
                    connection.command(Command::WriteReg {
                        address: 0x600B_1C00,
                        value: 0x0,
                        mask: None,
                    })?; // turn off RTC WDT
                    connection.command(Command::WriteReg {
                        address: 0x600B_1C18,
                        value: 0x0,
                        mask: None,
                    })?; // WP enable
                }
                _ => {}
            }
        }

        Ok(())
    }

    fn write_segment(
        &mut self,
        connection: &mut Connection,
        segment: RomSegment,
        progress: &mut Option<&mut dyn ProgressCallbacks>,
    ) -> Result<(), Error> {
        let addr = segment.addr;

        let mut md5_hasher = Md5::new();
        md5_hasher.update(&segment.data);
        let checksum_md5 = md5_hasher.finalize();

        if self.skip {
            let flash_checksum_md5: u128 =
                connection.with_timeout(CommandType::FlashMd5.timeout(), |connection| {
                    connection
                        .command(crate::command::Command::FlashMd5 {
                            offset: addr,
                            size: segment.data.len() as u32,
                        })?
                        .try_into()
                })?;

            if checksum_md5.as_slice() == flash_checksum_md5.to_be_bytes() {
                debug!("Skipping segment at address {:x}", addr);
                return Ok(());
            }
        }

        let mut encoder = ZlibEncoder::new(Vec::new(), Compression::best());
        encoder.write_all(&segment.data)?;
        let compressed = encoder.finish()?;

        let target = self.chip.into_target();
        let flash_write_size = target.flash_write_size(connection)?;
        let block_count = (compressed.len() + flash_write_size - 1) / flash_write_size;
        let erase_count = (segment.data.len() + FLASH_SECTOR_SIZE - 1) / FLASH_SECTOR_SIZE;

        // round up to sector size
        let erase_size = (erase_count * FLASH_SECTOR_SIZE) as u32;

        connection.with_timeout(
            CommandType::FlashDeflateBegin.timeout_for_size(erase_size),
            |connection| {
                connection.command(Command::FlashDeflateBegin {
                    size: segment.data.len() as u32,
                    blocks: block_count as u32,
                    block_size: flash_write_size as u32,
                    offset: addr,
                    supports_encryption: self.chip != Chip::Esp32 && !self.use_stub,
                })?;
                Ok(())
            },
        )?;
        self.need_deflate_end = true;

        let chunks = compressed.chunks(flash_write_size);
        let num_chunks = chunks.len();

        if let Some(cb) = progress.as_mut() {
            cb.init(addr, num_chunks)
        }

        // decode the chunks to see how much data the device will have to save
        let mut decoder = ZlibDecoder::new(Vec::new());
        let mut decoded_size = 0;

        for (i, block) in chunks.enumerate() {
            decoder.write_all(block)?;
            decoder.flush()?;
            let size = decoder.get_ref().len() - decoded_size;
            decoded_size = decoder.get_ref().len();

            connection.with_timeout(
                CommandType::FlashDeflateData.timeout_for_size(size as u32),
                |connection| {
                    connection.command(Command::FlashDeflateData {
                        sequence: i as u32,
                        pad_to: 0,
                        pad_byte: 0xff,
                        data: block,
                    })?;
                    Ok(())
                },
            )?;

            if let Some(cb) = progress.as_mut() {
                cb.update(i + 1)
            }
        }

        if let Some(cb) = progress.as_mut() {
            cb.finish()
        }

        if self.verify {
            let flash_checksum_md5: u128 =
                connection.with_timeout(CommandType::FlashMd5.timeout(), |connection| {
                    connection
                        .command(crate::command::Command::FlashMd5 {
                            offset: addr,
                            size: segment.data.len() as u32,
                        })?
                        .try_into()
                })?;

            if checksum_md5.as_slice() != flash_checksum_md5.to_be_bytes() {
                return Err(Error::VerifyFailed);
            }
        }

        Ok(())
    }

    fn finish(&mut self, connection: &mut Connection, reboot: bool) -> Result<(), Error> {
        if self.need_deflate_end {
            connection.with_timeout(CommandType::FlashDeflateEnd.timeout(), |connection| {
                connection.command(Command::FlashDeflateEnd { reboot: false })
            })?;
        }

        if reboot {
            connection.reset_after(self.use_stub, self.chip)?;
        }

        Ok(())
    }
}

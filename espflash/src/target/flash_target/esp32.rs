use std::io::Write;

use flate2::{
    Compression,
    write::{ZlibDecoder, ZlibEncoder},
};
use log::debug;
use md5::{Digest, Md5};

use crate::{
    Error,
    flasher::{FLASH_SECTOR_SIZE, SpiAttachParams},
    image_format::Segment,
    target::{Chip, WDT_WKEY},
};
#[cfg(feature = "serialport")]
use crate::{
    command::{Command, CommandType},
    connection::Connection,
    target::FlashTarget,
    target::ProgressCallbacks,
};

/// Applications running from an ESP32's (or variant's) flash
#[derive(Debug)]
pub struct Esp32Target {
    chip: Chip,
    spi_attach_params: SpiAttachParams,
    use_stub: bool,
    verify: bool,
    skip: bool,
    need_deflate_end: bool,
}

impl Esp32Target {
    /// Create a new ESP32 target.
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

#[cfg(feature = "serialport")]
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

        // The stub usually disables these watchdog timers, however if we're not using
        // the stub we need to disable them before flashing begins.
        //
        // TODO: the stub doesn't appear to disable the watchdog on ESP32-S3, so we
        //       explicitly disable the watchdog here.
        if connection.is_using_usb_serial_jtag() {
            if let (Some(wdt_wprotect), Some(wdt_config0)) =
                (self.chip.wdt_wprotect(), self.chip.wdt_config0())
            {
                connection.command(Command::WriteReg {
                    address: wdt_wprotect,
                    value: WDT_WKEY,
                    mask: None,
                })?; // WP disable
                connection.command(Command::WriteReg {
                    address: wdt_config0,
                    value: 0x0,
                    mask: None,
                })?; // turn off RTC WDT
                connection.command(Command::WriteReg {
                    address: wdt_wprotect,
                    value: 0x0,
                    mask: None,
                })?; // WP enable
            }
        }

        Ok(())
    }

    fn write_segment(
        &mut self,
        connection: &mut Connection,
        segment: Segment<'_>,
        progress: &mut dyn ProgressCallbacks,
    ) -> Result<(), Error> {
        let addr = segment.addr;

        let mut md5_hasher = Md5::new();
        md5_hasher.update(&segment.data);
        let checksum_md5 = md5_hasher.finalize();

        let mut encoder = ZlibEncoder::new(Vec::new(), Compression::best());
        encoder.write_all(&segment.data)?;
        let compressed = encoder.finish()?;

        let flash_write_size = self.chip.flash_write_size();
        let block_count = compressed.len().div_ceil(flash_write_size);
        let erase_count = segment.data.len().div_ceil(FLASH_SECTOR_SIZE);

        // round up to sector size
        let erase_size = (erase_count * FLASH_SECTOR_SIZE) as u32;

        let chunks = compressed.chunks(flash_write_size);
        let num_chunks = chunks.len();

        progress.init(addr, num_chunks + self.verify as usize);

        if self.skip {
            let flash_checksum_md5: u128 = connection.with_timeout(
                CommandType::FlashMd5.timeout_for_size(segment.data.len() as u32),
                |connection| {
                    connection
                        .command(Command::FlashMd5 {
                            offset: addr,
                            size: segment.data.len() as u32,
                        })?
                        .try_into()
                },
            )?;

            if checksum_md5.as_slice() == flash_checksum_md5.to_be_bytes() {
                debug!("Segment at address '0x{addr:x}' has not changed, skipping write");

                progress.finish(true);
                return Ok(());
            }
        }

        connection.with_timeout(
            CommandType::FlashDeflBegin.timeout_for_size(erase_size),
            |connection| {
                connection.command(Command::FlashDeflBegin {
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

        // decode the chunks to see how much data the device will have to save
        let mut decoder = ZlibDecoder::new(Vec::new());
        let mut decoded_size = 0;

        for (i, block) in chunks.enumerate() {
            decoder.write_all(block)?;
            decoder.flush()?;
            let size = decoder.get_ref().len() - decoded_size;
            decoded_size = decoder.get_ref().len();

            connection.with_timeout(
                CommandType::FlashDeflData.timeout_for_size(size as u32),
                |connection| {
                    connection.command(Command::FlashDeflData {
                        sequence: i as u32,
                        pad_to: 0,
                        pad_byte: 0xff,
                        data: block,
                    })?;
                    Ok(())
                },
            )?;

            progress.update(i + 1)
        }

        if self.verify {
            let flash_checksum_md5: u128 = connection.with_timeout(
                CommandType::FlashMd5.timeout_for_size(segment.data.len() as u32),
                |connection| {
                    connection
                        .command(Command::FlashMd5 {
                            offset: addr,
                            size: segment.data.len() as u32,
                        })?
                        .try_into()
                },
            )?;

            if checksum_md5.as_slice() != flash_checksum_md5.to_be_bytes() {
                return Err(Error::VerifyFailed);
            }
            debug!("Segment at address '0x{addr:x}' verified successfully");
            progress.update(num_chunks + 1)
        }

        progress.finish(false);

        Ok(())
    }

    fn finish(&mut self, connection: &mut Connection, reboot: bool) -> Result<(), Error> {
        if self.need_deflate_end {
            connection.with_timeout(CommandType::FlashDeflEnd.timeout(), |connection| {
                connection.command(Command::FlashDeflEnd { reboot: false })
            })?;
        }

        if reboot {
            connection.reset_after(self.use_stub, self.chip)?;
        }

        Ok(())
    }
}

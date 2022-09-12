use crate::command::{Command, CommandType};
use crate::connection::{Connection, USB_SERIAL_JTAG_PID};
use crate::elf::RomSegment;
use crate::error::Error;
use crate::flash_target::FlashTarget;
use crate::flasher::{SpiAttachParams, FLASH_SECTOR_SIZE};
use crate::Chip;
use flate2::write::{ZlibDecoder, ZlibEncoder};
use flate2::Compression;
use indicatif::{ProgressBar, ProgressStyle};
use std::io::Write;

pub struct Esp32Target {
    chip: Chip,
    spi_attach_params: SpiAttachParams,
    use_stub: bool,
}

impl Esp32Target {
    pub fn new(chip: Chip, spi_attach_params: SpiAttachParams, use_stub: bool) -> Self {
        Esp32Target {
            chip,
            spi_attach_params,
            use_stub,
        }
    }
}

impl FlashTarget for Esp32Target {
    fn begin(&mut self, connection: &mut Connection) -> Result<(), Error> {
        connection.with_timeout(CommandType::SpiAttach.timeout(), |connection| {
            connection.command(if self.use_stub {
                Command::SpiAttachStub {
                    spi_params: self.spi_attach_params,
                }
            } else {
                Command::SpiAttach {
                    spi_params: self.spi_attach_params,
                }
            })
        })?;

        // TODO remove this when we use the stub, the stub should be taking care of this.
        // TODO do we also need to disable rtc super wdt?
        if connection.get_usb_pid()? == USB_SERIAL_JTAG_PID {
            match self.chip {
                Chip::Esp32c3 => {
                    connection.command(Command::WriteReg {
                        address: 0x600080a8,
                        value: 0x50D83AA1u32,
                        mask: None,
                    })?; // WP disable
                    connection.command(Command::WriteReg {
                        address: 0x60008090,
                        value: 0x0,
                        mask: None,
                    })?; // turn off RTC WDG
                    connection.command(Command::WriteReg {
                        address: 0x600080a8,
                        value: 0x0,
                        mask: None,
                    })?; // WP enable
                }
                Chip::Esp32s3 => {
                    connection.command(Command::WriteReg {
                        address: 0x6000_80B0,
                        value: 0x50D83AA1u32,
                        mask: None,
                    })?; // WP disable
                    connection.command(Command::WriteReg {
                        address: 0x6000_8098,
                        value: 0x0,
                        mask: None,
                    })?; // turn off RTC WDG
                    connection.command(Command::WriteReg {
                        address: 0x6000_80B0,
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
    ) -> Result<(), Error> {
        let addr = segment.addr;
        let mut encoder = ZlibEncoder::new(Vec::new(), Compression::best());
        encoder.write_all(&segment.data)?;
        let compressed = encoder.finish()?;
        let flash_write_size = self.chip.flash_write_size(connection)?;
        let block_count = (compressed.len() + flash_write_size - 1) / flash_write_size;
        let erase_count = (segment.data.len() + FLASH_SECTOR_SIZE - 1) / FLASH_SECTOR_SIZE;

        // round up to sector size
        let erase_size = (erase_count * FLASH_SECTOR_SIZE) as u32;

        connection.with_timeout(
            CommandType::FlashDeflateBegin.timeout_for_size(erase_size),
            |connection| {
                connection.command(Command::FlashDeflateBegin {
                    size: erase_size,
                    blocks: block_count as u32,
                    block_size: flash_write_size as u32,
                    offset: addr,
                    supports_encryption: self.chip != Chip::Esp32 && !self.use_stub,
                })?;
                Ok(())
            },
        )?;

        let chunks = compressed.chunks(flash_write_size);

        let (_, chunk_size) = chunks.size_hint();
        let chunk_size = chunk_size.unwrap_or(0) as u64;
        let pb_chunk = ProgressBar::new(chunk_size);
        pb_chunk.set_style(
            ProgressStyle::default_bar()
                .template("[{elapsed_precise}] {bar:40.cyan/blue} {pos:>7}/{len:7} {msg}")
                .unwrap()
                .progress_chars("#>-"),
        );

        // decode the chunks to see how much data the device will have to save
        let mut decoder = ZlibDecoder::new(Vec::new());
        let mut decoded_size = 0;

        for (i, block) in chunks.enumerate() {
            decoder.write_all(block)?;
            decoder.flush()?;
            let size = decoder.get_ref().len() - decoded_size;
            decoded_size = decoder.get_ref().len();

            pb_chunk.set_message(format!("segment 0x{:X} writing chunks", addr));
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
            pb_chunk.inc(1);
        }

        pb_chunk.finish_with_message(format!("segment 0x{:X}", addr));

        Ok(())
    }

    fn finish(&mut self, connection: &mut Connection, reboot: bool) -> Result<(), Error> {
        connection.with_timeout(CommandType::FlashDeflateEnd.timeout(), |connection| {
            connection.command(Command::FlashDeflateEnd { reboot: false })
        })?;
        if reboot {
            connection.reset()
        } else {
            Ok(())
        }
    }
}

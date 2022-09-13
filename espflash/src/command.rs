use crate::flasher::{checksum, SpiAttachParams, CHECKSUM_INIT};
use bytemuck::{bytes_of, Pod, Zeroable};
use std::io::Write;
use std::mem::size_of;
use std::time::Duration;
use strum_macros::Display;

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(3);
const ERASE_REGION_TIMEOUT_PER_MB: Duration = Duration::from_secs(30);
const ERASE_WRITE_TIMEOUT_PER_MB: Duration = Duration::from_secs(40);
const ERASE_CHIP_TIMEOUT: Duration = Duration::from_secs(120);
const MEM_END_TIMEOUT: Duration = Duration::from_millis(50);
const SYNC_TIMEOUT: Duration = Duration::from_millis(100);

#[derive(Copy, Clone, Debug, Display)]
#[allow(dead_code)]
#[repr(u8)]
#[non_exhaustive]
pub enum CommandType {
    Unknown = 0,
    FlashBegin = 0x02,
    FlashData = 0x03,
    FlashEnd = 0x04,
    MemBegin = 0x05,
    MemEnd = 0x06,
    MemData = 0x07,
    Sync = 0x08,
    WriteReg = 0x09,
    ReadReg = 0x0a,
    SpiSetParams = 0x0B,
    SpiAttach = 0x0D,
    ChangeBaud = 0x0F,
    FlashDeflateBegin = 0x10,
    FlashDeflateData = 0x11,
    FlashDeflateEnd = 0x12,
    FlashMd5 = 0x13,
    FlashDetect = 0x9f,
    // Some commands supported by stub only
    EraseFlash = 0xd0,
    EraseRegion = 0xd1,
}

impl CommandType {
    pub fn timeout(&self) -> Duration {
        match self {
            CommandType::MemEnd => MEM_END_TIMEOUT,
            CommandType::Sync => SYNC_TIMEOUT,
            CommandType::EraseFlash => ERASE_CHIP_TIMEOUT,
            _ => DEFAULT_TIMEOUT,
        }
    }

    pub fn timeout_for_size(&self, size: u32) -> Duration {
        fn calc_timeout(timeout_per_mb: Duration, size: u32) -> Duration {
            let mb = size as f64 / 1_000_000.0;
            std::cmp::max(
                DEFAULT_TIMEOUT,
                Duration::from_millis((timeout_per_mb.as_millis() as f64 * mb) as u64),
            )
        }
        match self {
            CommandType::FlashBegin | CommandType::FlashDeflateBegin | CommandType::EraseRegion => {
                calc_timeout(ERASE_REGION_TIMEOUT_PER_MB, size)
            }
            CommandType::FlashData | CommandType::FlashDeflateData => {
                calc_timeout(ERASE_WRITE_TIMEOUT_PER_MB, size)
            }
            _ => self.timeout(),
        }
    }
}

#[derive(Copy, Clone, Debug)]
pub enum Command<'a> {
    FlashBegin {
        size: u32,
        blocks: u32,
        block_size: u32,
        offset: u32,
        supports_encryption: bool,
    },
    FlashData {
        data: &'a [u8],
        pad_to: usize,
        pad_byte: u8,
        sequence: u32,
    },
    FlashEnd {
        reboot: bool,
    },
    MemBegin {
        size: u32,
        blocks: u32,
        block_size: u32,
        offset: u32,
        supports_encryption: bool,
    },
    MemData {
        data: &'a [u8],
        pad_to: usize,
        pad_byte: u8,
        sequence: u32,
    },
    MemEnd {
        no_entry: bool,
        entry: u32,
    },
    Sync,
    WriteReg {
        address: u32,
        value: u32,
        mask: Option<u32>,
    },
    ReadReg {
        address: u32,
    },
    SpiAttach {
        spi_params: SpiAttachParams,
    },
    SpiAttachStub {
        spi_params: SpiAttachParams,
    },
    ChangeBaud {
        /// New baud rate
        new_baud: u32,
        /// Prior baud rate ('0' for ROM flasher)
        prior_baud: u32,
    },
    FlashDeflateBegin {
        size: u32,
        blocks: u32,
        block_size: u32,
        offset: u32,
        supports_encryption: bool,
    },
    FlashDeflateData {
        data: &'a [u8],
        pad_to: usize,
        pad_byte: u8,
        sequence: u32,
    },
    FlashDeflateEnd {
        reboot: bool,
    },
    FlashDetect,
    EraseFlash,
    EraseRegion {
        offset: u32,
        size: u32,
    },
}

impl<'a> Command<'a> {
    pub fn command_type(&self) -> CommandType {
        match self {
            Command::FlashBegin { .. } => CommandType::FlashBegin,
            Command::FlashData { .. } => CommandType::FlashData,
            Command::FlashEnd { .. } => CommandType::FlashEnd,
            Command::MemBegin { .. } => CommandType::MemBegin,
            Command::MemData { .. } => CommandType::MemData,
            Command::MemEnd { .. } => CommandType::MemEnd,
            Command::Sync => CommandType::Sync,
            Command::WriteReg { .. } => CommandType::WriteReg,
            Command::ReadReg { .. } => CommandType::ReadReg,
            Command::SpiAttach { .. } => CommandType::SpiAttach,
            Command::SpiAttachStub { .. } => CommandType::SpiAttach,
            Command::ChangeBaud { .. } => CommandType::ChangeBaud,
            Command::FlashDeflateBegin { .. } => CommandType::FlashDeflateBegin,
            Command::FlashDeflateData { .. } => CommandType::FlashDeflateData,
            Command::FlashDeflateEnd { .. } => CommandType::FlashDeflateEnd,
            Command::FlashDetect => CommandType::FlashDetect,
            Command::EraseFlash { .. } => CommandType::EraseFlash,
            Command::EraseRegion { .. } => CommandType::EraseRegion,
        }
    }

    pub fn timeout_for_size(&self, size: u32) -> Duration {
        self.command_type().timeout_for_size(size)
    }

    pub fn write<W: Write>(&self, mut writer: W) -> std::io::Result<()> {
        writer.write_all(&[0, self.command_type() as u8])?;
        match *self {
            Command::FlashBegin {
                size,
                blocks,
                block_size,
                offset,
                supports_encryption,
            } => {
                begin_command(
                    writer,
                    size,
                    blocks,
                    block_size,
                    offset,
                    supports_encryption,
                )?;
            }
            Command::FlashData {
                pad_to,
                pad_byte,
                data,
                sequence,
            } => {
                data_command(writer, data, pad_to, pad_byte, sequence)?;
            }
            Command::FlashEnd { reboot } => {
                write_basic(writer, &[if reboot { 0 } else { 1 }], 0)?;
            }
            Command::MemBegin {
                size,
                blocks,
                block_size,
                offset,
                supports_encryption,
            } => {
                begin_command(
                    writer,
                    size,
                    blocks,
                    block_size,
                    offset,
                    supports_encryption,
                )?;
            }
            Command::MemData {
                pad_to,
                pad_byte,
                data,
                sequence,
            } => {
                data_command(writer, data, pad_to, pad_byte, sequence)?;
            }
            Command::MemEnd {
                no_entry: reboot,
                entry,
            } => {
                #[derive(Zeroable, Pod, Copy, Clone)]
                #[repr(C)]
                struct EntryParams {
                    no_entry: u32,
                    entry: u32,
                }
                let params = EntryParams {
                    no_entry: if reboot { 1 } else { 0 },
                    entry,
                };
                write_basic(writer, bytes_of(&params), 0)?;
            }
            Command::Sync => {
                write_basic(
                    writer,
                    &[
                        0x07, 0x07, 0x12, 0x20, 0x55, 0x55, 0x55, 0x55, 0x55, 0x55, 0x55, 0x55,
                        0x55, 0x55, 0x55, 0x55, 0x55, 0x55, 0x55, 0x55, 0x55, 0x55, 0x55, 0x55,
                        0x55, 0x55, 0x55, 0x55, 0x55, 0x55, 0x55, 0x55, 0x55, 0x55, 0x55, 0x55,
                    ],
                    0,
                )?;
            }
            Command::WriteReg {
                address,
                value,
                mask,
            } => {
                #[derive(Zeroable, Pod, Copy, Clone, Debug)]
                #[repr(C)]
                struct WriteRegParams {
                    addr: u32,
                    value: u32,
                    mask: u32,
                    delay_us: u32,
                }
                let params = WriteRegParams {
                    addr: address,
                    value,
                    mask: mask.unwrap_or(0xFFFFFFFF),
                    delay_us: 0,
                };
                write_basic(writer, bytes_of(&params), 0)?;
            }
            Command::ReadReg { address } => {
                write_basic(writer, &address.to_le_bytes(), 0)?;
            }
            Command::SpiAttach { spi_params } => {
                write_basic(writer, &spi_params.encode(false), 0)?;
            }
            Command::SpiAttachStub { spi_params } => {
                write_basic(writer, &spi_params.encode(true), 0)?;
            }
            Command::ChangeBaud {
                new_baud,
                prior_baud,
            } => {
                // length
                writer.write_all(&(8u16.to_le_bytes()))?;
                // checksum
                writer.write_all(&(0u32.to_le_bytes()))?;
                // data
                writer.write_all(&new_baud.to_le_bytes())?;
                writer.write_all(&prior_baud.to_le_bytes())?;
            }
            Command::FlashDeflateBegin {
                size,
                blocks,
                block_size,
                offset,
                supports_encryption,
            } => {
                begin_command(
                    writer,
                    size,
                    blocks,
                    block_size,
                    offset,
                    supports_encryption,
                )?;
            }
            Command::FlashDeflateData {
                pad_to,
                pad_byte,
                data,
                sequence,
            } => {
                data_command(writer, data, pad_to, pad_byte, sequence)?;
            }
            Command::FlashDeflateEnd { reboot } => {
                write_basic(writer, &[if reboot { 0 } else { 1 }], 0)?;
            }
            Command::FlashDetect => {
                write_basic(writer, &[], 0)?;
            }
            Command::EraseFlash => {
                write_basic(writer, &[], 0)?;
            }
            Command::EraseRegion { offset, size } => {
                // length
                writer.write_all(&(8u16.to_le_bytes()))?;
                // checksum
                writer.write_all(&(0u32.to_le_bytes()))?;
                // data
                writer.write_all(&offset.to_le_bytes())?;
                writer.write_all(&size.to_le_bytes())?;
            }
        };
        Ok(())
    }
}

fn write_basic<W: Write>(mut writer: W, data: &[u8], checksum: u32) -> std::io::Result<()> {
    writer.write_all(&((data.len() as u16).to_le_bytes()))?;
    writer.write_all(&(checksum.to_le_bytes()))?;
    writer.write_all(data)?;
    Ok(())
}

fn begin_command<W: Write>(
    writer: W,
    size: u32,
    blocks: u32,
    block_size: u32,
    offset: u32,
    supports_encryption: bool,
) -> std::io::Result<()> {
    #[derive(Zeroable, Pod, Copy, Clone, Debug)]
    #[repr(C)]
    struct BeginParams {
        size: u32,
        blocks: u32,
        block_size: u32,
        offset: u32,
        encrypted: u32,
    }
    let params = BeginParams {
        size,
        blocks,
        block_size,
        offset,
        encrypted: 0,
    };

    let bytes = bytes_of(&params);
    let data = if !supports_encryption {
        // The ESP32 and ESP8266 do not take the `encrypted` field, so truncate the last
        // 4 bytes of the slice where it resides.
        let end = bytes.len() - 4;
        &bytes[0..end]
    } else {
        bytes
    };
    write_basic(writer, data, 0)
}

fn data_command<W: Write>(
    mut writer: W,
    block_data: &[u8],
    pad_to: usize,
    pad_byte: u8,
    sequence: u32,
) -> std::io::Result<()> {
    #[derive(Zeroable, Pod, Copy, Clone, Debug)]
    #[repr(C)]
    struct BlockParams {
        size: u32,
        sequence: u32,
        dummy1: u32,
        dummy2: u32,
    }

    let pad_length = pad_to.saturating_sub(block_data.len());

    let params = BlockParams {
        size: (block_data.len() + pad_length) as u32,
        sequence,
        dummy1: 0,
        dummy2: 0,
    };

    let mut check = checksum(block_data, CHECKSUM_INIT);

    for _ in 0..pad_length {
        check = checksum(&[pad_byte], check);
    }

    let total_length = size_of::<BlockParams>() + block_data.len() + pad_length;
    writer.write_all(&((total_length as u16).to_le_bytes()))?;
    writer.write_all(&((check as u32).to_le_bytes()))?;
    writer.write_all(bytes_of(&params))?;
    writer.write_all(block_data)?;
    for _ in 0..pad_length {
        writer.write_all(&[pad_byte])?;
    }
    Ok(())
}

//! Commands to work with a flasher stub running on a target device

use std::{io::Write, mem::size_of, time::Duration};

use bytemuck::{bytes_of, Pod, Zeroable};
use strum::Display;

use crate::flasher::{SpiAttachParams, SpiSetParams};

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(3);
const ERASE_REGION_TIMEOUT_PER_MB: Duration = Duration::from_secs(30);
const ERASE_WRITE_TIMEOUT_PER_MB: Duration = Duration::from_secs(40);
const ERASE_CHIP_TIMEOUT: Duration = Duration::from_secs(120);
const MEM_END_TIMEOUT: Duration = Duration::from_millis(50);
const SYNC_TIMEOUT: Duration = Duration::from_millis(100);
const FLASH_DEFLATE_END_TIMEOUT: Duration = Duration::from_secs(10);
const FLASH_MD5_TIMEOUT: Duration = Duration::from_secs(8);

/// Input data for SYNC command (36 bytes: 0x07 0x07 0x12 0x20, followed by
/// 32 x 0x55)
const SYNC_FRAME: [u8; 36] = [
    0x07, 0x07, 0x12, 0x20, 0x55, 0x55, 0x55, 0x55, 0x55, 0x55, 0x55, 0x55, 0x55, 0x55, 0x55, 0x55,
    0x55, 0x55, 0x55, 0x55, 0x55, 0x55, 0x55, 0x55, 0x55, 0x55, 0x55, 0x55, 0x55, 0x55, 0x55, 0x55,
    0x55, 0x55, 0x55, 0x55,
];

/// Types of commands that can be sent to a target device
///
/// https://docs.espressif.com/projects/esptool/en/latest/esp32c3/advanced-topics/serial-protocol.html#supported-by-stub-loader-and-rom-loader
#[derive(Copy, Clone, Debug, Display)]
#[non_exhaustive]
#[repr(u8)]
pub enum CommandType {
    Unknown = 0,
    // Commands supported by the ESP32's bootloaders
    FlashBegin = 0x02,
    FlashData = 0x03,
    FlashEnd = 0x04,
    MemBegin = 0x05,
    MemEnd = 0x06,
    MemData = 0x07,
    Sync = 0x08,
    WriteReg = 0x09,
    ReadReg = 0x0A,
    // Commands supported by the ESP32s bootloaders
    SpiSetParams = 0x0B,
    SpiAttach = 0x0D,
    ChangeBaudrate = 0x0F,
    FlashDeflBegin = 0x10,
    FlashDeflData = 0x11,
    FlashDeflEnd = 0x12,
    FlashMd5 = 0x13,
    // Not supported on ESP32
    GetSecurityInfo = 0x14,
    // Stub-only commands
    EraseFlash = 0xD0,
    EraseRegion = 0xD1,
    ReadFlash = 0xD2,
    RunUserCode = 0xD3,
    // Flash encryption debug mode supported command
    FlashEncryptedData = 0xD4,
    // Read SPI flash manufacturer and device id - Not part of the protocol
    FlashDetect = 0x9F,
}

impl CommandType {
    /// Return a timeout based on the command type
    pub fn timeout(&self) -> Duration {
        match self {
            CommandType::MemEnd => MEM_END_TIMEOUT,
            CommandType::Sync => SYNC_TIMEOUT,
            CommandType::EraseFlash => ERASE_CHIP_TIMEOUT,
            CommandType::FlashDeflEnd => FLASH_DEFLATE_END_TIMEOUT,
            CommandType::FlashMd5 => FLASH_MD5_TIMEOUT,
            _ => DEFAULT_TIMEOUT,
        }
    }

    /// Return a timeout based on the size
    pub fn timeout_for_size(&self, size: u32) -> Duration {
        fn calc_timeout(timeout_per_mb: Duration, size: u32) -> Duration {
            let mb = size as f64 / 1_000_000.0;
            std::cmp::max(
                FLASH_DEFLATE_END_TIMEOUT,
                Duration::from_millis((timeout_per_mb.as_millis() as f64 * mb) as u64),
            )
        }
        match self {
            CommandType::FlashBegin | CommandType::FlashDeflBegin | CommandType::EraseRegion => {
                calc_timeout(ERASE_REGION_TIMEOUT_PER_MB, size)
            }
            CommandType::FlashData | CommandType::FlashDeflData => {
                calc_timeout(ERASE_WRITE_TIMEOUT_PER_MB, size)
            }
            _ => self.timeout(),
        }
    }
}

/// Available commands
#[derive(Copy, Clone, Debug)]
#[non_exhaustive]
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
    MemEnd {
        no_entry: bool,
        entry: u32,
    },
    MemData {
        data: &'a [u8],
        pad_to: usize,
        pad_byte: u8,
        sequence: u32,
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
    SpiSetParams {
        spi_params: SpiSetParams,
    },
    SpiAttach {
        spi_params: SpiAttachParams,
    },
    SpiAttachStub {
        spi_params: SpiAttachParams,
    },
    ChangeBaudrate {
        /// New baud rate
        new_baud: u32,
        /// Prior baud rate ('0' for ROM flasher)
        prior_baud: u32,
    },
    FlashDeflBegin {
        size: u32,
        blocks: u32,
        block_size: u32,
        offset: u32,
        supports_encryption: bool,
    },
    FlashDeflData {
        data: &'a [u8],
        pad_to: usize,
        pad_byte: u8,
        sequence: u32,
    },
    FlashDeflEnd {
        reboot: bool,
    },
    FlashMd5 {
        offset: u32,
        size: u32,
    },
    EraseFlash,
    EraseRegion {
        offset: u32,
        size: u32,
    },
    ReadFlash {
        offset: u32,
        size: u32,
        block_size: u32,
        max_in_flight: u32,
    },
    RunUserCode,
    FlashDetect,
    GetSecurityInfo,
}

impl Command<'_> {
    /// Return the command type
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
            Command::SpiSetParams { .. } => CommandType::SpiSetParams,
            Command::SpiAttach { .. } => CommandType::SpiAttach,
            Command::SpiAttachStub { .. } => CommandType::SpiAttach,
            Command::ChangeBaudrate { .. } => CommandType::ChangeBaudrate,
            Command::FlashDeflBegin { .. } => CommandType::FlashDeflBegin,
            Command::FlashDeflData { .. } => CommandType::FlashDeflData,
            Command::FlashDeflEnd { .. } => CommandType::FlashDeflEnd,
            Command::FlashMd5 { .. } => CommandType::FlashMd5,
            Command::EraseFlash { .. } => CommandType::EraseFlash,
            Command::EraseRegion { .. } => CommandType::EraseRegion,
            Command::ReadFlash { .. } => CommandType::ReadFlash,
            Command::RunUserCode { .. } => CommandType::RunUserCode,
            Command::FlashDetect => CommandType::FlashDetect,
            Command::GetSecurityInfo => CommandType::GetSecurityInfo,
        }
    }

    /// Return a timeout based on the size
    pub fn timeout_for_size(&self, size: u32) -> Duration {
        self.command_type().timeout_for_size(size)
    }

    /// Write a command
    pub fn write<W: Write>(&self, mut writer: W) -> std::io::Result<()> {
        // Write the Direction and Command Indentifier
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
                write_basic(writer, &[u8::from(!reboot)], 0)?;
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
                    no_entry: u32::from(reboot),
                    entry,
                };
                write_basic(writer, bytes_of(&params), 0)?;
            }
            Command::Sync => {
                write_basic(writer, &SYNC_FRAME, 0)?;
            }
            Command::WriteReg {
                address,
                value,
                mask,
            } => {
                #[derive(Zeroable, Pod, Copy, Clone, Debug)]
                #[repr(C)]
                struct WriteRegParams {
                    address: u32,
                    value: u32,
                    mask: u32,
                    delay_us: u32,
                }
                let params = WriteRegParams {
                    address,
                    value,
                    mask: mask.unwrap_or(0xFFFFFFFF),
                    delay_us: 0,
                };
                write_basic(writer, bytes_of(&params), 0)?;
            }
            Command::ReadReg { address } => {
                write_basic(writer, &address.to_le_bytes(), 0)?;
            }
            Command::SpiSetParams { spi_params } => {
                write_basic(writer, &spi_params.encode(), 0)?;
            }
            Command::SpiAttach { spi_params } => {
                write_basic(writer, &spi_params.encode(false), 0)?;
            }
            Command::SpiAttachStub { spi_params } => {
                write_basic(writer, &spi_params.encode(true), 0)?;
            }
            Command::ChangeBaudrate {
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
            Command::FlashDeflBegin {
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
            Command::FlashDeflData {
                pad_to,
                pad_byte,
                data,
                sequence,
            } => {
                data_command(writer, data, pad_to, pad_byte, sequence)?;
            }
            Command::FlashDeflEnd { reboot } => {
                // As per the logic here: https://github.com/espressif/esptool/blob/0a9caaf04cfde6fd97c785d4811f3fde09b1b71f/flasher_stub/stub_flasher.c#L402
                // 0 means reboot, 1 means do nothing
                write_basic(writer, &[u8::from(!reboot)], 0)?;
            }
            Command::FlashMd5 { offset, size } => {
                // length
                writer.write_all(&(16u16.to_le_bytes()))?;
                // checksum
                writer.write_all(&(0u32.to_le_bytes()))?;
                // data
                writer.write_all(&offset.to_le_bytes())?;
                writer.write_all(&size.to_le_bytes())?;
                writer.write_all(&(0u32.to_le_bytes()))?;
                writer.write_all(&(0u32.to_le_bytes()))?;
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
            Command::ReadFlash {
                offset,
                size,
                block_size,
                max_in_flight,
            } => {
                // length
                writer.write_all(&(16u16.to_le_bytes()))?;
                // checksum
                writer.write_all(&(0u32.to_le_bytes()))?;
                // data
                writer.write_all(&offset.to_le_bytes())?;
                writer.write_all(&size.to_le_bytes())?;
                writer.write_all(&block_size.to_le_bytes())?;
                writer.write_all(&(max_in_flight.to_le_bytes()))?;
            }
            Command::RunUserCode => {
                write_basic(writer, &[], 0)?;
            }
            Command::FlashDetect => {
                write_basic(writer, &[], 0)?;
            }
            Command::GetSecurityInfo => {
                write_basic(writer, &[], 0)?;
            }
        };
        Ok(())
    }
}

/// Write a data array and its checksum to a writer
fn write_basic<W: Write>(mut writer: W, data: &[u8], checksum: u32) -> std::io::Result<()> {
    writer.write_all(&((data.len() as u16).to_le_bytes()))?;
    writer.write_all(&(checksum.to_le_bytes()))?;
    writer.write_all(data)?;
    Ok(())
}

/// Write a Begin command to a writer
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
        // The ESP32 does not take the `encrypted` field, so truncate the last
        // 4 bytes of the slice where it resides.
        let end = bytes.len() - 4;
        &bytes[0..end]
    } else {
        bytes
    };
    write_basic(writer, data, 0)
}

/// Write a Data command to a writer
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

const CHECKSUM_INIT: u8 = 0xEF;

fn checksum(data: &[u8], mut checksum: u8) -> u8 {
    for byte in data {
        checksum ^= *byte;
    }

    checksum
}

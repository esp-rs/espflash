use slip_codec::Error as SlipError;
use thiserror::Error;

#[derive(Error, Debug)]
#[non_exhaustive]
pub enum Error {
    #[error("IO error while using serial port: {0}")]
    Serial(#[from] serial::core::Error),
    #[error("Failed to connect to the device")]
    ConnectionFailed,
    #[error("Timeout while running command")]
    Timeout,
    #[error("Invalid SLIP framing")]
    FramingError,
    #[error("Packet to large for buffer")]
    OverSizedPacket,
    #[error("elf image is not valid")]
    InvalidElf,
    #[error("elf image can not be ran from ram")]
    ElfNotRamLoadable,
    #[error("bootloader returned an error: {0:?}")]
    RomError(RomError),
    #[error("chip not recognized")]
    UnrecognizedChip,
    #[error("flash chip not supported")]
    UnsupportedFlash,
}

impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Self {
        Self::Serial(serial::core::Error::from(err))
    }
}

impl From<SlipError> for Error {
    fn from(err: SlipError) -> Self {
        match err {
            SlipError::FramingError => Self::FramingError,
            SlipError::OversizedPacket => Self::OverSizedPacket,
            SlipError::ReadError(io) => Self::from(io),
        }
    }
}

impl From<binread::Error> for Error {
    fn from(err: binread::Error) -> Self {
        match err {
            binread::Error::Io(e) => Error::from(e),
            _ => unreachable!(),
        }
    }
}

#[derive(Copy, Clone, Debug)]
#[allow(dead_code)]
#[repr(u8)]
pub enum RomError {
    InvalidMessage = 0x05,
    FailedToAct = 0x06,
    InvalidCrc = 0x07,
    FlashWriteError = 0x08,
    FlashReadError = 0x09,
    FlashReadLengthError = 0x0a,
    DeflateError = 0x0b,
    Other = 0xff,
}

impl From<u8> for RomError {
    fn from(raw: u8) -> Self {
        match raw {
            0x05 => RomError::InvalidMessage,
            0x06 => RomError::FailedToAct,
            0x07 => RomError::InvalidCrc,
            0x08 => RomError::FlashWriteError,
            0x09 => RomError::FlashReadError,
            0x0a => RomError::FlashReadLengthError,
            0x0b => RomError::DeflateError,
            _ => RomError::Other,
        }
    }
}

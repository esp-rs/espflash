use slip_codec::Error as SlipError;
use thiserror::Error;

#[derive(Error, Debug)]
#[non_exhaustive]
pub enum Error {
    #[error("IO error while using serial port: {0}")]
    Serial(#[from] serial::core::Error),
    #[error("Failed to connect to the device")]
    ConnectionFailed,
    #[error("Invalid SLIP framing")]
    FramingError,
    #[error("Packet to large for buffer")]
    OverSizedPacket,
    #[error("elf image is not valid")]
    InvalidElf,
    #[error("elf image can not be ran from ram")]
    ElfNotRamLoadable,
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

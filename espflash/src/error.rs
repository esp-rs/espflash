//! Library and application errors

#[cfg(feature = "serialport")]
use std::fmt::{Display, Formatter};
use std::{array::TryFromSliceError, io};

use miette::Diagnostic;
#[cfg(feature = "serialport")]
use slip_codec::SlipError;
use strum::VariantNames;
use thiserror::Error;

#[cfg(feature = "cli")]
use crate::cli::monitor::parser::esp_defmt::DefmtError;
#[cfg(feature = "serialport")]
use crate::command::CommandType;
use crate::{
    flasher::{FlashFrequency, FlashSize},
    targets::Chip,
};

/// All possible errors returned by espflash
#[derive(Debug, Diagnostic, Error)]
#[non_exhaustive]
pub enum Error {
    #[error("App partition not found")]
    #[diagnostic(code(espflash::app_partition_not_found))]
    AppPartitionNotFound,

    #[error("Operation was cancelled by the user")]
    #[diagnostic(code(espflash::cancelled))]
    Cancelled,

    #[error("Unrecognized magic value: {0:#x}")]
    #[diagnostic(
        code(espflash::chip_detect_error),
        help("Supported chips are: {}\n\
              If your chip is supported, try hard-resetting the device and try again",
             Chip::VARIANTS.join(", "))
    )]
    ChipDetectError(u32),

    #[error("Chip provided ({0}) with `-c/--chip` does not match the detected chip ({1})")]
    #[diagnostic(
        code(espflash::chip_missmatch),
        help("Ensure that the correct chip is selected, or remove the `-c/--chip` option to autodetect the chip")
    )]
    ChipMismatch(String, String),

    #[error("Chip not argument provided, this is required when using the `--before no-reset-no-sync` option")]
    #[diagnostic(
        code(espflash::chip_not_provided),
        help("Ensure that you provide the `-c/--chip` option with the proper chip")
    )]
    ChipNotProvided,

    #[error("Corrupt data, expected {0:2x?} bytes but receved {1:2x?} bytes")]
    #[diagnostic(code(espflash::read_flash::corrupt_data))]
    CorruptData(usize, usize),

    #[error("MD5 digest missmatch: expected {0:2x?}, received: {1:2x?}")]
    #[diagnostic(code(espflash::read_flash::digest_missmatch))]
    DigestMissmatch(Vec<u8>, Vec<u8>),

    #[error("Supplied ELF image can not be run from RAM, as it includes segments mapped to ROM addresses")]
    #[diagnostic(
        code(espflash::not_ram_loadable),
        help("Either build the binary to be all in RAM, or remove the `--ram` option to load the image to flash")
    )]
    ElfNotRamLoadable,

    #[error(
        "Supplied ELF image of {0}B is too big, and doesn't fit configured app partition of {1}B"
    )]
    #[diagnostic(
        code(espflash::image_too_big),
        help("Reduce the size of the binary or increase the size of the app partition."),
        url("https://docs.espressif.com/projects/esp-idf/en/latest/esp32/api-guides/partition-tables.html#built-in-partition-tables")
    )]
    ElfTooBig(u32, u32),

    #[error("Failed to connect to on-device flash")]
    #[diagnostic(code(espflash::flash_connect))]
    FlashConnect,

    #[error("Expected MD5 digest (16 bytes), received: {0:#x} bytes")]
    #[diagnostic(code(espflash::read_flash::incorrect_digest_length))]
    IncorrectDigestLength(usize),

    #[error("Incorrect response from the sutb/ROM loader")]
    #[diagnostic(code(espflash::read_flash::incorrect_response))]
    IncorrectReposnse,

    #[error("The provided bootloader binary is invalid")]
    InvalidBootloader,

    #[error("Specified bootloader path is not a .bin file")]
    #[diagnostic(code(espflash::invalid_bootloader_path))]
    InvalidBootloaderPath,

    #[error("The flash size '{0}' is invalid")]
    #[diagnostic(
        code(espflash::invalid_flash_size),
        help("The accepted values are: {:?}", FlashSize::VARIANTS)
    )]
    InvalidFlashSize(String),

    #[cfg(not(feature = "serialport"))]
    #[error(transparent)]
    IoError(#[from] io::Error),

    #[error("Specified partition table path is not a .bin or .csv file")]
    #[diagnostic(code(espflash::invalid_partition_table_path))]
    InvalidPartitionTablePath,

    #[error("No serial ports could be detected")]
    #[diagnostic(
        code(espflash::no_serial),
        help("Make sure you have connected a device to the host system. If the device is connected but not listed, try using the `--list-all-ports` flag.")
    )]
    NoSerial,

    #[error("Read more bytes than expected")]
    #[diagnostic(code(espflash::read_flash::read_more_than_expected))]
    ReadMoreThanExpected,

    #[error("This command requires using the RAM stub")]
    #[diagnostic(
        code(espflash::stub_required),
        help("Don't use the `--no-stub` option with the command")
    )]
    StubRequired,

    #[error("The serial port '{0}' could not be found")]
    #[diagnostic(
        code(espflash::serial_not_found),
        help("Make sure the correct device is connected to the host system")
    )]
    SerialNotFound(String),

    #[error("The {chip} does not support {feature}")]
    #[diagnostic(code(espflash::unsupported_feature))]
    UnsupportedFeature { chip: Chip, feature: String },

    #[error("Flash chip not supported, unrecognized flash ID: {0:#x}")]
    #[diagnostic(code(espflash::unrecognized_flash))]
    UnsupportedFlash(u8),

    #[error("The specified flash frequency '{frequency}' is not supported by the {chip}")]
    #[diagnostic(code(espflash::unsupported_flash_frequency))]
    UnsupportedFlashFrequency {
        chip: Chip,
        frequency: FlashFrequency,
    },

    #[error(
        "Minimum supported revision is {major}.{minor}, connected device's revision is {found_major}.{found_minor}"
    )]
    #[diagnostic(code(espflash::unsupported_chip_revision))]
    UnsupportedChipRevision {
        major: u16,
        minor: u16,
        found_major: u16,
        found_minor: u16,
    },

    #[error("Failed to parse chip revision: {chip_rev}. Chip revision must be in the format `major.minor`")]
    #[diagnostic(code(espflash::cli::parse_chip_rev_error))]
    ParseChipRevError { chip_rev: String },

    #[error("Error while connecting to device")]
    #[diagnostic(transparent)]
    Connection(#[source] ConnectionError),

    #[error("Communication error while flashing device")]
    #[diagnostic(transparent)]
    Flashing(#[source] ConnectionError),

    #[error("Supplied ELF image is not valid")]
    #[diagnostic(
        code(espflash::invalid_elf),
        help("Try running `cargo clean` and rebuilding the image")
    )]
    InvalidElf(#[from] ElfError),

    #[error("The bootloader returned an error")]
    #[cfg(feature = "serialport")]
    #[diagnostic(transparent)]
    RomError(#[from] RomError),

    #[cfg(feature = "cli")]
    #[error(transparent)]
    #[diagnostic(transparent)]
    Defmt(#[from] DefmtError),

    #[error("Verification of flash content failed")]
    #[diagnostic(code(espflash::verify_failed))]
    VerifyFailed,

    #[cfg(feature = "cli")]
    #[error(transparent)]
    #[diagnostic(code(espflash::dialoguer_error))]
    DialoguerError(#[from] dialoguer::Error),

    #[error("Internal Error")]
    InternalError,

    #[error("Failed to open file: {0}")]
    FileOpenError(String, #[source] io::Error),

    #[error("Failed to parse partition table")]
    Partition(#[from] esp_idf_part::Error),

    #[error("Invalid response length, expected >= {expected}, got {got}")]
    InvalidResponse {
        expected: u32,
        got: usize,
    },
}

#[cfg(feature = "serialport")]
impl From<io::Error> for Error {
    fn from(err: io::Error) -> Self {
        Self::Connection(err.into())
    }
}

#[cfg(feature = "serialport")]
#[cfg_attr(docsrs, doc(cfg(feature = "serialport")))]
impl From<serialport::Error> for Error {
    fn from(err: serialport::Error) -> Self {
        Self::Connection(err.into())
    }
}

#[cfg(feature = "serialport")]
impl From<SlipError> for Error {
    fn from(err: SlipError) -> Self {
        Self::Connection(err.into())
    }
}

impl From<TryFromSliceError> for Error {
    fn from(_: TryFromSliceError) -> Self {
            Error::InvalidResponse {
                expected: 0,
                got: 0,
            }
        }
}

/// Connection-related errors
#[derive(Debug, Diagnostic, Error)]
#[non_exhaustive]
pub enum ConnectionError {
    #[error("Failed to connect to the device")]
    #[diagnostic(
        code(espflash::connection_failed),
        help("Ensure that the device is connected and the reset and boot pins are not being held down")
    )]
    ConnectionFailed,

    #[error("Serial port not found")]
    #[diagnostic(
        code(espflash::connection_failed),
        help("Ensure that the device is connected and your host recognizes the serial adapter")
    )]
    DeviceNotFound,

    #[error("Received packet has invalid SLIP framing")]
    #[diagnostic(
        code(espflash::slip_framing),
        help("Try hard-resetting the device and try again, if the error persists your ROM may be corrupted")
    )]
    FramingError,

    #[error("Invalid stub handshake response received")]
    InvalidStubHandshake,

    #[error("Download mode successfully detected, but getting no sync reply")]
    #[diagnostic(
        code(espflash::no_sync_reply),
        help("The serial TX path seems to be down")
    )]
    NoSyncReply,

    #[error("Received packet to large for buffer")]
    #[diagnostic(
        code(espflash::oversized_packet),
        help("Try hard-resetting the device and try again, if the error persists your ROM may be corrupted")
    )]
    OverSizedPacket,

    #[error("Failed to read the available bytes on the serial port. Available bytes: {0}, Read bytes: {1}")]
    #[diagnostic(code(espflash::read_missmatch))]
    ReadMissmatch(u32, u32),

    #[cfg(feature = "serialport")]
    #[error("Timeout while running {0}command")]
    #[diagnostic(code(espflash::timeout))]
    Timeout(TimedOutCommand),

    #[cfg(feature = "serialport")]
    #[error("IO error while using serial port: {0}")]
    #[diagnostic(code(espflash::serial_error))]
    Serial(#[source] serialport::Error),

    #[error("Wrong boot mode detected ({0})! The chip needs to be in download mode.")]
    #[diagnostic(code(espflash::wrong_boot_mode))]
    WrongBootMode(String),
}

#[cfg(feature = "serialport")]
impl From<io::Error> for ConnectionError {
    fn from(err: io::Error) -> Self {
        from_error_kind(err.kind(), err)
    }
}

#[cfg(feature = "serialport")]
#[cfg_attr(docsrs, doc(cfg(feature = "serialport")))]
impl From<serialport::Error> for ConnectionError {
    fn from(err: serialport::Error) -> Self {
        use serialport::ErrorKind;

        match err.kind() {
            ErrorKind::Io(kind) => from_error_kind(kind, err),
            ErrorKind::NoDevice => ConnectionError::DeviceNotFound,
            _ => ConnectionError::Serial(err),
        }
    }
}

#[cfg(feature = "serialport")]
impl From<SlipError> for ConnectionError {
    fn from(err: SlipError) -> Self {
        match err {
            SlipError::FramingError => Self::FramingError,
            SlipError::OversizedPacket => Self::OverSizedPacket,
            SlipError::ReadError(io) => Self::from(io),
            SlipError::EndOfStream => Self::FramingError,
        }
    }
}

/// An executed command which has timed out
#[derive(Clone, Debug, Default)]
#[cfg(feature = "serialport")]
pub struct TimedOutCommand {
    command: Option<CommandType>,
}

#[cfg(feature = "serialport")]
impl Display for TimedOutCommand {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match &self.command {
            Some(command) => write!(f, "{} ", command),
            None => Ok(()),
        }
    }
}

#[cfg(feature = "serialport")]
impl From<CommandType> for TimedOutCommand {
    fn from(ct: CommandType) -> Self {
        TimedOutCommand { command: Some(ct) }
    }
}

/// Errors originating from a device's ROM functionality
#[derive(Clone, Copy, Debug, Default, Diagnostic, Error, strum::FromRepr)]
#[non_exhaustive]
#[repr(u8)]
#[cfg(feature = "serialport")]
pub enum RomErrorKind {
    #[error("Invalid message received")]
    #[diagnostic(code(espflash::rom::invalid_message))]
    InvalidMessage = 0x05,

    #[error("Bootloader failed to execute command")]
    #[diagnostic(code(espflash::rom::failed))]
    FailedToAct = 0x06,

    #[error("Received message has invalid CRC")]
    #[diagnostic(code(espflash::rom::crc))]
    InvalidCrc = 0x07,

    #[error("Bootloader failed to write to flash")]
    #[diagnostic(code(espflash::rom::flash_write))]
    FlashWriteError = 0x08,

    #[error("Bootloader failed to read from flash")]
    #[diagnostic(code(espflash::rom::flash_read))]
    FlashReadError = 0x09,

    #[error("Invalid length for flash read")]
    #[diagnostic(code(espflash::rom::flash_read_length))]
    FlashReadLengthError = 0x0a,

    #[error("Malformed compressed data received")]
    #[diagnostic(code(espflash::rom::deflate))]
    DeflateError = 0x0b,

    #[error("Bad data length")]
    #[diagnostic(code(espflash::rom::data_len))]
    BadDataLen = 0xc0,

    #[error("Bad data checksum")]
    #[diagnostic(code(espflash::rom::data_crc))]
    BadDataChecksum = 0xc1,

    #[error("Bad block size")]
    #[diagnostic(code(espflash::rom::block_size))]
    BadBlocksize = 0xc2,

    #[error("Invalid command")]
    #[diagnostic(code(espflash::rom::cmd))]
    InvalidCommand = 0xc3,

    #[error("SPI operation failed")]
    #[diagnostic(code(espflash::rom::spi))]
    FailedSpiOp = 0xc4,

    #[error("SPI unlock failed")]
    #[diagnostic(code(espflash::rom::spi_unlock))]
    FailedSpiUnlock = 0xc5,

    #[error("Not in flash mode")]
    #[diagnostic(code(espflash::rom::flash_mode))]
    NotInFlashMode = 0xc6,

    #[error("Error when uncompressing the data")]
    #[diagnostic(code(espflash::rom::inflate))]
    InflateError = 0xc7,

    #[error("Didn't receive enough data")]
    #[diagnostic(code(espflash::rom::not_enough))]
    NotEnoughData = 0xc8,

    #[error("Received too much data")]
    #[diagnostic(code(espflash::rom::too_much_data))]
    TooMuchData = 0xc9,

    #[default]
    #[error("Other")]
    #[diagnostic(code(espflash::rom::other))]
    Other = 0xff,
}

#[cfg(feature = "serialport")]
impl From<u8> for RomErrorKind {
    fn from(raw: u8) -> Self {
        Self::from_repr(raw).unwrap_or_default()
    }
}

/// An error originating from a device's ROM functionality
#[derive(Clone, Copy, Debug, Diagnostic, Error)]
#[error("Error while running {command} command")]
#[cfg(feature = "serialport")]
#[non_exhaustive]
pub struct RomError {
    command: CommandType,
    #[source]
    kind: RomErrorKind,
}

#[cfg(feature = "serialport")]
impl RomError {
    pub fn new(command: CommandType, kind: RomErrorKind) -> RomError {
        RomError { command, kind }
    }
}

/// Missing partition error
#[derive(Debug, Diagnostic, Error)]
#[error("Missing partition")]
#[diagnostic(
    code(espflash::partition_table::missing_partition),
    help("Partition table must contain the partition of type `{0}` to be erased")
)]
pub struct MissingPartition(String);

impl From<String> for MissingPartition {
    fn from(part: String) -> Self {
        MissingPartition(part)
    }
}

/// Missing partition table error
#[derive(Debug, Error, Diagnostic)]
#[error("No partition table could be found")]
#[diagnostic(
    code(espflash::partition_table::missing_partition_table),
    help("Try providing a CSV or binary paritition table with the `--partition-table` argument.")
)]
pub struct MissingPartitionTable;

/// Invalid ELF file error
#[derive(Debug, Error)]
#[error("{0}")]
pub struct ElfError(&'static str);

impl From<&'static str> for ElfError {
    fn from(err: &'static str) -> Self {
        ElfError(err)
    }
}

#[cfg(feature = "serialport")]
pub(crate) trait ResultExt {
    /// Mark an error as having occurred during the flashing stage
    fn flashing(self) -> Self;
    /// Mark the command from which this error originates
    fn for_command(self, command: CommandType) -> Self;
}

#[cfg(feature = "serialport")]
impl<T> ResultExt for Result<T, Error> {
    fn flashing(self) -> Self {
        match self {
            Err(Error::Connection(err)) => Err(Error::Flashing(err)),
            res => res,
        }
    }

    fn for_command(self, command: CommandType) -> Self {
        match self {
            Err(Error::Connection(ConnectionError::Timeout(_))) => {
                Err(Error::Connection(ConnectionError::Timeout(command.into())))
            }
            Err(Error::Flashing(ConnectionError::Timeout(_))) => {
                Err(Error::Flashing(ConnectionError::Timeout(command.into())))
            }
            res => res,
        }
    }
}

#[cfg(feature = "serialport")]
#[cfg_attr(docsrs, doc(cfg(feature = "serialport")))]
fn from_error_kind<E>(kind: io::ErrorKind, err: E) -> ConnectionError
where
    E: Into<serialport::Error>,
{
    use io::ErrorKind;

    match kind {
        ErrorKind::TimedOut => ConnectionError::Timeout(TimedOutCommand::default()),
        ErrorKind::NotFound => ConnectionError::DeviceNotFound,
        _ => ConnectionError::Serial(err.into()),
    }
}

use std::{
    fmt::{Display, Formatter},
    io,
};

use miette::{Diagnostic, SourceOffset, SourceSpan};
use slip_codec::SlipError;
use strum::VariantNames;
use thiserror::Error;

use crate::{
    command::CommandType,
    elf::{FlashFrequency, FlashMode},
    flasher::FlashSize,
    image_format::ImageFormatId,
    partition_table::{CoreType, SubType, Type},
    Chip,
};

#[derive(Error, Debug, Diagnostic)]
#[non_exhaustive]
pub enum Error {
    #[error("Error while connecting to device")]
    #[diagnostic(transparent)]
    Connection(#[source] ConnectionError),
    #[error("Communication error while flashing device")]
    #[diagnostic(transparent)]
    Flashing(#[source] ConnectionError),
    #[error("Supplied elf image is not valid")]
    #[diagnostic(
        code(espflash::invalid_elf),
        help("Try running `cargo clean` and rebuilding the image")
    )]
    InvalidElf(#[from] ElfError),
    #[error("Supplied elf image can not be ran from ram as it includes segments mapped to rom addresses")]
    #[diagnostic(
        code(espflash::not_ram_loadable),
        help("Either build the binary to be all in ram or remove the `--ram` option to load the image to flash")
    )]
    ElfNotRamLoadable,
    #[error("Supplied elf image is too big and doesn't fit configured app partition")]
    ElfTooBig,
    #[error("The bootloader returned an error")]
    #[diagnostic(transparent)]
    RomError(#[from] RomError),
    #[error("Chip not recognized, supported chip types are esp32, esp32-c3, esp32-s2, esp32-s3, and esp8266")]
    #[diagnostic(code(espflash::unrecognized_chip))]
    UnrecognizedChipName,
    #[error("Chip not recognized, supported chip types are esp32, esp32-c3, esp32-s2, esp32-s3, and esp8266")]
    #[diagnostic(
        code(espflash::unrecognized_chip),
        help("If your chip is supported, try hard-resetting the device and try again")
    )]
    UnrecognizedChip(#[from] ChipDetectError),
    #[error("Flash chip not supported, flash sizes from 1 to 16MB are supported")]
    #[diagnostic(code(espflash::unrecognized_flash))]
    UnsupportedFlash(#[from] FlashDetectError),
    #[error("Failed to connect to on-device flash")]
    #[diagnostic(code(espflash::flash_connect))]
    FlashConnect,
    #[error(transparent)]
    #[diagnostic(transparent)]
    MalformedPartitionTable(#[from] PartitionTableError),
    #[error(transparent)]
    #[diagnostic(transparent)]
    UnsupportedImageFormat(#[from] UnsupportedImageFormatError),
    #[error("Unrecognized image format {0}")]
    #[diagnostic(
        code(espflash::unknown_format),
        help("The following image formats are {}", ImageFormatId::VARIANTS.join(", "))
    )]
    UnknownImageFormat(String),
    #[error("binary is not setup correct to support direct boot")]
    #[diagnostic(
        code(espflash::invalid_direct_boot),
        help(
            "See the following page for documentation on how to setup your binary for direct boot:
https://github.com/espressif/esp32c3-direct-boot-example"
        )
    )]
    InvalidDirectBootBinary,
    #[error("No serial ports could be detected")]
    #[diagnostic(
        code(espflash::no_serial),
        help("Make sure you have connected a device to the host system")
    )]
    NoSerial,
    #[error("The serial port '{0}' could not be found")]
    #[diagnostic(
        code(espflash::serial_not_found),
        help("Make sure the correct device is connected to the host system")
    )]
    SerialNotFound(String),
    #[error("Canceled by user")]
    Canceled,
    #[error("The flash mode '{0}' is not valid")]
    #[diagnostic(
        code(espflash::invalid_flash_mode),
        help("The accepted values are: {:?}", FlashMode::VARIANTS)
    )]
    InvalidFlashMode(String),
    #[error("The flash frequency '{0}' is not valid")]
    #[diagnostic(
        code(espflash::invalid_flash_frequency),
        help("The accepted values are: {:?}", FlashFrequency::VARIANTS)
    )]
    InvalidFlashFrequency(String),
    #[error("The flash size '{0}' is not valid")]
    #[diagnostic(
        code(espflash::invalid_flash_size),
        help("The accepted values are: {:?}", FlashSize::VARIANTS)
    )]
    InvalidFlashSize(String),
    #[error("The provided bootloader binary is not valid")]
    InvalidBootloader,
    #[error("The specified flash frequency ({frequency}) is not supported by the {chip}")]
    #[diagnostic(code(espflash::unsupported_flash_frequency))]
    UnsupportedFlashFrequency {
        chip: Chip,
        frequency: FlashFrequency,
    },
}

#[derive(Error, Debug, Diagnostic)]
#[non_exhaustive]
pub enum ConnectionError {
    #[error("IO error while using serial port: {0}")]
    #[diagnostic(code(espflash::serial_error))]
    Serial(#[source] serialport::Error),
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
    #[error("Timeout while running {0}command")]
    #[diagnostic(code(espflash::timeout))]
    Timeout(TimedOutCommand),
    #[error("Received packet has invalid SLIP framing")]
    #[diagnostic(
        code(espflash::slip_framing),
        help("Try hard-resetting the device and try again, if the error persists your rom might be corrupted")
    )]
    FramingError,
    #[error("Received packet to large for buffer")]
    #[diagnostic(
        code(espflash::oversized_packet),
        help("Try hard-resetting the device and try again, if the error persists your rom might be corrupted")
    )]
    OverSizedPacket,
    #[error("Invalid stub handshake response received")]
    InvalidStubHandshake,
}

#[derive(Debug, Default, Clone)]
pub struct TimedOutCommand {
    command: Option<CommandType>,
}

impl From<CommandType> for TimedOutCommand {
    fn from(c: CommandType) -> Self {
        TimedOutCommand { command: Some(c) }
    }
}

impl Display for TimedOutCommand {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match &self.command {
            Some(command) => write!(f, "{} ", command),
            None => Ok(()),
        }
    }
}

impl From<serialport::Error> for ConnectionError {
    fn from(err: serialport::Error) -> Self {
        match err.kind() {
            serialport::ErrorKind::Io(kind) => from_error_kind(kind, err),
            serialport::ErrorKind::NoDevice => ConnectionError::DeviceNotFound,
            _ => ConnectionError::Serial(err),
        }
    }
}

impl From<serialport::Error> for Error {
    fn from(err: serialport::Error) -> Self {
        Self::Connection(err.into())
    }
}

impl From<io::Error> for ConnectionError {
    fn from(err: io::Error) -> Self {
        from_error_kind(err.kind(), err)
    }
}

impl From<io::Error> for Error {
    fn from(err: io::Error) -> Self {
        Self::Connection(err.into())
    }
}

fn from_error_kind<E: Into<serialport::Error>>(kind: io::ErrorKind, err: E) -> ConnectionError {
    match kind {
        io::ErrorKind::TimedOut => ConnectionError::Timeout(TimedOutCommand::default()),
        io::ErrorKind::NotFound => ConnectionError::DeviceNotFound,
        _ => ConnectionError::Serial(err.into()),
    }
}

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

impl From<SlipError> for Error {
    fn from(err: SlipError) -> Self {
        Self::Connection(err.into())
    }
}

impl From<binread::Error> for ConnectionError {
    fn from(err: binread::Error) -> Self {
        match err {
            binread::Error::Io(e) => ConnectionError::from(e),
            _ => unreachable!(),
        }
    }
}

impl From<binread::Error> for Error {
    fn from(err: binread::Error) -> Self {
        Self::Connection(err.into())
    }
}

#[derive(Copy, Clone, Debug, Error, Diagnostic)]
#[allow(dead_code)]
#[repr(u8)]
#[non_exhaustive]
pub enum RomErrorKind {
    #[error("Invalid message received")]
    #[diagnostic(code(espflash::rom::invalid_message))]
    InvalidMessage = 0x05,
    #[error("Bootloader failed to execute command")]
    #[diagnostic(code(espflash::rom::failed))]
    FailedToAct = 0x06,
    #[error("Received message has invalid crc")]
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
    #[error("Other")]
    #[diagnostic(code(espflash::rom::other))]
    Other = 0xff,
}

impl From<u8> for RomErrorKind {
    fn from(raw: u8) -> Self {
        match raw {
            0x05 => RomErrorKind::InvalidMessage,
            0x06 => RomErrorKind::FailedToAct,
            0x07 => RomErrorKind::InvalidCrc,
            0x08 => RomErrorKind::FlashWriteError,
            0x09 => RomErrorKind::FlashReadError,
            0x0a => RomErrorKind::FlashReadLengthError,
            0x0b => RomErrorKind::DeflateError,
            _ => RomErrorKind::Other,
        }
    }
}

#[derive(Copy, Clone, Debug, Error, Diagnostic)]
#[allow(dead_code)]
#[non_exhaustive]
#[error("Error while running {command} command")]
pub struct RomError {
    command: CommandType,
    #[source]
    kind: RomErrorKind,
}

impl RomError {
    pub fn new(command: CommandType, kind: RomErrorKind) -> RomError {
        RomError { command, kind }
    }
}

pub(crate) trait ResultExt {
    /// mark an error as having occurred during the flashing stage
    fn flashing(self) -> Self;
    /// mark the command from which this error originates
    fn for_command(self, command: CommandType) -> Self;
}

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

#[derive(Debug, Error, Diagnostic)]
pub enum PartitionTableError {
    #[error(transparent)]
    #[diagnostic(transparent)]
    Csv(#[from] CSVError),
    #[error(transparent)]
    #[diagnostic(transparent)]
    Overlapping(#[from] OverlappingPartitionsError),
    #[error(transparent)]
    #[diagnostic(transparent)]
    Duplicate(#[from] DuplicatePartitionsError),
    #[error(transparent)]
    #[diagnostic(transparent)]
    InvalidSubType(#[from] InvalidSubTypeError),
    #[error(transparent)]
    #[diagnostic(transparent)]
    NoApp(#[from] NoAppError),
    #[error(transparent)]
    #[diagnostic(transparent)]
    UnalignedPartitionError(#[from] UnalignedPartitionError),
    #[error(transparent)]
    #[diagnostic(transparent)]
    LengthNotMultipleOf32(#[from] LengthNotMultipleOf32),
    #[error(transparent)]
    #[diagnostic(transparent)]
    InvalidChecksum(#[from] InvalidChecksum),
    #[error(transparent)]
    #[diagnostic(transparent)]
    NoEndMarker(#[from] NoEndMarker),
    #[error(transparent)]
    #[diagnostic(transparent)]
    InvalidPartitionTable(#[from] InvalidPartitionTable),
    #[error(transparent)]
    #[diagnostic(transparent)]
    MissingPartitionTable(#[from] MissingPartitionTable),
}

#[derive(Debug, Error, Diagnostic)]
#[error("Malformed partition table")]
#[diagnostic(
    code(espflash::partition_table::mallformed),
    help("{}See the espressif documentation for information on the partition table format:

https://docs.espressif.com/projects/esp-idf/en/latest/esp32/api-guides/partition-tables.html#creating-custom-tables", self.help)
)]
pub struct CSVError {
    #[source_code]
    source: String,
    #[label("{}", self.hint)]
    err_span: SourceSpan,
    hint: String,
    #[source]
    error: csv::Error,
    help: String,
}

impl CSVError {
    pub fn new(error: csv::Error, source: String) -> Self {
        let err_line = match error.kind() {
            csv::ErrorKind::Deserialize { pos: Some(pos), .. } => pos.line(),
            csv::ErrorKind::UnequalLengths { pos: Some(pos), .. } => pos.line(),
            _ => 0,
        };
        let mut hint = match error.kind() {
            csv::ErrorKind::Deserialize { err, .. } => err.to_string(),
            csv::ErrorKind::UnequalLengths {
                expected_len, len, ..
            } => format!(
                "record has {} fields, but the previous record has {} fields",
                len, expected_len
            ),
            _ => String::new(),
        };
        let mut help = String::new();

        // string matching is fragile but afaik there is no better way in this case
        // and if it does break the error is still not bad
        if hint == "data did not match any variant of untagged enum SubType" {
            hint = "Unknown sub-type".into();
            help = format!(
                "the following sub-types are supported:
    {} for data partitions
    {} for app partitions
    0x00-0xFE for custom partitions\n\n",
                CoreType::Data.subtype_hint(),
                CoreType::App.subtype_hint()
            )
        }

        let err_span = line_to_span(&source, err_line as usize);

        CSVError {
            source,
            err_span,
            hint,
            error,
            help,
        }
    }
}

/// since csv doesn't give us the position in the line the error occurs, we
/// highlight the entire line
///
/// line starts at 1
fn line_to_span(source: &str, line: usize) -> SourceSpan {
    let line_length = source.lines().nth(line - 1).unwrap().len().into();
    SourceSpan::new(SourceOffset::from_location(source, line, 2), line_length)
}

#[derive(Debug, Error, Diagnostic)]
#[error("Overlapping partitions")]
#[diagnostic(code(espflash::partition_table::overlapping))]
pub struct OverlappingPartitionsError {
    #[source_code]
    source_code: String,
    #[label("This partition")]
    partition1_span: SourceSpan,
    #[label("overlaps with this partition")]
    partition2_span: SourceSpan,
}

impl OverlappingPartitionsError {
    pub fn new(source: &str, partition1_line: usize, partition2_line: usize) -> Self {
        OverlappingPartitionsError {
            source_code: source.into(),
            partition1_span: line_to_span(source, partition1_line),
            partition2_span: line_to_span(source, partition2_line),
        }
    }
}

#[derive(Debug, Error, Diagnostic)]
#[error("Duplicate partitions")]
#[diagnostic(code(espflash::partition_table::duplicate))]
pub struct DuplicatePartitionsError {
    #[source_code]
    source_code: String,
    #[label("This partition")]
    partition1_span: SourceSpan,
    #[label("has the same {} as this partition", self.ty)]
    partition2_span: SourceSpan,
    ty: &'static str,
}

impl DuplicatePartitionsError {
    pub fn new(
        source: &str,
        partition1_line: usize,
        partition2_line: usize,
        ty: &'static str,
    ) -> Self {
        DuplicatePartitionsError {
            source_code: source.into(),
            partition1_span: line_to_span(source, partition1_line),
            partition2_span: line_to_span(source, partition2_line),
            ty,
        }
    }
}

#[derive(Debug, Error, Diagnostic)]
#[error("Invalid subtype for type")]
#[diagnostic(
    code(espflash::partition_table::invalid_type),
    help("'{}' supports the following subtypes: {}", self.ty, self.ty.subtype_hint())
)]
pub struct InvalidSubTypeError {
    #[source_code]
    source_code: String,
    #[label("'{}' is not a valid subtype for '{}'", self.sub_type, self.ty)]
    span: SourceSpan,
    ty: Type,
    sub_type: SubType,
}

impl InvalidSubTypeError {
    pub fn new(source: &str, line: usize, ty: Type, sub_type: SubType) -> Self {
        InvalidSubTypeError {
            source_code: source.into(),
            span: line_to_span(source, line),
            ty,
            sub_type,
        }
    }
}

#[derive(Debug, Error, Diagnostic)]
#[error("No app partition was found")]
#[diagnostic(
    code(espflash::partition_table::no_app),
    help("Partition table must contain a factory or ota app partition")
)]
pub struct NoAppError {
    #[source_code]
    source_code: String,
}

impl NoAppError {
    pub fn new(source: &str) -> Self {
        NoAppError {
            source_code: source.into(),
        }
    }
}
#[derive(Debug, Error, Diagnostic)]
#[error("No otadata partition was found")]
#[diagnostic(
    code(espflash::partition_table::no_otadata),
    help("Partition table must contain an otadata partition when trying to erase it")
)]

pub struct NoOtadataError;
#[derive(Debug, Error, Diagnostic)]
#[error("Unaligned partition")]
#[diagnostic(code(espflash::partition_table::unaligned))]
pub struct UnalignedPartitionError {
    #[source_code]
    source_code: String,
    #[label("App partition is not aligned to 64k (0x10000)")]
    span: SourceSpan,
}

impl UnalignedPartitionError {
    pub fn new(source: &str, line: usize) -> Self {
        UnalignedPartitionError {
            source_code: source.into(),
            span: line_to_span(source, line),
        }
    }
}

#[derive(Debug, Error, Diagnostic)]
#[error("Partition table length not a multiple of 32")]
#[diagnostic(code(espflash::partition_table::invalid_length))]
pub struct LengthNotMultipleOf32;

#[derive(Debug, Error, Diagnostic)]
#[error("Checksum invalid")]
#[diagnostic(code(espflash::partition_table::invalid_checksum))]
pub struct InvalidChecksum;

#[derive(Debug, Error, Diagnostic)]
#[error("No end marker found")]
#[diagnostic(code(espflash::partition_table::no_end_marker))]
pub struct NoEndMarker;

#[derive(Debug, Error, Diagnostic)]
#[error("Invalid partition table")]
#[diagnostic(code(espflash::partition_table::invalid_partition_table))]
pub struct InvalidPartitionTable;

#[derive(Debug, Error, Diagnostic)]
#[error("Missing partition table")]
#[diagnostic(code(espflash::partition_table::missing_partition_table))]
pub struct MissingPartitionTable;

#[derive(Debug, Error)]
#[error("{0}")]
pub struct ElfError(&'static str);

impl From<&'static str> for ElfError {
    fn from(err: &'static str) -> Self {
        ElfError(err)
    }
}

#[derive(Debug, Error)]
#[error("Unrecognized magic value {0:#x}")]
pub struct ChipDetectError(u32);

impl From<u32> for ChipDetectError {
    fn from(err: u32) -> Self {
        ChipDetectError(err)
    }
}

#[derive(Debug, Error)]
#[error("Unrecognized flash id {0:#x}")]
pub struct FlashDetectError(u8);

impl From<u8> for FlashDetectError {
    fn from(err: u8) -> Self {
        FlashDetectError(err)
    }
}

#[derive(Debug)]
pub struct UnsupportedImageFormatError {
    format: ImageFormatId,
    chip: Chip,
    revision: Option<u32>,
}

impl Display for UnsupportedImageFormatError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Image format {} is not supported by the {}",
            self.format, self.chip
        )?;
        if let Some(revision) = self.revision {
            write!(f, " revision {}", revision)?;
        }
        Ok(())
    }
}

impl std::error::Error for UnsupportedImageFormatError {}

impl Diagnostic for UnsupportedImageFormatError {
    fn code<'a>(&'a self) -> Option<Box<dyn Display + 'a>> {
        Some(Box::new("espflash::unsupported_image_format"))
    }

    fn help<'a>(&'a self) -> Option<Box<dyn Display + 'a>> {
        let str = if self.chip == Chip::Esp32c3 && self.format == ImageFormatId::DirectBoot {
            format!(
                "The {}: only supports direct-boot starting with revision 3",
                self.chip,
            )
        } else {
            format!(
                "The following image formats are supported by the {}: {}",
                self.chip,
                self.supported_formats()
            )
        };
        Some(Box::new(str))
    }
}

impl UnsupportedImageFormatError {
    pub fn new(format: ImageFormatId, chip: Chip, revision: Option<u32>) -> Self {
        UnsupportedImageFormatError {
            format,
            chip,
            revision,
        }
    }

    fn supported_formats(&self) -> String {
        self.chip
            .supported_image_formats()
            .iter()
            .map(|format| format.into())
            .collect::<Vec<&'static str>>()
            .join(", ")
    }
}

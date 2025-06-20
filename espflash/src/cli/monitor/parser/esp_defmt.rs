use std::io::Write;

use crossterm::{QueueableCommand, style::Print};
use defmt_decoder::{
    Frame,
    Table,
    log::format::{Formatter, FormatterConfig, FormatterFormat},
};
use log::warn;
use miette::{Context, Diagnostic, Result, bail, ensure};
use thiserror::Error;

use crate::cli::monitor::parser::InputParser;

/// Errors that can occur when setting up the defmt logger.
#[derive(Clone, Copy, Debug, Diagnostic, Error)]
#[error("Could not set up defmt logger")]
pub enum DefmtError {
    #[error("No elf data available")]
    #[diagnostic(
        code(espflash::monitor::defmt::no_elf),
        help("Please provide an ELF file with the `--elf` argument")
    )]
    NoElf,

    #[error("No defmt data was found in the elf file")]
    #[diagnostic(code(espflash::monitor::defmt::no_defmt))]
    NoDefmtData,

    #[error("Failed to parse defmt data")]
    #[diagnostic(code(espflash::monitor::defmt::table_parse_failed))]
    TableParseFailed,

    #[error("Failed to parse defmt location data")]
    #[diagnostic(code(espflash::monitor::defmt::location_parse_failed))]
    LocationDataParseFailed,

    #[error("Unsupported defmt encoding: {0:?}. Only rzcobs is supported.")]
    #[diagnostic(code(espflash::monitor::defmt::unsupported_encoding))]
    UnsupportedEncoding(defmt_decoder::Encoding),
}

#[derive(Debug, PartialEq)]
enum FrameKind<'a> {
    Defmt(&'a [u8]),
    Raw(&'a [u8]),
}

#[derive(Debug)]
struct FrameDelimiter {
    buffer: Vec<u8>,
    in_frame: bool,
}

// Framing info added by esp-println
const FRAME_START: &[u8] = &[0xFF, 0x00];
const FRAME_END: &[u8] = &[0x00];

impl FrameDelimiter {
    fn new() -> Self {
        Self {
            buffer: Vec::new(),
            in_frame: false,
        }
    }

    fn search(haystack: &[u8], look_for_end: bool) -> Option<(&[u8], usize)> {
        let needle = if look_for_end { FRAME_END } else { FRAME_START };
        let start = if look_for_end {
            // skip leading zeros
            haystack.iter().position(|&b| b != 0)?
        } else {
            0
        };

        let end = haystack[start..]
            .windows(needle.len())
            .position(|window| window == needle)?;

        Some((&haystack[start..][..end], start + end + needle.len()))
    }

    /// Feeds data into the parser, extracting and processing framed or raw
    /// data.
    pub fn feed(&mut self, buffer: &[u8], mut process: impl FnMut(FrameKind<'_>)) {
        self.buffer.extend_from_slice(buffer);

        while let Some((frame, consumed)) = Self::search(&self.buffer, self.in_frame) {
            if self.in_frame {
                process(FrameKind::Defmt(frame));
            } else if !frame.is_empty() {
                process(FrameKind::Raw(frame));
            }
            self.in_frame = !self.in_frame;

            self.buffer.drain(..consumed);
        }

        if !self.in_frame {
            // If we have a 0xFF byte at the end, we should assume it's the start of a new
            // frame.
            let consume = if self.buffer.ends_with(&[0xFF]) {
                &self.buffer[..self.buffer.len() - 1]
            } else {
                self.buffer.as_slice()
            };

            if !consume.is_empty() {
                process(FrameKind::Raw(consume));
                self.buffer.drain(..consume.len());
            }
        }
    }
}

struct DefmtData {
    table: Table,
    locs: Option<defmt_decoder::Locations>,
    formatter: Formatter,
}

impl DefmtData {
    /// Loads symbols from the ELF file (if provided) and initializes the
    /// context.
    fn load(elf: Option<&[u8]>, output_format: Option<String>) -> Result<Self> {
        let Some(elf) = elf else {
            bail!(DefmtError::NoElf);
        };

        let table = match Table::parse(elf) {
            Ok(Some(table)) => table,
            Ok(None) => bail!(DefmtError::NoDefmtData),
            Err(e) => return Err(DefmtError::TableParseFailed).with_context(|| e),
        };

        let encoding = table.encoding();

        // We only support rzcobs encoding because it is the only way to multiplex
        // a defmt stream and an ASCII log stream over the same serial port.
        ensure!(
            encoding == defmt_decoder::Encoding::Rzcobs,
            DefmtError::UnsupportedEncoding(encoding)
        );

        let locs = table
            .get_locations(elf)
            .map_err(|_e| DefmtError::LocationDataParseFailed)?;

        let locs = if !table.is_empty() && locs.is_empty() {
            warn!(
                "Insufficient DWARF info; compile your program with `debug = 2` to enable location info."
            );
            None
        } else if table.indices().all(|idx| locs.contains_key(&(idx as u64))) {
            Some(locs)
        } else {
            warn!("Location info is incomplete; it will be omitted from the output.");
            None
        };

        let show_location = locs.is_some();
        let has_timestamp = table.has_timestamp();

        let format = match output_format.as_deref() {
            None | Some("oneline") => FormatterFormat::OneLine {
                with_location: show_location,
            },
            Some("full") => FormatterFormat::Default {
                with_location: show_location,
            },
            Some(format) => FormatterFormat::Custom(format),
        };

        Ok(Self {
            table,
            locs,
            formatter: Formatter::new(FormatterConfig {
                format,
                is_timestamp_available: has_timestamp,
            }),
        })
    }

    fn print(&self, frame: Frame<'_>, out: &mut dyn Write) {
        let loc = self.locs.as_ref().and_then(|locs| locs.get(&frame.index()));
        let (file, line, module) = if let Some(loc) = loc {
            (
                Some(loc.file.display().to_string()),
                Some(loc.line.try_into().unwrap()),
                Some(loc.module.as_str()),
            )
        } else {
            (None, None, None)
        };
        let s = self
            .formatter
            .format_frame(frame, file.as_deref(), line, module);

        out.queue(Print(s)).unwrap();
        out.queue(Print("\r\n")).unwrap();

        out.flush().unwrap();
    }
}

/// A parser for defmt-encoded data.
pub struct EspDefmt {
    delimiter: FrameDelimiter,
    defmt_data: DefmtData,
}

impl std::fmt::Debug for EspDefmt {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EspDefmt").finish()
    }
}

impl EspDefmt {
    /// Creates a new `EspDefmt` parser.
    pub fn new(elf: Option<&[u8]>, output_format: Option<String>) -> Result<Self> {
        DefmtData::load(elf, output_format).map(|defmt_data| Self {
            delimiter: FrameDelimiter::new(),
            defmt_data,
        })
    }
}

impl InputParser for EspDefmt {
    fn feed(&mut self, bytes: &[u8], out: &mut dyn Write) {
        let mut decoder = self.defmt_data.table.new_stream_decoder();

        self.delimiter.feed(bytes, |frame| match frame {
            FrameKind::Defmt(frame) => {
                decoder.received(frame);
                // small reliance on rzcobs internals: we need to feed the terminating zero
                decoder.received(FRAME_END);

                if let Ok(frame) = decoder.decode() {
                    self.defmt_data.print(frame, out);
                } else {
                    log::warn!("Failed to decode defmt frame");
                }
            }
            FrameKind::Raw(bytes) => out.write_all(bytes).unwrap(),
        });
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn framing_prints_raw_data_by_default() {
        let mut parser = FrameDelimiter::new();

        let mut asserted = 0;
        parser.feed(b"hello", |frame| {
            assert_eq!(frame, FrameKind::Raw(b"hello"));
            asserted += 1;
        });
        assert_eq!(asserted, 1);
    }

    #[test]
    fn start_byte_on_end_is_not_part_of_the_raw_sequence() {
        let mut parser = FrameDelimiter::new();

        let mut asserted = 0;
        parser.feed(b"hello\xFF", |frame| {
            assert_eq!(frame, FrameKind::Raw(b"hello"));
            asserted += 1;
        });
        assert_eq!(asserted, 1);
    }

    #[test]
    fn frame_start_on_end_is_not_part_of_the_raw_sequence() {
        let mut parser = FrameDelimiter::new();

        let mut asserted = 0;
        parser.feed(b"hello\xFF\x00", |frame| {
            assert_eq!(frame, FrameKind::Raw(b"hello"));
            asserted += 1;
        });
        assert_eq!(asserted, 1);
    }

    #[test]
    fn process_data_after_frame() {
        let mut parser = FrameDelimiter::new();

        let mut asserted = 0;
        parser.feed(b"\xFF\x00frame data\x00hello", |frame| {
            match asserted {
                0 => assert_eq!(frame, FrameKind::Defmt(b"frame data")),
                1 => assert_eq!(frame, FrameKind::Raw(b"hello")),
                _ => panic!("Too many frames"),
            }
            asserted += 1;
        });
        assert_eq!(asserted, 2);
    }

    #[test]
    fn can_concatenate_partial_defmt_frames() {
        let mut parser = FrameDelimiter::new();

        let mut asserted = 0;
        parser.feed(b"\xFF\x00frame", |_| {
            panic!("Should not have a frame yet");
        });
        parser.feed(b" data\x00\xFF", |frame| {
            assert_eq!(frame, FrameKind::Defmt(b"frame data"));
            asserted += 1;
        });
        parser.feed(b"\x00second frame", |_| {
            panic!("Should not have a frame yet");
        });
        parser.feed(b"\x00last part", |frame| {
            match asserted {
                1 => assert_eq!(frame, FrameKind::Defmt(b"second frame")),
                2 => assert_eq!(frame, FrameKind::Raw(b"last part")),
                _ => panic!("Too many frames"),
            }
            asserted += 1;
        });
        assert_eq!(asserted, 3);
    }

    #[test]
    fn defmt_frames_back_to_back() {
        let mut parser = FrameDelimiter::new();

        let mut asserted = 0;
        parser.feed(b"\xFF\x00frame data1\x00\xFF\x00frame data2\x00", |frame| {
            match asserted {
                0 => assert_eq!(frame, FrameKind::Defmt(b"frame data1")),
                1 => assert_eq!(frame, FrameKind::Defmt(b"frame data2")),
                _ => panic!("Too many frames"),
            }
            asserted += 1;
        });
        assert_eq!(asserted, 2);
    }

    #[test]
    fn output_includes_ff_and_0_bytes() {
        let mut parser = FrameDelimiter::new();

        let mut asserted = 0;
        parser.feed(
            b"some message\xFF with parts of\0 a defmt \0\xFF frame delimiter",
            |frame| {
                assert_eq!(
                    frame,
                    FrameKind::Raw(
                        b"some message\xFF with parts of\0 a defmt \0\xFF frame delimiter"
                    )
                );
                asserted += 1;
            },
        );
        assert_eq!(asserted, 1);
    }
}

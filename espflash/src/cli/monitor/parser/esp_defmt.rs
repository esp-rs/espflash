use std::io::Write;

use crossterm::{style::Print, QueueableCommand};
use defmt_decoder::{Frame, Table};
use miette::{bail, Context, Diagnostic, Result};
use thiserror::Error;

use crate::cli::monitor::parser::InputParser;

#[derive(Clone, Copy, Debug, Diagnostic, Error)]
#[error("Could not set up defmt logger")]
pub enum DefmtError {
    #[error("No elf data available")]
    #[diagnostic(code(espflash::monitor::defmt::no_elf))]
    NoElf,

    #[error("No defmt data was found in the elf file")]
    #[diagnostic(code(espflash::monitor::defmt::no_defmt))]
    NoDefmtData,

    #[error("Failed to parse defmt data")]
    #[diagnostic(code(espflash::monitor::defmt::parse_failed))]
    TableParseFailed,

    #[error("Unsupported defmt encoding: {0:?}. Only rzcobs is supported.")]
    #[diagnostic(code(espflash::monitor::defmt::unsupported_encoding))]
    UnsupportedEncoding(defmt_decoder::Encoding),
}

#[derive(Debug, PartialEq)]
enum FrameKind<'a> {
    Defmt(&'a [u8]),
    Raw(&'a [u8]),
}

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

pub struct EspDefmt {
    delimiter: FrameDelimiter,
    table: Table,
}

impl EspDefmt {
    /// Loads symbols from the ELF file (if provided) and initializes the
    /// context.
    fn load_table(elf: Option<&[u8]>) -> Result<Table> {
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
        if encoding == defmt_decoder::Encoding::Rzcobs {
            Ok(table)
        } else {
            bail!(DefmtError::UnsupportedEncoding(encoding))
        }
    }

    pub fn new(elf: Option<&[u8]>) -> Result<Self> {
        Self::load_table(elf).map(|table| Self {
            delimiter: FrameDelimiter::new(),
            table,
        })
    }

    fn handle_raw(bytes: &[u8], out: &mut dyn Write) {
        out.write_all(bytes).unwrap();
    }

    fn handle_defmt(frame: Frame<'_>, out: &mut dyn Write) {
        out.queue(Print(frame.display(true).to_string())).unwrap();
        out.queue(Print("\r\n")).unwrap();

        out.flush().unwrap();
    }
}

impl InputParser for EspDefmt {
    fn feed(&mut self, bytes: &[u8], out: &mut dyn Write) {
        let mut decoder = self.table.new_stream_decoder();

        self.delimiter.feed(bytes, |frame| match frame {
            FrameKind::Defmt(frame) => {
                decoder.received(frame);
                // small reliance on rzcobs internals: we need to feed the terminating zero
                decoder.received(FRAME_END);

                if let Ok(frame) = decoder.decode() {
                    Self::handle_defmt(frame, out);
                } else {
                    log::warn!("Failed to decode defmt frame");
                }
            }
            FrameKind::Raw(bytes) => Self::handle_raw(bytes, out),
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

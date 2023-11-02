use std::io::Write;

use crossterm::{
    style::{Color, Print, PrintStyledContent, Stylize},
    QueueableCommand,
};
use defmt_decoder::{Frame, Table};

use crate::cli::monitor::parser::InputParser;

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
            } else {
                process(FrameKind::Raw(frame));
            }
            self.in_frame = !self.in_frame;

            self.buffer.drain(..consumed);
        }
    }
}

pub struct EspDefmt {
    delimiter: FrameDelimiter,
    table: Option<Table>,
}

impl EspDefmt {
    fn load_table(elf: Option<&[u8]>) -> Option<Table> {
        // Load symbols from the ELF file (if provided) and initialize the context.
        Table::parse(elf?).ok().flatten().and_then(|table| {
            let encoding = table.encoding();

            // We only support rzcobs encoding because it is the only way to multiplex
            // a defmt stream and an ASCII log stream over the same serial port.
            if encoding == defmt_decoder::Encoding::Rzcobs {
                Some(table)
            } else {
                log::warn!("Unsupported defmt encoding: {:?}", encoding);
                None
            }
        })
    }

    pub fn new(elf: Option<&[u8]>) -> Self {
        Self {
            delimiter: FrameDelimiter {
                buffer: Vec::new(),
                in_frame: false,
            },
            table: Self::load_table(elf),
        }
    }

    fn handle_raw(bytes: &[u8], out: &mut impl Write) {
        out.write_all(bytes).unwrap();
    }

    fn handle_defmt(frame: Frame<'_>, out: &mut impl Write) {
        match frame.level() {
            Some(level) => {
                let color = match level {
                    defmt_parser::Level::Trace => Color::Cyan,
                    defmt_parser::Level::Debug => Color::Blue,
                    defmt_parser::Level::Info => Color::Green,
                    defmt_parser::Level::Warn => Color::Yellow,
                    defmt_parser::Level::Error => Color::Red,
                };

                // Print the level before each line.
                let level = level.as_str().to_uppercase();
                for line in frame.display_message().to_string().lines() {
                    out.queue(PrintStyledContent(
                        format!("[{level}] - {line}\r\n").with(color),
                    ))
                    .unwrap();
                }
            }
            None => {
                out.queue(Print(frame.display_message().to_string()))
                    .unwrap();
                out.queue(Print("\r\n")).unwrap();
            }
        }

        out.flush().unwrap();
    }
}

impl InputParser for EspDefmt {
    fn feed(&mut self, bytes: &[u8], out: &mut impl Write) {
        let Some(table) = self.table.as_mut() else {
            Self::handle_raw(bytes, out);
            return;
        };

        let mut decoder = table.new_stream_decoder();

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

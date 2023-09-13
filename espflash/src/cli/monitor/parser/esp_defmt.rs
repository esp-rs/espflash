use std::io::Write;

use crossterm::{
    style::{Color, Print, PrintStyledContent, Stylize},
    QueueableCommand,
};
use defmt_decoder::{Frame, Table};

use crate::cli::monitor::parser::InputParser;

enum FrameKind<'a> {
    Defmt(Frame<'a>),
    Raw(&'a [u8]),
}

struct FrameDelimiter {
    buffer: Vec<u8>,
    table: Option<Table>,
    in_frame: bool,
}

// Framing info added by esp-println
const FRAME_START: &[u8] = &[0xFF, 0x00];
const FRAME_END: &[u8] = &[0x00];

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

impl FrameDelimiter {
    pub fn feed(&mut self, buffer: &[u8], mut process: impl FnMut(FrameKind<'_>)) {
        let Some(table) = self.table.as_mut() else {
            process(FrameKind::Raw(buffer));
            return;
        };

        let mut decoder = table.new_stream_decoder();

        self.buffer.extend_from_slice(buffer);

        while let Some((frame, consumed)) = search(&self.buffer, self.in_frame) {
            if !self.in_frame {
                process(FrameKind::Raw(frame));
                self.in_frame = true;
            } else {
                decoder.received(frame);
                // small reliance on rzcobs internals: we need to feed the terminating zero
                decoder.received(FRAME_END);
                if let Ok(frame) = decoder.decode() {
                    process(FrameKind::Defmt(frame));
                } else {
                    log::warn!("Failed to decode defmt frame");
                }
                self.in_frame = false;
            };

            self.buffer.drain(..consumed);
        }
    }
}

pub struct EspDefmt {
    delimiter: FrameDelimiter,
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
                table: Self::load_table(elf),
                in_frame: false,
            },
        }
    }
}

impl InputParser for EspDefmt {
    fn feed(&mut self, bytes: &[u8], out: &mut impl Write) {
        self.delimiter.feed(bytes, |frame| match frame {
            FrameKind::Defmt(frame) => {
                match frame.level() {
                    Some(level) => {
                        let color = match level {
                            defmt_parser::Level::Trace => Color::Cyan,
                            defmt_parser::Level::Debug => Color::Blue,
                            defmt_parser::Level::Info => Color::Green,
                            defmt_parser::Level::Warn => Color::Yellow,
                            defmt_parser::Level::Error => Color::Red,
                        };
                        out.queue(PrintStyledContent(
                            format!(
                                "[{}] - {}",
                                level.as_str().to_uppercase(),
                                frame.display_message()
                            )
                            .with(color),
                        ))
                        .unwrap();
                    }
                    None => {
                        out.queue(Print(frame.display_message())).unwrap();
                    }
                };

                // Remember to begin a new line after we have printed this one!
                out.write_all(b"\r\n").unwrap();
            }
            FrameKind::Raw(bytes) => out.write_all(bytes).unwrap(),
        });
    }
}

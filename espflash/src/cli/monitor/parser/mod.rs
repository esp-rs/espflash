use std::{borrow::Cow, io::Write, sync::LazyLock};

use crossterm::{
    style::{Color, Print, PrintStyledContent, Stylize},
    QueueableCommand,
};
use regex::Regex;

use crate::cli::monitor::{line_endings::normalized, symbols::Symbols};

pub mod esp_defmt;
pub mod serial;

pub trait InputParser {
    fn feed(&mut self, bytes: &[u8], out: &mut dyn Write);
}

// Pattern to much a function address in serial output.
static RE_FN_ADDR: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"0[xX][[:xdigit:]]{8}").unwrap());

fn resolve_addresses(
    symbols: &Symbols<'_>,
    line: &str,
    out: &mut dyn Write,
) -> std::io::Result<()> {
    // Check the previous line for function addresses. For each address found,
    // attempt to look up the associated function's name and location and write both
    // to the terminal.
    for matched in RE_FN_ADDR.find_iter(line).map(|m| m.as_str()) {
        // Since our regular expression already confirms that this is a correctly
        // formatted hex literal, we can (fairly) safely assume that it will parse
        // successfully into an integer.
        let addr = u64::from_str_radix(&matched[2..], 16).unwrap();

        let name = symbols.get_name(addr);
        let location = symbols.get_location(addr);

        if let Some(name) = name {
            let output = if line.trim() == format!("0x{:x}", addr) {
                if let Some((file, line_num)) = location {
                    format!("{name}\r\n    at {file}:{line_num}\r\n")
                } else {
                    format!("{name}\r\n    at ??:??\r\n")
                }
            } else if let Some((file, line_num)) = location {
                format!("{matched} - {name}\r\n    at {file}:{line_num}\r\n")
            } else {
                format!("{matched} - {name}\r\n    at ??:??\r\n")
            };

            out.queue(PrintStyledContent(output.with(Color::Yellow)))?;
        }
    }

    Ok(())
}

#[derive(Debug)]
struct Utf8Merger {
    incomplete_utf8_buffer: Vec<u8>,
}

impl Utf8Merger {
    fn new() -> Self {
        Self {
            incomplete_utf8_buffer: Vec::new(),
        }
    }

    fn process_utf8(&mut self, buff: &[u8]) -> String {
        let mut buffer = std::mem::take(&mut self.incomplete_utf8_buffer);
        buffer.extend(normalized(buff.iter().copied()));

        // look for longest slice that we can then lossily convert without introducing
        // errors for partial sequences (#457)
        let mut len = 0;

        loop {
            match std::str::from_utf8(&buffer[len..]) {
                // whole input is valid
                Ok(str) if len == 0 => return String::from(str),

                // input is valid after the last error, and we could ignore the last error, so
                // let's process the whole input
                Ok(_) => return String::from_utf8_lossy(&buffer).to_string(),

                // input has some errors. We can ignore invalid sequences and replace them later,
                // but we have to stop if we encounter an incomplete sequence.
                Err(e) => {
                    len += e.valid_up_to();
                    if let Some(error_len) = e.error_len() {
                        len += error_len;
                    } else {
                        // incomplete sequence. We split it off, save it for later
                        let (bytes, incomplete) = buffer.split_at(len);
                        self.incomplete_utf8_buffer = incomplete.to_vec();
                        return String::from_utf8_lossy(bytes).to_string();
                    }
                }
            }
        }
    }
}

#[allow(missing_debug_implementations)]
pub struct ResolvingPrinter<'ctx, W: Write> {
    writer: W,
    symbols: Option<Symbols<'ctx>>,
    merger: Utf8Merger,
    line_fragment: String,
}

impl<'ctx, W: Write> ResolvingPrinter<'ctx, W> {
    pub fn new(elf: Option<&'ctx [u8]>, writer: W) -> Self {
        Self {
            writer,
            symbols: elf.and_then(|elf| Symbols::try_from(elf).ok()),
            merger: Utf8Merger::new(),
            line_fragment: String::new(),
        }
    }
}

impl<W: Write> Write for ResolvingPrinter<'_, W> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let text = self.merger.process_utf8(buf);

        // Split the text into lines, storing the last of which separately if it is
        // incomplete (ie. does not end with '\n') because these need special handling.
        let mut lines = text.lines().collect::<Vec<_>>();
        let incomplete = if text.ends_with('\n') {
            None
        } else {
            lines.pop()
        };

        // Iterate through all *complete* lines (ie. those ending with '\n') ...
        for line in lines {
            // ... and print the line.
            self.writer.queue(Print(line))?;

            // If there is a previous line fragment, that means that the current line must
            // be appended to it in order to form the complete line. Since we want to look
            // for function addresses in the *entire* previous line we combine these prior
            // to performing the symbol lookup(s).
            let fragment = std::mem::take(&mut self.line_fragment);
            let line = if fragment.is_empty() {
                Cow::from(line)
            } else {
                // The previous fragment has been completed (by this current line).
                Cow::from(format!("{fragment}{line}"))
            };

            // Remember to begin a new line after we have printed this one!
            self.writer.queue(Print("\r\n"))?;

            // If we have loaded some symbols...
            if let Some(symbols) = self.symbols.as_ref() {
                // Try to print the names of addresses in the current line.
                resolve_addresses(symbols, &line, &mut self.writer)?;
            }
        }

        // If there is an incomplete line we will still print it. However, we will not
        // perform function name lookups or terminate it with a newline.
        if let Some(line) = incomplete {
            self.writer.queue(Print(line))?;

            let fragment = std::mem::take(&mut self.line_fragment);
            self.line_fragment = format!("{fragment}{line}");
        }

        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.writer.flush()
    }
}

#[cfg(test)]
mod test {
    use super::Utf8Merger;

    #[test]
    fn returns_valid_strings_immediately() {
        let mut ctx = Utf8Merger::new();
        let buff = b"Hello, world!";
        let text = ctx.process_utf8(buff);
        assert_eq!(text, "Hello, world!");
    }

    #[test]
    fn does_not_repeat_valid_strings() {
        let mut ctx = Utf8Merger::new();
        let text = ctx.process_utf8(b"Hello, world!");
        assert_eq!(text, "Hello, world!");
        let text = ctx.process_utf8(b"Something else");
        assert_eq!(text, "Something else");
    }

    #[test]
    fn replaces_invalid_sequence() {
        let mut ctx = Utf8Merger::new();
        let text = ctx.process_utf8(b"Hello, \xFF world!");
        assert_eq!(text, "Hello, \u{FFFD} world!");
    }

    #[test]
    fn can_replace_unfinished_incomplete_sequence() {
        let mut ctx = Utf8Merger::new();
        let mut incomplete = Vec::from("Hello, ".as_bytes());
        let utf8 = "🙈".as_bytes();
        incomplete.extend_from_slice(&utf8[..utf8.len() - 1]);
        let text = ctx.process_utf8(&incomplete);
        assert_eq!(text, "Hello, ");

        let text = ctx.process_utf8(b" world!");
        assert_eq!(text, "\u{FFFD} world!");
    }

    #[test]
    fn can_merge_incomplete_sequence() {
        let mut ctx = Utf8Merger::new();
        let mut incomplete = Vec::from("Hello, ".as_bytes());
        let utf8 = "🙈".as_bytes();
        incomplete.extend_from_slice(&utf8[..utf8.len() - 1]);

        let text = ctx.process_utf8(&incomplete);
        assert_eq!(text, "Hello, ");

        let text = ctx.process_utf8(&utf8[utf8.len() - 1..]);
        assert_eq!(text, "🙈");
    }

    #[test]
    fn issue_457() {
        let mut ctx = Utf8Merger::new();
        let mut result = String::new();

        result.push_str(&ctx.process_utf8(&[0x48]));
        result.push_str(&ctx.process_utf8(&[0x65, 0x6C, 0x6C]));
        result.push_str(&ctx.process_utf8(&[
            0x6F, 0x20, 0x77, 0x6F, 0x72, 0x6C, 0x64, 0x21, 0x20, 0x77, 0x69, 0x74,
        ]));
        result.push_str(&ctx.process_utf8(&[
            0x68, 0x20, 0x55, 0x54, 0x46, 0x3A, 0x20, 0x77, 0x79, 0x73, 0x79,
        ]));
        result.push_str(&ctx.process_utf8(&[0xC5, 0x82, 0x61, 0x6D, 0x0A]));

        assert_eq!(result, "Hello world! with UTF: wysyłam\r\n");
    }
}

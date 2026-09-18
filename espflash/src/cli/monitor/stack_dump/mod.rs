//! Decoding of stack dumps into backtraces.
//!
//! A panic handler that can't produce a backtrace on the device can instead
//! print the registers and the raw stack memory, and leave the unwinding to
//! the monitor, which has the ELF file and thus the call frame information
//! (see [`cfi_unwind`](super::cfi_unwind)).
//!
//! A [`Collector`] recognizes one dump format in the output, line by line, and
//! turns a dump into [`Registers`] plus a [`StackMemory`], which
//! [`print_backtrace`] unwinds and prints. There is one collector per
//! supported format:
//!
//! - [`esp_backtrace`] for the `STACKDUMP:` line of `esp-backtrace`
//!   (bare-metal).
//! - [`esp_idf`] for the register and stack memory dump of the ESP-IDF panic
//!   handler.

pub(crate) mod esp_backtrace;
pub(crate) mod esp_idf;

use std::io::Write;

use crossterm::{
    QueueableCommand,
    style::{Color, PrintStyledContent, Stylize},
};

use crate::cli::monitor::{
    cfi_unwind::{Frame, Memory, REGISTER_NAMES, Registers, Stop, Unwinder},
    symbols::Symbols,
};

/// The most frames a backtrace is allowed to have.
const MAX_FRAMES: usize = 100;

/// The dumped part of the stack.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct StackMemory {
    /// Runs of consecutive words, each with the address of its first word.
    segments: Vec<(u32, Vec<u32>)>,
}

impl StackMemory {
    /// Adds the words starting at `addr`.
    pub fn push(&mut self, addr: u32, words: &[u32]) {
        if let Some((base, segment)) = self.segments.last_mut()
            && base.wrapping_add(4 * segment.len() as u32) == addr
        {
            segment.extend_from_slice(words);
        } else {
            self.segments.push((addr, words.to_vec()));
        }
    }

    /// The number of dumped bytes.
    pub fn len(&self) -> usize {
        self.segments.iter().map(|(_, words)| 4 * words.len()).sum()
    }
}

impl Memory for StackMemory {
    fn read_u32(&self, addr: u32) -> Option<u32> {
        if !addr.is_multiple_of(4) {
            return None;
        }

        self.segments.iter().find_map(|(base, words)| {
            let index = addr.checked_sub(*base)? / 4;

            words.get(index as usize).copied()
        })
    }
}

/// A complete stack dump, ready to be unwound.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Dump {
    /// The registers at the time the dump was taken.
    pub registers: Registers,
    /// The dumped stack memory.
    pub memory: StackMemory,
}

/// What a [`Collector`] made of a line.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Line {
    /// Not part of a stack dump.
    Other,
    /// Part of a stack dump which is still being collected. Its contents are
    /// data, not code addresses.
    Data,
    /// The line completed a stack dump. `includes_line` tells whether the line
    /// is itself part of the dump or the first line after it.
    Complete {
        dump: Box<Dump>,
        includes_line: bool,
    },
}

/// Recognizes the stack dumps of one format in the output, line by line.
///
/// Collectors for different formats are chained into one by putting them in a
/// tuple: every collector sees every line, and the first one to recognize a
/// line decides what it is.
pub(crate) trait Collector {
    /// Feeds the next complete line of output.
    fn feed(&mut self, line: &str, symbols: &[Symbols<'_>]) -> Line;
}

impl<A: Collector, B: Collector> Collector for (A, B) {
    fn feed(&mut self, line: &str, symbols: &[Symbols<'_>]) -> Line {
        let first = self.0.feed(line, symbols);
        let second = self.1.feed(line, symbols);

        if first == Line::Other { second } else { first }
    }
}

/// The collector for all supported stack dump formats.
pub(crate) fn default_collector() -> impl Collector {
    (esp_backtrace::EspBacktrace, esp_idf::EspIdf::default())
}

/// Unwinds the stack and prints the resulting backtrace.
pub(crate) fn print_backtrace(
    dump: &Dump,
    unwinder: &Unwinder<'_>,
    symbols: &[Symbols<'_>],
    out: &mut dyn Write,
) -> std::io::Result<()> {
    let (frames, stop) = unwinder.unwind(dump.registers.clone(), &dump.memory, MAX_FRAMES);

    let mut text = String::from("\r\nBacktrace (decoded from the stack dump):\r\n");
    for frame in &frames {
        format_frame(frame, symbols, &mut text);
    }

    let note = match stop {
        Stop::EndOfStack => None,
        Stop::ReturnAddressLost(pc) => Some(format!(
            "0x{pc:08x} didn't save the return address of its caller, which couldn't be reconstructed from the stack"
        )),
        Stop::NoUnwindInfo(pc) => Some(format!(
            "no unwind info for 0x{pc:08x}; is the ELF built with debug info?"
        )),
        Stop::MemoryUnavailable(addr) => Some(format!(
            "0x{addr:08x} is outside the dumped stack memory ({} bytes)",
            dump.memory.len()
        )),
        Stop::UnknownRegister(reg) => Some(format!(
            "the value of {} is unknown",
            REGISTER_NAMES.get(reg as usize).unwrap_or(&"a register")
        )),
        Stop::Unsupported(pc) => Some(format!("unsupported unwind rule at 0x{pc:08x}")),
        Stop::TooManyFrames => Some(format!("more than {MAX_FRAMES} frames")),
        Stop::Inconsistent => Some("inconsistent stack frame".to_string()),
    };

    if let Some(note) = note {
        text.push_str(&format!("(backtrace stopped: {note})\r\n"));
    }
    text.push_str("\r\n");

    out.queue(PrintStyledContent(text.with(Color::Yellow)))?;

    Ok(())
}

fn format_frame(frame: &Frame, symbols: &[Symbols<'_>], out: &mut String) {
    let lookup_pc = frame.lookup_pc() as u64;

    let resolved = symbols
        .iter()
        .map(|symbols| symbols.frames(lookup_pc))
        .find(|frames| !frames.is_empty())
        .unwrap_or_default();

    // A frame whose return address was lost and which was reconstructed
    // from the stack (see `cfi_unwind::Unwinder::recover`) is marked as such.
    let marker = if frame.recovered {
        "(reconstructed) "
    } else {
        ""
    };

    if resolved.is_empty() {
        out.push_str(&format!(
            "0x{:08x} - {marker}??\r\n    at ??:??\r\n",
            frame.pc
        ));
        return;
    }

    // Innermost function first, followed by the functions it was inlined into.
    for (index, function) in resolved.iter().enumerate() {
        let name = function.name.as_deref().unwrap_or("??");

        if index == 0 {
            out.push_str(&format!("0x{:08x} - {marker}{name}\r\n", frame.pc));
        } else {
            out.push_str(&format!("    (inlined by) {name}\r\n"));
        }

        match &function.location {
            Some((file, line)) => out.push_str(&format!("    at {file}:{line}\r\n")),
            None => out.push_str("    at ??:??\r\n"),
        }
    }
}

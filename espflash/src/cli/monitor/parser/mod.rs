use std::{borrow::Cow, io::Write, sync::LazyLock};

use crossterm::{
    QueueableCommand,
    style::{Color, Print, PrintStyledContent, Stylize},
};
use regex::Regex;

use crate::cli::monitor::{
    UnwindTables,
    cfi_unwind::Unwinder,
    line_endings::normalized,
    stack_dump::{self, Collector, Dump, Line},
    symbols::Symbols,
};

pub mod esp_defmt;
pub mod serial;

/// Trait for parsing input data.
pub trait InputParser {
    /// Feeds the parser with new data.
    fn feed(&mut self, bytes: &[u8], out: &mut dyn Write);
}

// Pattern to much a function address in serial output.
static RE_FN_ADDR: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"0[xX][[:xdigit:]]{8}").unwrap());

// We won't try to resolve addresses for lines starting with these prefixes.
// Those lines are output from the first stage bootloader mostly about loading
// the 2nd stage bootloader. The resolved addresses are not useful and mostly
// confusing noise.
const SUPPRESS_FOR_LINE_START: &[&str] = &[
    "Saved PC:", // this might be useful to see in some situations
    "load:0x",
    "entry 0x",
];

fn resolve_addresses(
    symbols: &Symbols<'_>,
    line: &str,
    out: &mut dyn Write,
    try_resolve_all_addresses: bool,
) -> std::io::Result<()> {
    // suppress resolving well known misleading addresses
    if !try_resolve_all_addresses && SUPPRESS_FOR_LINE_START.iter().any(|s| line.starts_with(s)) {
        return Ok(());
    }

    // Check the previous line for function addresses. For each address found,
    // attempt to look up the associated function's name and location and write
    // both to the terminal.
    for matched in RE_FN_ADDR.find_iter(line).map(|m| m.as_str()) {
        // Since our regular expression already confirms that this is a
        // correctly formatted hex literal, we can (fairly) safely
        // assume that it will parse successfully into an integer.
        let addr = u64::from_str_radix(&matched[2..], 16).unwrap();

        let name = symbols.name(addr);
        let location = symbols.location(addr);

        if let Some(name) = name {
            let output = if line.trim() == format!("0x{addr:x}") {
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

        // look for longest slice that we can then lossily convert without
        // introducing errors for partial sequences (#457)
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
                        // incomplete sequence. We split it off, save it for
                        // later
                        let (bytes, incomplete) = buffer.split_at(len);
                        self.incomplete_utf8_buffer = incomplete.to_vec();
                        return String::from_utf8_lossy(bytes).to_string();
                    }
                }
            }
        }
    }
}

/// A printer that resolves symbol names and writes formatted output.
#[allow(missing_debug_implementations)]
pub struct ResolvingPrinter<'ctx, W: Write> {
    writer: W,
    symbols: Vec<Symbols<'ctx>>,
    elfs: Vec<&'ctx [u8]>,
    merger: Utf8Merger,
    line_fragment: String,
    disable_address_resolution: bool,
    try_resolve_all_addresses: bool,
    /// Recognizes the stack dumps in the output.
    collector: Box<dyn Collector>,
    /// The unwind tables to decode stack dumps with.
    unwind_tables: UnwindTables,
    /// Built from `elfs` the first time a stack has to be unwound.
    unwinder: Option<Unwinder<'ctx>>,
}

impl<'ctx, W: Write> ResolvingPrinter<'ctx, W> {
    /// Creates a new `ResolvingPrinter` with the given ELF file and writer.
    pub fn new(
        elf: Vec<&'ctx [u8]>,
        writer: W,
        try_resolve_all_addresses: bool,
        unwind_tables: UnwindTables,
    ) -> Self {
        Self {
            writer,
            symbols: elf
                .iter()
                .filter_map(|elf| Symbols::try_from(elf).ok())
                .collect(),
            elfs: elf,
            merger: Utf8Merger::new(),
            line_fragment: String::new(),
            disable_address_resolution: false,
            try_resolve_all_addresses,
            collector: Box::new(stack_dump::default_collector()),
            unwind_tables,
            unwinder: None,
        }
    }

    /// Creates a new `ResolvingPrinter` with address resolution disabled.
    pub fn new_no_addresses(_elf: Option<&'ctx [u8]>, writer: W) -> Self {
        Self {
            writer,
            symbols: Vec::new(), // Don't load symbols when address resolution is disabled
            elfs: Vec::new(),
            merger: Utf8Merger::new(),
            line_fragment: String::new(),
            disable_address_resolution: true,
            try_resolve_all_addresses: false,
            collector: Box::new(stack_dump::default_collector()),
            unwind_tables: UnwindTables::default(),
            unwinder: None,
        }
    }

    /// Unwinds a stack dump and prints the resulting backtrace.
    fn print_backtrace(&mut self, dump: &Dump) -> std::io::Result<()> {
        let unwinder = self
            .unwinder
            .get_or_insert_with(|| Unwinder::new(&self.elfs, self.unwind_tables));

        stack_dump::print_backtrace(dump, unwinder, &self.symbols, &mut self.writer)
    }
}

impl<W: Write> Write for ResolvingPrinter<'_, W> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let text = self.merger.process_utf8(buf);

        // Split the text into lines, storing the last of which separately if it
        // is incomplete (ie. does not end with '\n') because these need
        // special handling.
        let mut lines = text.lines().collect::<Vec<_>>();
        let incomplete = if text.ends_with('\n') {
            None
        } else {
            lines.pop()
        };

        // Iterate through all *complete* lines (ie. those ending with '\n') ...
        for line in lines {
            // If there is a previous line fragment, that means that the current
            // line must be appended to it in order to form the
            // complete line. Since we want to look for function
            // addresses in the *entire* previous line we combine these prior
            // to performing the symbol lookup(s).
            let fragment = std::mem::take(&mut self.line_fragment);
            let full_line = if fragment.is_empty() {
                Cow::from(line)
            } else {
                // The previous fragment has been completed (by this current
                // line).
                Cow::from(format!("{fragment}{line}"))
            };

            // Keep track of the stack dumps in the output.
            let kind = if self.disable_address_resolution {
                Line::Other
            } else {
                self.collector.feed(&full_line, &self.symbols)
            };
            let (dump, line_is_data) = match &kind {
                Line::Other => (None, false),
                Line::Data => (None, true),
                Line::Complete {
                    dump,
                    includes_line,
                } => (Some(dump), *includes_line),
            };

            // A dump which ended before this line is decoded first, so that
            // its backtrace directly follows it.
            if let Some(dump) = dump
                && !line_is_data
            {
                self.print_backtrace(dump)?;
            }

            // ... and print the line.
            self.writer.queue(Print(line))?;

            // Remember to begin a new line after we have printed this one!
            self.writer.queue(Print("\r\n"))?;

            // If we have loaded some symbols and address resolution is not
            // disabled...
            if !self.disable_address_resolution {
                // The contents of a stack dump are data rather than code
                // addresses, so resolving them only produces noise; the
                // backtrace decoded from the dump replaces that.
                if !line_is_data || self.try_resolve_all_addresses {
                    for symbols in &self.symbols {
                        // Try to print the names of addresses in the current
                        // line.
                        resolve_addresses(
                            symbols,
                            &full_line,
                            &mut self.writer,
                            self.try_resolve_all_addresses,
                        )?;
                    }
                }

                // A dump contained in this line is decoded after printing it.
                if let Some(dump) = dump
                    && line_is_data
                {
                    self.print_backtrace(dump)?;
                }
            }
        }

        // If there is an incomplete line we will still print it. However, we
        // will not perform function name lookups or terminate it with a
        // newline.
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
    use std::io::Write;

    use super::{ResolvingPrinter, Utf8Merger};
    use crate::cli::monitor::UnwindTables;

    /// Runs `text` through a `ResolvingPrinter` loaded with the given ELF
    /// files and returns what it printed.
    fn print(elfs: Vec<&[u8]>, text: &str) -> String {
        let mut out = Vec::new();

        let mut printer = ResolvingPrinter::new(elfs, &mut out, false, UnwindTables::Auto);
        printer.write_all(text.as_bytes()).unwrap();
        printer.flush().unwrap();

        String::from_utf8_lossy(&out).into_owned()
    }

    /// The ESP-IDF `hello_world` example built for the ESP32-C61 (see
    /// `tests/data/README.md`), whose CFI lives in `.debug_frame`.
    fn espidf_elf() -> Vec<u8> {
        std::fs::read(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data/esp32c61")).unwrap()
    }

    /// The bare-metal `esp-backtrace` example for the ESP32-C6 (see
    /// `tests/data/README.md`), whose CFI lives in `.eh_frame` and, being
    /// built with `-C force-frame-pointers`, is frame pointer based.
    fn esp_backtrace_elf() -> Vec<u8> {
        std::fs::read(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/data/esp32c6_backtrace"
        ))
        .unwrap()
    }

    /// Builds the `STACKDUMP:` line `esp-backtrace` prints for a stack of
    /// `words` (little-endian, starting at `sp`).
    fn stack_dump_line(pc: u32, words: &[u32]) -> String {
        let mut line = format!("STACKDUMP: {pc:08x} ");
        for word in words {
            for byte in word.to_le_bytes() {
                line.push_str(&format!("{byte:02x}"));
            }
        }
        line.push_str("\r\n");

        line
    }

    /// A stack for the `esp32c6_backtrace` ELF as `esp-backtrace` would dump
    /// it while `core::panicking::panic_bounds_check` (64 byte frame) is
    /// executing, called from `main` (32 byte frame) whose saved `ra` is zero.
    /// Both functions switch their CFA to `s0` after the prologue. `len` is
    /// the number of words to dump; anything not needed by the unwinder is
    /// zero.
    fn esp_backtrace_stack(len: usize, main_s0: u32) -> Vec<u32> {
        let mut words = vec![0; len];
        words[14] = main_s0; // s0 saved by panic_bounds_check = CFA of main
        words[15] = 0x420012cc; // ra saved by panic_bounds_check: after the call in main
        words
    }

    #[test]
    fn decodes_esp_backtrace_dump_with_recovered_sp() {
        // 128 bytes are less than the smallest dump size esp-backtrace can be
        // configured with, so the dump ended at `_stack_start` (0x4086e610 in
        // this ELF), which makes sp = 0x4086e590 and the CFA of main
        // 0x4086e590 + 64 + 32.
        let elf = esp_backtrace_elf();
        let line = stack_dump_line(0x420012e8, &esp_backtrace_stack(32, 0x4086e5f0));
        let output = print(vec![&elf], &line);

        assert_in_order(
            &output,
            &[
                "Backtrace (decoded from the stack dump):\r\n",
                "0x420012e8 - core::panicking::panic_bounds_check\r\n    at ",
                "0x420012cc - main\r\n    at ",
                "main.rs:",
            ],
        );
        assert!(!output.contains("backtrace stopped"), "{output}");
    }

    #[test]
    fn decodes_esp_backtrace_dump_with_placeholder_sp() {
        // A dump of the maximum size doesn't reveal the stack pointer, so the
        // frame pointer based CFA rules have to be replaced by the stack
        // pointer based ones, and the saved `s0` is ignored.
        let elf = esp_backtrace_elf();
        let line = stack_dump_line(0x420012e8, &esp_backtrace_stack(1024, 0xdeadbeef));
        let output = print(vec![&elf], &line);

        assert_in_order(
            &output,
            &[
                "0x420012e8 - core::panicking::panic_bounds_check\r\n",
                "0x420012cc - main\r\n",
            ],
        );
        assert!(!output.contains("backtrace stopped"), "{output}");
    }

    #[test]
    fn esp_backtrace_dump_needs_eh_frame_here() {
        // The bare-metal ELF only has `.eh_frame`, so restricting the unwinder
        // to `.debug_frame` leaves nothing to unwind with.
        let elf = esp_backtrace_elf();
        let line = stack_dump_line(0x420012e8, &esp_backtrace_stack(32, 0x4086e5f0));

        let mut out = Vec::new();
        let mut printer =
            ResolvingPrinter::new(vec![&elf], &mut out, false, UnwindTables::DebugFrame);
        printer.write_all(line.as_bytes()).unwrap();
        printer.flush().unwrap();
        let output = String::from_utf8_lossy(&out).into_owned();

        assert_in_order(
            &output,
            &[
                "0x420012e8 - core::panicking::panic_bounds_check\r\n",
                "(backtrace stopped: no unwind info for 0x420012e8",
            ],
        );
    }

    /// An `abort()` crash dump laid out by hand for the `esp32c61` ELF, with
    /// the return addresses at the places the CFI of each function dictates:
    ///
    /// - `panic_abort` (frameless, `ra` still live) called from
    /// - `esp_system_abort` (16 byte frame, `ra` at the top) called from
    /// - `abort` (112 byte frame) called from
    /// - `app_main` (32 byte frame) called from
    /// - `main_task` (32 byte frame), whose saved `ra` is zero.
    const ESPIDF_ABORT_DUMP: &str = "\
Guru Meditation Error: Core  0 panic'ed (Illegal instruction). Exception was unhandled.

Core  0 register dump:
MEPC    : 0x40803956  RA      : 0x40803914  SP      : 0x40815e10  GP      : 0x4080c000
TP      : 0x00000000  T0      : 0x00000000  T1      : 0x00000000  T2      : 0x00000000
S0/FP   : 0x00000000  S1      : 0x00000000  A0      : 0x00000000  A1      : 0x00000000
A2      : 0x00000000  A3      : 0x00000000  A4      : 0x00000000  A5      : 0x00000000
A6      : 0x00000000  A7      : 0x00000000  S2      : 0x00000000  S3      : 0x00000000
S4      : 0x00000000  S5      : 0x00000000  S6      : 0x00000000  S7      : 0x00000000
S8      : 0x00000000  S9      : 0x00000000  S10     : 0x00000000  S11     : 0x00000000
T3      : 0x00000000  T4      : 0x00000000  T5      : 0x00000000  T6      : 0x00000000
MSTATUS : 0x00001881  MTVEC   : 0x40800001  MCAUSE  : 0x00000002  MTVAL   : 0x00000000
MHARTID : 0x00000000

Stack memory:
40815e10: 0x00000000 0x00000000 0x00000000 0x40808282 0x00000000 0x00000000 0x00000000 0x00000000
40815e30: 0x00000000 0x00000000 0x00000000 0x00000000 0x00000000 0x00000000 0x00000000 0x00000000
40815e50: 0x00000000 0x00000000 0x00000000 0x00000000 0x00000000 0x00000000 0x00000000 0x00000000
40815e70: 0x00000000 0x00000000 0x00000000 0x00000000 0x00000000 0x00000000 0x00000000 0x42005e86
40815e90: 0x00000000 0x00000000 0x00000000 0x00000000 0x00000000 0x00000000 0x00000000 0x4200e598
40815eb0: 0x00000000 0x00000000 0x00000000 0x00000000 0x00000000 0x00000000 0x00000000 0x00000000
40815ed0: 0x00000000 0x00000000 0x00000000 0x00000000 0x00000000 0x00000000 0x00000000 0x00000000
40815ef0: 0x00000000 0x00000000 0x00000000 0x00000000 0x00000000 0x00000000 0x00000000 0x00000000


ELF file SHA256: 0000000000000000

Rebooting...
";

    /// Asserts that `needles` occur in `haystack` in the given order.
    fn assert_in_order(haystack: &str, needles: &[&str]) {
        let mut from = 0;
        for needle in needles {
            match haystack[from..].find(needle) {
                Some(index) => from += index + needle.len(),
                None => panic!("{needle:?} not found after offset {from} in:\n{haystack}"),
            }
        }
    }

    #[test]
    fn decodes_espidf_abort_dump() {
        let elf = espidf_elf();
        let output = print(vec![&elf], ESPIDF_ABORT_DUMP);

        assert_in_order(
            &output,
            &[
                "Backtrace (decoded from the stack dump):\r\n",
                "0x40803956 - panic_abort\r\n    at /Users/playfulfence/esp/esp-idf/components/esp_system/panic.c:491\r\n",
                "0x40803914 - esp_system_abort\r\n    at /Users/playfulfence/esp/esp-idf/components/esp_system/port/esp_system_chip.c:87\r\n",
                "0x40808282 - abort\r\n    at /Users/playfulfence/esp/esp-idf/components/newlib/src/abort.c:38\r\n",
                "0x42005e86 - app_main\r\n    at /Users/playfulfence/esp/esp-idf/examples/get-started/hello_world/main/hello_world_main.c:18\r\n",
                "0x4200e598 - main_task\r\n    at /Users/playfulfence/esp/esp-idf/components/freertos/app_startup.c:208\r\n",
                // The backtrace is printed before the line that ended the dump.
                "ELF file SHA256",
            ],
        );
        assert!(!output.contains("backtrace stopped"), "{output}");

        // The words of the stack memory aren't resolved as addresses, so
        // `app_main` only shows up in the backtrace.
        assert_eq!(output.matches("app_main").count(), 1, "{output}");
    }

    /// Lays out `words` as ESP-IDF prints the stack memory, starting at the
    /// stack pointer of `ESPIDF_ABORT_DUMP`.
    fn espidf_stack_lines(words: &[u32]) -> String {
        let mut stack = String::new();
        for (index, line) in words.chunks(8).enumerate() {
            stack.push_str(&format!("{:08x}:", 0x40815e10 + 32 * index));
            for word in line {
                stack.push_str(&format!(" 0x{word:08x}"));
            }
            stack.push('\n');
        }

        stack
    }

    #[test]
    fn reconstructs_caller_after_lost_return_address() {
        // `abort` (112 byte frame) "returns" into `panic_abort`, which never
        // saves `ra`, so the return address into panic_abort's caller is
        // lost. The stack above panic_abort's frame holds a stale return
        // address first (after a call to `puts`, which doesn't call
        // panic_abort) and then the real one: the return address into `abort`
        // after its call to `esp_system_abort`, which does call panic_abort
        // and has a 16 byte frame. From there the chain continues normally.
        let elf = espidf_elf();
        let mut words = vec![0u32; 80];
        words[27] = 0x40803956; // ra saved by abort (cfa-4): inside panic_abort
        words[28] = 0x42005e86; // stale: app_main after `jal puts`
        words[31] = 0x40808282; // abort after `jal esp_system_abort`
        words[59] = 0x42005e86; // ra saved by abort: app_main
        words[67] = 0x4200e598; // ra saved by app_main: main_task
        let (head, _) = ESPIDF_ABORT_DUMP.split_once("40815e10: ").unwrap();
        let dump = format!("{head}{}\n\nRebooting...\n", espidf_stack_lines(&words))
            .replace("MEPC    : 0x40803956", "MEPC    : 0x40808240");
        let output = print(vec![&elf], &dump);

        assert_in_order(
            &output,
            &[
                "0x40808240 - abort\r\n",
                "0x40803956 - panic_abort\r\n",
                "0x40803914 - (reconstructed) esp_system_abort\r\n    at /Users/playfulfence/esp/esp-idf/components/esp_system/port/esp_system_chip.c:87\r\n",
                "0x40808282 - abort\r\n",
                "0x42005e86 - app_main\r\n",
                "0x4200e598 - main_task\r\n",
            ],
        );
        assert!(!output.contains("backtrace stopped"), "{output}");
        assert_eq!(output.matches("esp_system_abort").count(), 1, "{output}");
    }

    #[test]
    fn reports_lost_return_address() {
        // As above, but with nothing usable on the stack above panic_abort.
        let elf = espidf_elf();
        let mut words = vec![0u32; 40];
        words[27] = 0x40803956;
        words[28] = 0x42005e86;
        let (head, _) = ESPIDF_ABORT_DUMP.split_once("40815e10: ").unwrap();
        let dump = format!("{head}{}\n\nRebooting...\n", espidf_stack_lines(&words))
            .replace("MEPC    : 0x40803956", "MEPC    : 0x40808240");
        let output = print(vec![&elf], &dump);

        assert_in_order(
            &output,
            &[
                "0x40808240 - abort\r\n",
                "0x40803956 - panic_abort\r\n",
                "(backtrace stopped: 0x40803956 didn't save the return address of its caller",
            ],
        );
    }

    #[test]
    fn decodes_espidf_dump_with_bogus_pc() {
        // A jump through a bad pointer: no unwind info for the PC, so the
        // caller is where `ra` points.
        let elf = espidf_elf();
        let dump = ESPIDF_ABORT_DUMP.replace("MEPC    : 0x40803956", "MEPC    : 0x1c80006e");
        let output = print(vec![&elf], &dump);

        assert_in_order(
            &output,
            &[
                "0x1c80006e - ??\r\n    at ??:??\r\n",
                "0x40803914 - esp_system_abort\r\n",
                "0x40808282 - abort\r\n",
                "0x42005e86 - app_main\r\n",
                "0x4200e598 - main_task\r\n",
            ],
        );
        assert!(!output.contains("backtrace stopped"), "{output}");
    }

    #[test]
    fn reports_exhausted_stack_memory() {
        let elf = espidf_elf();
        let (head, _) = ESPIDF_ABORT_DUMP.split_once("40815e50: ").unwrap();
        let output = print(vec![&elf], &format!("{head}\r\nRebooting...\r\n"));

        assert_in_order(
            &output,
            &[
                "0x40803956 - panic_abort\r\n",
                "0x40803914 - esp_system_abort\r\n",
                "0x40808282 - abort\r\n",
                "(backtrace stopped: 0x40815e8c is outside the dumped stack memory (64 bytes))",
            ],
        );
        assert!(!output.contains("app_main"), "{output}");
    }

    #[test]
    fn espidf_dump_without_elf_stops_after_ra() {
        let output = print(Vec::new(), ESPIDF_ABORT_DUMP);

        assert_in_order(
            &output,
            &[
                "Stack memory:",
                "0x40803956 - ??\r\n",
                "0x40803914 - ??\r\n",
                "(backtrace stopped: no unwind info for 0x40803914",
            ],
        );
    }

    /// Decodes a real crash dump from a file. Run with
    /// `ESPFLASH_DUMP=<dump> ESPFLASH_ELF=<elf>[:<elf>...] cargo test -p
    /// espflash decode_espidf_dump_from_env -- --ignored --nocapture`.
    #[test]
    #[ignore = "needs ESPFLASH_DUMP and ESPFLASH_ELF"]
    fn decode_espidf_dump_from_env() {
        let dump = std::fs::read_to_string(std::env::var("ESPFLASH_DUMP").unwrap()).unwrap();
        let elfs = std::env::var("ESPFLASH_ELF")
            .unwrap()
            .split(':')
            .map(|path| std::fs::read(path).unwrap())
            .collect::<Vec<_>>();

        let output = print(elfs.iter().map(|elf| elf.as_slice()).collect(), &dump);

        println!("{output}");
        assert!(output.contains("Backtrace (decoded from the stack dump):"));
    }

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

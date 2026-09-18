//! Collection of the crash dumps printed by ESP-IDF on RISC-V chips.
//!
//! Unless configured to produce a backtrace on its own (with
//! `CONFIG_ESP_SYSTEM_USE_EH_FRAME` or `CONFIG_ESP_SYSTEM_USE_FRAME_POINTER`),
//! the ESP-IDF panic handler prints a register dump followed by a raw dump of
//! the stack memory:
//!
//! ```text
//! Core  0 register dump:
//! MEPC    : 0x1c80006e  RA      : 0x4212bca4  SP      : 0x4087e1f0  GP      : 0x408173f4
//! TP      : 0x00000000  T0      : 0x400228f4  T1      : 0x40800bd0  T2      : 0x00000000
//! ...
//! MHARTID : 0x00000000
//!
//! Stack memory:
//! 4087e1f0: 0xffff8000 0x4227e287 0x421ed0a3 0x4086fb7a 0x422419ca 0xf8fc5606 0xdf1a9ebd 0x00010000
//! 4087e210: 0x24f856d7 0x001f0000 0x00010020 0x000a0020 0x421f0020 0x42000020 0x0008e268 0x001ed084
//! ...
//! ```
//!
//! [`EspIdf`] gathers these lines as they arrive. A stack memory dump without
//! a preceding register dump is left alone, since it can't be unwound without
//! knowing the program counter.

use std::sync::LazyLock;

use regex::Regex;

use super::{Collector, Dump, Line, StackMemory};
use crate::cli::monitor::{
    cfi_unwind::{REGISTER_NAMES, Registers},
    symbols::Symbols,
};

/// Start of the register dump, e.g. `Core  0 register dump:`.
static RE_REGISTER_DUMP_HEADER: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^Core\s+(\d+) register dump:").unwrap());

/// A `NAME : 0xVALUE` pair of a register dump line, e.g. `S0/FP   :
/// 0x4227e2a4`.
static RE_REGISTER: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"([A-Z][A-Z0-9/]*)\s*:\s*0x([[:xdigit:]]{1,8})").unwrap());

/// A line of the stack memory dump: the address followed by eight words.
static RE_STACK_LINE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^([[:xdigit:]]{8}):((?:\s+0x[[:xdigit:]]{8})+)\s*$").unwrap());

const STACK_MEMORY_HEADER: &str = "Stack memory:";

#[derive(Debug, Default, PartialEq, Eq)]
enum State {
    #[default]
    Idle,
    Registers,
    Stack,
}

/// Collects the crash dumps of the ESP-IDF panic handler.
#[derive(Debug, Default)]
pub(crate) struct EspIdf {
    state: State,
    pc: Option<u32>,
    registers: Option<Registers>,
    stack: StackMemory,
}

impl Collector for EspIdf {
    fn feed(&mut self, line: &str, _symbols: &[Symbols<'_>]) -> Line {
        match self.state {
            State::Idle => {
                self.start(line);

                Line::Other
            }
            State::Registers => {
                if RE_REGISTER_DUMP_HEADER.is_match(line) {
                    self.start(line);
                } else if line.starts_with(STACK_MEMORY_HEADER) {
                    if self.registers.is_some() {
                        self.state = State::Stack;
                    } else {
                        self.reset();
                    }
                } else if !self.parse_registers(line) && !line.trim().is_empty() {
                    // Something else follows the register dump, e.g. the
                    // backtrace of a firmware that produces one on its own.
                    self.reset();
                }

                Line::Other
            }
            State::Stack => {
                if self.parse_stack_line(line) {
                    Line::Data
                } else {
                    let dump = self.finish();
                    self.start(line);

                    Line::Complete {
                        dump: Box::new(dump),
                        includes_line: false,
                    }
                }
            }
        }
    }
}

impl EspIdf {
    fn reset(&mut self) {
        *self = Self::default();
    }

    /// Checks whether `line` starts a crash dump and if so, starts collecting.
    fn start(&mut self, line: &str) {
        if RE_REGISTER_DUMP_HEADER.is_match(line) {
            self.reset();
            self.state = State::Registers;
        }
    }

    /// Ends the collection. Only called once the registers are known.
    fn finish(&mut self) -> Dump {
        let dump = Dump {
            registers: self.registers.take().unwrap_or_else(|| Registers::new(0)),
            memory: std::mem::take(&mut self.stack),
        };
        self.reset();

        dump
    }

    /// Parses the `NAME : 0xVALUE` pairs of a register dump line. Returns
    /// `false` if the line isn't one.
    fn parse_registers(&mut self, line: &str) -> bool {
        let mut matched = false;

        for captures in RE_REGISTER.captures_iter(line) {
            let Ok(value) = u32::from_str_radix(&captures[2], 16) else {
                continue;
            };
            matched = true;

            let name = &captures[1];
            if name == "MEPC" {
                self.pc = Some(value);
            } else if let Some(reg) = REGISTER_NAMES.iter().position(|n| *n == name) {
                let pc = self.pc.unwrap_or(0);
                self.registers
                    .get_or_insert_with(|| Registers::new(pc))
                    .set(reg as u16, value);
            }
        }

        if let Some(pc) = self.pc {
            self.registers.get_or_insert_with(|| Registers::new(pc)).pc = pc;
        }

        matched
    }

    /// Parses a line of the stack memory dump. Returns `false` if the line
    /// isn't one.
    fn parse_stack_line(&mut self, line: &str) -> bool {
        let Some(captures) = RE_STACK_LINE.captures(line) else {
            return false;
        };

        let Ok(addr) = u32::from_str_radix(&captures[1], 16) else {
            return false;
        };

        let words = captures[2]
            .split_whitespace()
            .filter_map(|word| u32::from_str_radix(&word[2..], 16).ok())
            .collect::<Vec<_>>();

        self.stack.push(addr, &words);

        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::monitor::cfi_unwind::{Memory, REG_RA, REG_SP};

    const DUMP: &str = "\
Guru Meditation Error: Core  0 panic'ed (Instruction access fault). Exception was unhandled.

Core  0 register dump:
MEPC    : 0x1c80006e  RA      : 0x4212bca4  SP      : 0x4087e1f0  GP      : 0x408173f4
TP      : 0x00000000  T0      : 0x400228f4  T1      : 0x40800bd0  T2      : 0x00000000
S0/FP   : 0x4227e2a4  S1      : 0x4227e000  A0      : 0x00000000  A1      : 0x00000004
A2      : 0x00000019  A3      : 0x00000000  A4      : 0x000000e9  A5      : 0x1c80006f
A6      : 0xa0000000  A7      : 0x0000000a  S2      : 0xffff8000  S3      : 0x40800810
S4      : 0x42000000  S5      : 0x43000000  S6      : 0x000a0020  S7      : 0x001ed084
S8      : 0x0008e268  S9      : 0x421f0000  S10     : 0x00000000  S11     : 0x00000000
T3      : 0x00000000  T4      : 0x00280000  T5      : 0x00180000  T6      : 0x0000001b
MSTATUS : 0x00001881  MTVEC   : 0x40800001  MCAUSE  : 0x00000001  MTVAL   : 0x1c80006e
MHARTID : 0x00000000

Stack memory:
4087e1f0: 0xffff8000 0x4227e287 0x421ed0a3 0x4086fb7a 0x422419ca 0xf8fc5606 0xdf1a9ebd 0x00010000
4087e210: 0x24f856d7 0x001f0000 0x00010020 0x000a0020 0x421f0020 0x42000020 0x0008e268 0x001ed084


ELF file SHA256: 0000000000000000

Rebooting...
";

    /// Feeds `text` line by line, returning what each line was and the dumps
    /// completed along the way.
    fn collect(text: &str) -> (Vec<Line>, Vec<Dump>) {
        let mut collector = EspIdf::default();
        let mut lines = Vec::new();
        let mut dumps = Vec::new();

        for line in text.lines() {
            let fed = collector.feed(line, &[]);
            if let Line::Complete {
                dump,
                includes_line,
            } = &fed
            {
                assert!(!includes_line);
                dumps.push((**dump).clone());
            }
            lines.push(fed);
        }

        (lines, dumps)
    }

    #[test]
    fn collects_registers_and_stack() {
        let (lines, dumps) = collect(DUMP);
        let [dump] = dumps.as_slice() else {
            panic!("expected one dump, got {}", dumps.len());
        };

        let registers = &dump.registers;
        assert_eq!(registers.pc, 0x1c80006e);
        assert!(!registers.pc_is_return_address);
        assert!(!registers.sp_is_placeholder);
        assert_eq!(registers.get(REG_RA), Some(0x4212bca4));
        assert_eq!(registers.get(REG_SP), Some(0x4087e1f0));
        assert_eq!(registers.get(8), Some(0x4227e2a4)); // S0/FP
        assert_eq!(registers.get(31), Some(0x1b)); // T6
        assert_eq!(registers.get(0), Some(0));

        assert_eq!(dump.memory.len(), 64);
        assert_eq!(dump.memory.read_u32(0x4087e1f0), Some(0xffff8000));
        assert_eq!(dump.memory.read_u32(0x4087e1f4), Some(0x4227e287));
        assert_eq!(dump.memory.read_u32(0x4087e22c), Some(0x001ed084));
        assert_eq!(dump.memory.read_u32(0x4087e230), None);
        assert_eq!(dump.memory.read_u32(0x4087e1ec), None);
        assert_eq!(dump.memory.read_u32(0x4087e1f1), None);

        // Only the stack memory lines are data; the dump completes on the
        // first line after them.
        let data_lines = lines.iter().filter(|line| **line == Line::Data).count();
        assert_eq!(data_lines, 2);
        assert!(matches!(lines[17], Line::Complete { .. }));
        assert!(
            lines
                .iter()
                .enumerate()
                .all(|(index, line)| *line == Line::Other || (15..=17).contains(&index))
        );
    }

    #[test]
    fn stack_memory_without_registers_is_not_collected() {
        let text = "Stack memory:\n40800000: 0x00000001 0x00000002\n\n";
        let (lines, dumps) = collect(text);

        assert!(dumps.is_empty());
        assert!(lines.iter().all(|line| *line == Line::Other));
    }

    #[test]
    fn unrelated_output_after_registers_aborts_collection() {
        let text = "Core  1 register dump:\nMEPC    : 0x40800000  RA      : 0x40800004\nBacktrace: 0x40800000:0x40800000\n\n";
        let (lines, dumps) = collect(text);

        assert!(dumps.is_empty());
        assert!(lines.iter().all(|line| *line == Line::Other));
    }

    #[test]
    fn back_to_back_dumps() {
        let text = "Core  0 register dump:\nMEPC    : 0x40800000  RA      : 0x40800004  SP      : 0x40810000\n\nStack memory:\n40810000: 0x00000001 0x00000002\nCore  1 register dump:\nMEPC    : 0x40800010  RA      : 0x40800014  SP      : 0x40820000\n\nStack memory:\n40820000: 0x00000003 0x00000004\n\n";
        let (_, dumps) = collect(text);

        assert_eq!(dumps.len(), 2);
        assert_eq!(dumps[0].registers.pc, 0x40800000);
        assert_eq!(dumps[0].memory.read_u32(0x40810004), Some(2));
        assert_eq!(dumps[1].registers.pc, 0x40800010);
        assert_eq!(dumps[1].memory.read_u32(0x40820000), Some(3));
    }
}

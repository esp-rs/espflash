//! Collection of the stack dump line printed by `esp-backtrace`.

use super::{Collector, Dump, Line, StackMemory};
use crate::cli::monitor::{
    cfi_unwind::{REG_SP, Registers},
    symbols::Symbols,
};

/// Start of the stack dump line printed by `esp-backtrace`.
pub(crate) const MARKER: &str = "STACKDUMP: ";

/// The smallest stack dump size `esp-backtrace` can be configured with
/// (`stack_dump_max_size_4k`). A shorter dump ended at the top of the stack.
const MIN_MAX_STACK_DUMP_SIZE: usize = 4 * 1024;

/// The stack pointer used when the dump doesn't reveal its real value.
const PLACEHOLDER_SP: u32 = 0x1000_0000;

/// Collects the stack dump line of `esp-backtrace`:
///
/// ```text
/// STACKDUMP: <pc> <stack bytes as hex, starting at sp>
/// ```
///
/// `pc` is the return address of the call which took the snapshot. The dump
/// extends up to the top of the stack or up to a configured maximum size,
/// whichever comes first; in the former case the stack pointer is recovered
/// from the `_stack_start` symbol, otherwise a placeholder is used.
#[derive(Debug, Default)]
pub(crate) struct EspBacktrace;

impl Collector for EspBacktrace {
    fn feed(&mut self, line: &str, symbols: &[Symbols<'_>]) -> Line {
        match parse(line, symbols) {
            Some(dump) => Line::Complete {
                dump: Box::new(dump),
                includes_line: true,
            },
            None => Line::Other,
        }
    }
}

/// Parses the stack dump line of `esp-backtrace`, see [`EspBacktrace`].
fn parse(line: &str, symbols: &[Symbols<'_>]) -> Option<Dump> {
    let (pc, bytes) = line.strip_prefix(MARKER)?.trim().split_once(' ')?;

    let pc = u32::from_str_radix(pc, 16).ok()?;
    let bytes = (0..bytes.len() / 2)
        .map(|index| u8::from_str_radix(&bytes[2 * index..2 * index + 2], 16))
        .collect::<Result<Vec<_>, _>>()
        .ok()?;

    let stack_start = symbols
        .iter()
        .find_map(|symbols| symbols.symbol_address("_stack_start"));

    let mut registers = Registers::new(pc);
    registers.pc_is_return_address = true;

    let sp = match stack_start {
        Some(stack_start) if bytes.len() < MIN_MAX_STACK_DUMP_SIZE => {
            (stack_start as u32).wrapping_sub(bytes.len() as u32)
        }
        _ => {
            registers.sp_is_placeholder = true;

            PLACEHOLDER_SP
        }
    };
    registers.set(REG_SP, sp);

    let words = bytes
        .as_chunks::<4>()
        .0
        .iter()
        .map(|word| u32::from_le_bytes(*word))
        .collect::<Vec<_>>();
    let mut memory = StackMemory::default();
    memory.push(sp, &words);

    Some(Dump { registers, memory })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::monitor::cfi_unwind::Memory;

    #[test]
    fn parses_stack_dump_line_without_symbols() {
        let Dump { registers, memory } =
            parse("STACKDUMP: 420012e8 01000000cc120042\n", &[]).unwrap();

        assert_eq!(registers.pc, 0x420012e8);
        assert!(registers.pc_is_return_address);
        assert!(registers.sp_is_placeholder);
        let sp = registers.get(REG_SP).unwrap();
        assert_eq!(memory.len(), 8);
        assert_eq!(memory.read_u32(sp), Some(1));
        assert_eq!(memory.read_u32(sp + 4), Some(0x420012cc));
        assert_eq!(memory.read_u32(sp + 8), None);
    }

    #[test]
    fn esp_backtrace_collector_completes_on_the_line_itself() {
        let mut collector = EspBacktrace;

        assert_eq!(collector.feed("Hello, world!", &[]), Line::Other);
        assert!(matches!(
            collector.feed("STACKDUMP: 420012e8 01000000", &[]),
            Line::Complete {
                includes_line: true,
                ..
            }
        ));
    }

    #[test]
    fn rejects_malformed_stack_dump_lines() {
        assert!(parse("STACKDUMP: 420012e8", &[]).is_none());
        assert!(parse("STACKDUMP: xyz 01000000", &[]).is_none());
        assert!(parse("STACKDUMP: 420012e8 0100zz00", &[]).is_none());
        assert!(parse("Backtrace: 0x420012e8", &[]).is_none());
    }
}

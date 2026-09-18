//! Stack unwinding driven by DWARF call frame information (CFI).
//!
//! Given a snapshot of the CPU registers and a window of stack memory, both as
//! printed by a panic handler, [`Unwinder`] walks the call chain by evaluating
//! the unwind tables of the ELF files it was built from. Since it relies on
//! CFI rather than on frame pointers it works with code built without
//! `-C force-frame-pointers`.
//!
//! The CFI is read from `.debug_frame`, which the compiler emits together with
//! debug info, or from `.eh_frame`, which is emitted instead when unwind tables
//! are requested (`-C force-unwind-tables`). Both encode the same rules.
//!
//! The register numbering follows the DWARF register mapping for RISC-V, where
//! `x0`..`x31` are numbered `0`..`31`.

use std::{collections::HashMap, hash::Hash};

use gimli::{
    BaseAddresses,
    CfaRule,
    CieOrFde,
    CommonInformationEntry,
    DebugFrame,
    EhFrame,
    EndianSlice,
    FrameDescriptionEntry,
    Register,
    RegisterRule,
    RunTimeEndian,
    UnwindContext,
    UnwindSection,
};
use object::{Object, ObjectSection, SectionKind};

use crate::cli::monitor::UnwindTables;

type Reader<'a> = EndianSlice<'a, RunTimeEndian>;

/// DWARF register number of the return address register (`x1`/`ra`).
pub(crate) const REG_RA: u16 = 1;
/// DWARF register number of the stack pointer (`x2`/`sp`).
pub(crate) const REG_SP: u16 = 2;
/// Number of general purpose registers.
pub(crate) const REG_COUNT: usize = 32;

/// The ABI names of the general purpose registers, indexed by their DWARF
/// register number.
pub(crate) const REGISTER_NAMES: [&str; REG_COUNT] = [
    "ZERO", "RA", "SP", "GP", "TP", "T0", "T1", "T2", "S0/FP", "S1", "A0", "A1", "A2", "A3", "A4",
    "A5", "A6", "A7", "S2", "S3", "S4", "S5", "S6", "S7", "S8", "S9", "S10", "S11", "T3", "T4",
    "T5", "T6",
];

/// Registers which a called function must preserve (RISC-V calling
/// convention): `sp`, `gp`, `tp` and `s0`..`s11`. Their values survive into the
/// caller's frame unless the unwind rules say otherwise.
fn is_callee_saved(reg: u16) -> bool {
    matches!(reg, 2..=4 | 8..=9 | 18..=27)
}

/// A snapshot of the general purpose registers, with `None` for registers
/// whose value isn't known.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Registers {
    /// The program counter.
    pub pc: u32,
    /// Whether `pc` is the return address of a call rather than the address
    /// of the executing instruction, e.g. because the snapshot was taken by a
    /// helper function.
    pub pc_is_return_address: bool,
    /// Whether the stack pointer is a placeholder rather than its real value.
    /// Only offsets from a placeholder are meaningful, so it can't be combined
    /// with other registers holding stack addresses (like the frame pointer).
    pub sp_is_placeholder: bool,
    regs: [Option<u32>; REG_COUNT],
}

impl Registers {
    /// Creates a snapshot where only the program counter is known.
    pub fn new(pc: u32) -> Self {
        Self {
            pc,
            pc_is_return_address: false,
            sp_is_placeholder: false,
            regs: [None; REG_COUNT],
        }
    }

    /// Returns the value of register `reg`, if known.
    pub fn get(&self, reg: u16) -> Option<u32> {
        match reg {
            0 => Some(0),
            _ => self.regs.get(reg as usize).copied().flatten(),
        }
    }

    /// Sets the value of register `reg`.
    pub fn set(&mut self, reg: u16, value: u32) {
        if let Some(slot) = self.regs.get_mut(reg as usize) {
            *slot = Some(value);
        }
    }

    /// Marks register `reg` as unknown.
    pub fn clear(&mut self, reg: u16) {
        if let Some(slot) = self.regs.get_mut(reg as usize) {
            *slot = None;
        }
    }
}

/// Memory the unwinder can read; typically the dumped part of the stack.
pub(crate) trait Memory {
    /// Reads the little-endian 32-bit word at `addr`, if that address is
    /// available.
    fn read_u32(&self, addr: u32) -> Option<u32>;
}

/// One frame of the unwound call chain.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Frame {
    /// The address of the executing instruction, or the return address into
    /// the frame's function (see `is_return_address`).
    pub pc: u32,
    /// The stack pointer while the frame's function was executing.
    pub sp: u32,
    /// Whether `pc` is a return address rather than the address of the
    /// executing instruction.
    pub is_return_address: bool,
    /// Whether the frame was reconstructed by scanning the stack, because its
    /// callee didn't save the return address into it (see
    /// [`Stop::ReturnAddressLost`]).
    pub recovered: bool,
}

impl Frame {
    /// The address to use for symbol and line lookups.
    ///
    /// A return address points to the instruction *after* the call, which may
    /// belong to a different source line or even to a different (inlined)
    /// function. Moving it back into the call instruction makes lookups land
    /// on the call itself.
    pub fn lookup_pc(&self) -> u32 {
        if self.is_return_address {
            self.pc.wrapping_sub(1)
        } else {
            self.pc
        }
    }
}

/// Why unwinding stopped.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Stop {
    /// The outermost frame was reached: the return address is zero or the
    /// unwind rules mark it as undefined.
    EndOfStack,
    /// The function at the given address didn't save the return address of
    /// its caller (functions which never return may skip that), and the
    /// caller couldn't be reconstructed from the stack.
    ReturnAddressLost(u32),
    /// No unwind information covers the given address.
    NoUnwindInfo(u32),
    /// A value needed for unwinding lives at the given address, which isn't
    /// part of the available memory.
    MemoryUnavailable(u32),
    /// The value of the given register is needed but unknown.
    UnknownRegister(u16),
    /// The unwind rules at the given address use a DWARF feature the unwinder
    /// doesn't support (e.g. DWARF expressions).
    Unsupported(u32),
    /// The frame limit was reached.
    TooManyFrames,
    /// The computed caller frame doesn't sit above the current one on the
    /// stack, which means that the input is corrupt.
    Inconsistent,
}

/// The unwind rules in effect at some address.
struct Row {
    /// The address range of the function the rules belong to.
    function: std::ops::Range<u32>,
    /// Whether the function saves `ra` anywhere. Functions which never
    /// return may skip that altogether.
    function_saves_ra: bool,
    cfa: CfaRule<usize>,
    registers: Vec<(Register, RegisterRule<usize>)>,
    /// The offset of the last stack pointer based CFA rule of the function
    /// before (or at) the address. Functions using a frame pointer switch
    /// their CFA rule to it once it is set up; when its value isn't known,
    /// this is the next best thing.
    sp_cfa_offset: Option<i64>,
}

/// An indexed unwind section.
trait CfiTable {
    /// Returns the unwind rules in effect at `pc`, if the section covers it.
    fn row(&self, pc: u64) -> Option<Row>;
}

struct Table<'a, S: UnwindSection<Reader<'a>>> {
    section: S,
    bases: BaseAddresses,
    /// All FDEs of the section, sorted by their start address.
    fdes: Vec<FrameDescriptionEntry<Reader<'a>>>,
}

impl<'a, S> Table<'a, S>
where
    S: UnwindSection<Reader<'a>>,
    S::Offset: Hash + Eq + Copy,
{
    fn new(section: S, bases: BaseAddresses) -> Self {
        let mut fdes = Vec::new();
        let mut cies: HashMap<S::Offset, gimli::Result<CommonInformationEntry<Reader<'a>>>> =
            HashMap::new();

        let mut entries = section.entries(&bases);

        // Stop at the first malformed entry and keep what was parsed so far.
        while let Ok(Some(entry)) = entries.next() {
            let CieOrFde::Fde(partial) = entry else {
                continue;
            };

            let fde = partial.parse(|section, bases, offset| {
                cies.entry(offset)
                    .or_insert_with(|| section.cie_from_offset(bases, offset))
                    .clone()
            });

            if let Ok(fde) = fde
                && fde.len() > 0
            {
                fdes.push(fde);
            }
        }

        fdes.sort_by_key(|fde| fde.initial_address());

        Self {
            section,
            bases,
            fdes,
        }
    }
}

impl<'a, S: UnwindSection<Reader<'a>>> CfiTable for Table<'a, S> {
    fn row(&self, pc: u64) -> Option<Row> {
        // FDEs normally don't overlap, so the one covering `pc` is the last
        // one starting at or before it. Check a few of its predecessors too,
        // in case some FDE is nested in a larger one.
        let end = self.fdes.partition_point(|fde| fde.initial_address() <= pc);
        let fde = self.fdes[..end]
            .iter()
            .rev()
            .take(4)
            .find(|fde| fde.contains(pc))?;

        let mut ctx = UnwindContext::new();
        let mut table = fde.rows(&self.section, &self.bases, &mut ctx).ok()?;
        let mut sp_cfa_offset = None;
        let mut found = None;
        let mut function_saves_ra = false;

        while let Ok(Some(row)) = table.next_row() {
            if row.registers().any(|(register, _)| register.0 == REG_RA) {
                function_saves_ra = true;
            }

            if found.is_some() {
                continue;
            }

            if let CfaRule::RegisterAndOffset { register, offset } = row.cfa()
                && register.0 == REG_SP
            {
                sp_cfa_offset = Some(*offset);
            }

            if row.contains(pc) {
                found = Some((
                    row.cfa().clone(),
                    row.registers()
                        .map(|(register, rule)| (*register, rule.clone()))
                        .collect::<Vec<_>>(),
                ));
            }
        }

        let (cfa, registers) = found?;

        Some(Row {
            function: fde.initial_address() as u32..fde.end_address() as u32,
            function_saves_ra,
            cfa,
            registers,
            sp_cfa_offset,
        })
    }
}

/// A stack unwinder using the CFI of one or more ELF files.
pub(crate) struct Unwinder<'a> {
    tables: Vec<Box<dyn CfiTable + 'a>>,
    /// The executable sections, as (address, contents).
    code: Vec<(u64, &'a [u8])>,
}

/// What [`Unwinder::step`] can fail with.
enum Failure {
    Stop(Stop),
    /// The frame's function didn't save the return address of its caller.
    /// Carries the frame's CFA and function start.
    ReturnAddressLost {
        cfa: u32,
        function_start: u32,
    },
}

impl From<Stop> for Failure {
    fn from(stop: Stop) -> Self {
        Self::Stop(stop)
    }
}

impl<'a> Unwinder<'a> {
    /// Indexes the unwind sections selected by `tables` of the given ELF
    /// files. Files or sections which can't be parsed are skipped.
    ///
    /// Within an ELF file, `.debug_frame` takes precedence over `.eh_frame`;
    /// the files are consulted in the given order.
    pub fn new(elfs: &[&'a [u8]], tables: UnwindTables) -> Self {
        let use_debug_frame = matches!(tables, UnwindTables::Auto | UnwindTables::DebugFrame);
        let use_eh_frame = matches!(tables, UnwindTables::Auto | UnwindTables::EhFrame);

        let mut tables: Vec<Box<dyn CfiTable + 'a>> = Vec::new();
        let mut code = Vec::new();

        for elf in elfs {
            let Ok(file) = object::File::parse(*elf) else {
                continue;
            };

            for section in file.sections() {
                if section.kind() == SectionKind::Text
                    && let Ok(data) = section.data()
                    && !data.is_empty()
                {
                    code.push((section.address(), data));
                }
            }

            let endian = if file.is_little_endian() {
                RunTimeEndian::Little
            } else {
                RunTimeEndian::Big
            };
            let address_size = file
                .architecture()
                .address_size()
                .map(|size| size.bytes())
                .unwrap_or(4);

            if use_debug_frame && let Some(data) = section_data(&file, ".debug_frame") {
                let mut section = DebugFrame::new(data, endian);
                section.set_address_size(address_size);

                tables.push(Box::new(Table::new(section, BaseAddresses::default())));
            }

            if use_eh_frame
                && let Some(section) = file.section_by_name(".eh_frame")
                && let Ok(data) = section.data()
                && !data.is_empty()
            {
                // `.eh_frame` uses PC-relative pointer encodings, so the
                // section addresses are needed to decode it.
                let mut bases = BaseAddresses::default().set_eh_frame(section.address());
                if let Some(hdr) = file.section_by_name(".eh_frame_hdr") {
                    bases = bases.set_eh_frame_hdr(hdr.address());
                }
                if let Some(text) = file.section_by_name(".text") {
                    bases = bases.set_text(text.address());
                }
                if let Some(got) = file.section_by_name(".got") {
                    bases = bases.set_got(got.address());
                }

                let mut section = EhFrame::new(data, endian);
                section.set_address_size(address_size);

                tables.push(Box::new(Table::new(section, bases)));
            }
        }

        Self { tables, code }
    }

    fn row(&self, pc: u64) -> Option<Row> {
        self.tables.iter().find_map(|table| table.row(pc))
    }

    /// Reads `N` bytes of code at `addr`.
    fn code<const N: usize>(&self, addr: u32) -> Option<[u8; N]> {
        let addr = addr as u64;

        self.code.iter().find_map(|(start, data)| {
            let offset = addr.checked_sub(*start)? as usize;

            data.get(offset..offset + N)?.try_into().ok()
        })
    }

    /// Returns the call instruction ending right before `return_address`, if
    /// there is one.
    fn call_before(&self, return_address: u32) -> Option<Call> {
        // A 32-bit `jal ra` or `jalr ra`.
        if let Some(insn) = self.code::<4>(return_address.wrapping_sub(4)) {
            let insn = u32::from_le_bytes(insn);
            let call_pc = return_address.wrapping_sub(4);

            if insn & 0xfff == 0x0ef {
                return Some(Call {
                    target: Some(call_pc.wrapping_add(j_immediate(insn) as u32)),
                });
            }

            if insn & 0x7fff == 0x00e7 {
                let rs1 = (insn >> 15) & 0x1f;
                let offset = (insn as i32) >> 20;

                // `auipc ra, hi` + `jalr ra, lo(ra)` is how far calls are
                // made; anything else is a call through a register.
                let target = self
                    .code::<4>(call_pc.wrapping_sub(4))
                    .map(u32::from_le_bytes)
                    .filter(|auipc| rs1 == REG_RA as u32 && auipc & 0xfff == 0x097)
                    .map(|auipc| {
                        call_pc
                            .wrapping_sub(4)
                            .wrapping_add(auipc & 0xffff_f000)
                            .wrapping_add(offset as u32)
                    });

                return Some(Call { target });
            }
        }

        // A 16-bit `c.jal` (RV32 only) or `c.jalr`.
        if let Some(insn) = self.code::<2>(return_address.wrapping_sub(2)) {
            let insn = u16::from_le_bytes(insn);
            let call_pc = return_address.wrapping_sub(2);

            if insn & 0xe003 == 0x2001 {
                return Some(Call {
                    target: Some(call_pc.wrapping_add(cj_immediate(insn) as u32)),
                });
            }

            if insn & 0xf07f == 0x9002 && (insn >> 7) & 0x1f != 0 {
                return Some(Call { target: None });
            }
        }

        None
    }

    /// Lists the calls in the given code range as (return address, target).
    fn calls(&self, range: std::ops::Range<u32>) -> Vec<(u32, Option<u32>)> {
        let mut calls = Vec::new();
        let mut pc = range.start;

        while pc < range.end {
            let Some(insn) = self.code::<2>(pc) else {
                break;
            };
            let insn = u16::from_le_bytes(insn);
            let len = if insn & 3 == 3 { 4 } else { 2 };
            let return_address = pc.wrapping_add(len);

            if let Some(call) = self.call_before(return_address) {
                calls.push((return_address, call.target));
            }

            pc = return_address;
        }

        calls
    }

    /// Find a chain of calls leading from the given code range to `target`:
    /// either directly, or through up to `depth` intermediate functions which
    /// don't save `ra` at their call (never-returning wrappers, which is what
    /// loses return addresses in the first place).
    ///
    /// Returns, for each call along the chain starting with the one in the
    /// given range, its return address and the (stack pointer based) CFA
    /// offset of the calling function at that point.
    fn call_chain(
        &self,
        range: std::ops::Range<u32>,
        target: u32,
        depth: usize,
    ) -> Option<Vec<(u32, u32)>> {
        let calls = self.calls(range);

        let link = |return_address: u32| {
            let row = self.row(return_address.wrapping_sub(1) as u64)?;
            let offset = row.sp_cfa_offset? as u32;
            Some((return_address, offset))
        };

        if let Some((return_address, _)) = calls.iter().find(|(_, t)| *t == Some(target)) {
            let (return_address, offset) = link(*return_address)?;

            return Some(vec![(return_address, offset)]);
        }

        if depth == 0 {
            return None;
        }

        for (return_address, callee) in calls {
            let Some(callee) = callee else {
                continue;
            };
            let Some((return_address, offset)) = link(return_address) else {
                continue;
            };
            let Some(callee_entry) = self.row(callee as u64) else {
                continue;
            };

            // Only follow calls into functions which never save `ra`: those
            // are the never-returning wrappers which lose return addresses,
            // and they are small, which keeps the search cheap.
            if callee_entry.function_saves_ra {
                continue;
            }
            let Some(mut chain) = self.call_chain(callee_entry.function.clone(), target, depth - 1)
            else {
                continue;
            };
            chain.insert(0, (return_address, offset));

            return Some(chain);
        }

        None
    }

    /// Reconstruct the callers of the function starting at `function_start`,
    /// whose frame ends at `cfa`, after that function lost the return address
    /// into its caller (see [`Failure::ReturnAddressLost`]).
    ///
    /// The callers' frames sit right above `cfa`, and the first caller which
    /// saved its own return address did so at the top of its frame. That
    /// address is found by scanning the stack upwards for a word which looks
    /// like a return address, i.e. follows a call. To reject stale words, the
    /// call must target a function which itself calls `function_start` (if
    /// need be through other functions which don't save `ra`), and whose
    /// frame size per its own CFI places its saved return address exactly
    /// where the word was found.
    ///
    /// Returns the reconstructed frames (innermost first) and the registers
    /// of the frame above them.
    fn recover(
        &self,
        cfa: u32,
        function_start: u32,
        memory: &dyn Memory,
        sp_is_placeholder: bool,
    ) -> Option<(Vec<Frame>, Registers)> {
        /// How many functions without a saved `ra` may sit between the
        /// function which lost the return address and the frame which saved
        /// one. Rust's panic path has four; the bound only guards against
        /// runaway searches.
        const MAX_INTERMEDIATE: usize = 8;

        let mut addr = cfa;

        while let Some(word) = memory.read_u32(addr) {
            let word_addr = addr;
            addr = addr.wrapping_add(4);

            let Some(Call {
                target: Some(caller_start),
            }) = self.call_before(word)
            else {
                continue;
            };

            // The caller has CFI and does call our function...
            let Some(caller_entry) = self.row(caller_start as u64) else {
                continue;
            };
            let Some(chain) = self.call_chain(
                caller_entry.function.clone(),
                function_start,
                MAX_INTERMEDIATE,
            ) else {
                continue;
            };

            // ... and the frames along the chain, as they are at those
            // calls, place the caller's saved return address at the word.
            let mut frames = Vec::new();
            let mut sp = cfa;
            for (return_address, offset) in chain[1..].iter().rev() {
                frames.push(Frame {
                    pc: *return_address,
                    sp,
                    is_return_address: true,
                    recovered: true,
                });
                sp = sp.wrapping_add(*offset);
            }

            let (caller_pc, frame_size) = chain[0];
            let caller_cfa = sp.wrapping_add(frame_size);
            let Some(caller_row) = self.row(caller_pc.wrapping_sub(1) as u64) else {
                continue;
            };
            let ra_rule = caller_row
                .registers
                .iter()
                .find(|(register, _)| register.0 == REG_RA)
                .map(|(_, rule)| rule.clone());
            let Some(RegisterRule::Offset(offset)) = ra_rule else {
                continue;
            };
            if caller_cfa.wrapping_add(offset as u32) != word_addr {
                continue;
            }

            frames.push(Frame {
                pc: caller_pc,
                sp,
                is_return_address: true,
                recovered: true,
            });

            let mut registers = Registers::new(word);
            registers.sp_is_placeholder = sp_is_placeholder;
            registers.set(REG_SP, caller_cfa);
            for (register, rule) in &caller_row.registers {
                if let RegisterRule::Offset(offset) = rule
                    && let Some(value) = memory.read_u32(caller_cfa.wrapping_add(*offset as u32))
                {
                    registers.set(register.0, value);
                }
            }

            return Some((frames, registers));
        }

        None
    }

    /// Walks the call chain starting at `registers`, reading saved values from
    /// `memory`. Returns the frames found (innermost first, always including
    /// the starting frame) and the reason why the walk stopped.
    pub fn unwind(
        &self,
        registers: Registers,
        memory: &dyn Memory,
        max_frames: usize,
    ) -> (Vec<Frame>, Stop) {
        let mut frames = Vec::new();
        let mut registers = registers;
        let mut is_return_address = registers.pc_is_return_address;

        loop {
            let Some(sp) = registers.get(REG_SP) else {
                return (frames, Stop::UnknownRegister(REG_SP));
            };

            if frames.len() >= max_frames {
                return (frames, Stop::TooManyFrames);
            }

            let frame = Frame {
                pc: registers.pc,
                sp,
                is_return_address,
                recovered: false,
            };
            frames.push(frame);

            let caller = match self.step(&registers, frame.lookup_pc(), memory, frames.len() == 1) {
                Ok(caller) => caller,
                Err(Failure::ReturnAddressLost {
                    cfa,
                    function_start,
                }) => {
                    let Some((recovered, caller)) =
                        self.recover(cfa, function_start, memory, registers.sp_is_placeholder)
                    else {
                        return (frames, Stop::ReturnAddressLost(registers.pc));
                    };

                    if frames.len() + recovered.len() > max_frames {
                        return (frames, Stop::TooManyFrames);
                    }
                    frames.extend(recovered);

                    caller
                }
                Err(Failure::Stop(stop)) => return (frames, stop),
            };

            let caller_sp = caller.get(REG_SP).unwrap_or(sp);

            // The stack grows downwards, so a caller's frame is never below
            // its callee's. Equal stack pointers are only fine for the
            // frameless innermost frame.
            if caller_sp < sp || (caller_sp == sp && caller.pc == registers.pc) {
                return (frames, Stop::Inconsistent);
            }

            registers = caller;
            is_return_address = true;
        }
    }

    /// Computes the registers of the caller of the frame executing at
    /// `lookup_pc`. `innermost` tells whether the frame is the one that was
    /// interrupted, i.e. whether the live values of the caller-saved registers
    /// (in particular `ra`) are known.
    fn step(
        &self,
        registers: &Registers,
        lookup_pc: u32,
        memory: &dyn Memory,
        innermost: bool,
    ) -> Result<Registers, Failure> {
        let sp = registers.get(REG_SP).ok_or(Stop::UnknownRegister(REG_SP))?;

        let Some(row) = self.row(lookup_pc as u64) else {
            // Without unwind info, the best guess for the innermost frame is
            // that it has no stack frame of its own: a leaf function without
            // CFI, hand-written assembly, or a jump through a bad pointer.
            // Its caller is then where `ra` points, with the stack pointer
            // unchanged. For any other frame there is nothing to go on.
            if innermost
                && let Some(ra) = registers.get(REG_RA)
                && ra != 0
            {
                let mut caller = registers.clone();
                caller.pc = ra;
                caller.clear(REG_RA);

                return Ok(caller);
            }

            return Err(Stop::NoUnwindInfo(registers.pc).into());
        };

        let cfa = match row.cfa {
            CfaRule::RegisterAndOffset { register, offset } => {
                // A CFA based on another register (the frame pointer) can
                // only be evaluated if that register holds a known, real
                // address. Otherwise fall back to the function's last stack
                // pointer based rule, which is right unless the function moved
                // `sp` after setting up the frame pointer (dynamic stack
                // allocation).
                let base = if register.0 == REG_SP || !registers.sp_is_placeholder {
                    registers.get(register.0)
                } else {
                    None
                };

                match (base, row.sp_cfa_offset) {
                    (Some(base), _) => base.wrapping_add(offset as u32),
                    (None, Some(sp_offset)) => sp.wrapping_add(sp_offset as u32),
                    (None, None) => return Err(Stop::UnknownRegister(register.0).into()),
                }
            }
            // DWARF expressions and anything else.
            _ => return Err(Stop::Unsupported(registers.pc).into()),
        };

        let mut caller = Registers::new(0);
        caller.sp_is_placeholder = registers.sp_is_placeholder;
        for reg in 0..REG_COUNT as u16 {
            if is_callee_saved(reg)
                && let Some(value) = registers.get(reg)
            {
                caller.set(reg, value);
            }
        }
        caller.set(REG_SP, cfa);

        let mut ra_rule = None;
        for (register, rule) in row.registers {
            if register.0 == REG_RA {
                ra_rule = Some(rule);
                continue;
            }

            let value = match rule {
                RegisterRule::Undefined => None,
                RegisterRule::SameValue => registers.get(register.0),
                RegisterRule::Offset(offset) => memory.read_u32(cfa.wrapping_add(offset as u32)),
                RegisterRule::ValOffset(offset) => Some(cfa.wrapping_add(offset as u32)),
                RegisterRule::Register(other) => registers.get(other.0),
                RegisterRule::Constant(value) => Some(value as u32),
                // DWARF expressions and anything else: unknown.
                _ => None,
            };

            match value {
                Some(value) => caller.set(register.0, value),
                None => caller.clear(register.0),
            }
        }

        // A function that hasn't (yet) saved `ra` has no rule for it. For the
        // innermost frame the live `ra` is then what we need. For any other
        // frame `ra` was clobbered by the call to the callee, so the return
        // address is lost; that happens in functions which never return and
        // therefore don't bother saving it.
        //
        // A function whose CFI marks `ra` as undefined is the outermost one.
        let return_address = match ra_rule {
            None if innermost => registers.get(REG_RA),
            None => {
                return Err(Failure::ReturnAddressLost {
                    cfa,
                    function_start: row.function.start,
                });
            }
            Some(RegisterRule::Undefined) => None,
            Some(RegisterRule::SameValue) => registers.get(REG_RA),
            Some(RegisterRule::Offset(offset)) => {
                let addr = cfa.wrapping_add(offset as u32);

                Some(memory.read_u32(addr).ok_or(Stop::MemoryUnavailable(addr))?)
            }
            Some(RegisterRule::ValOffset(offset)) => Some(cfa.wrapping_add(offset as u32)),
            Some(RegisterRule::Register(other)) => registers.get(other.0),
            Some(RegisterRule::Constant(value)) => Some(value as u32),
            // DWARF expressions and anything else.
            Some(_) => return Err(Stop::Unsupported(registers.pc).into()),
        };

        match return_address {
            None | Some(0) => Err(Stop::EndOfStack.into()),
            Some(pc) => {
                caller.pc = pc;

                Ok(caller)
            }
        }
    }
}

/// A call instruction, with its target unless it goes through a register.
struct Call {
    target: Option<u32>,
}

/// Decodes the immediate of a J-type (`jal`) instruction.
fn j_immediate(insn: u32) -> i32 {
    let imm = ((insn >> 31) & 1) << 20
        | ((insn >> 21) & 0x3ff) << 1
        | ((insn >> 20) & 1) << 11
        | ((insn >> 12) & 0xff) << 12;

    // Sign-extend from 21 bits.
    ((imm << 11) as i32) >> 11
}

/// Decodes the immediate of a CJ-type (`c.jal`) instruction.
fn cj_immediate(insn: u16) -> i32 {
    let insn = insn as u32;
    let imm = ((insn >> 12) & 1) << 11
        | ((insn >> 11) & 1) << 4
        | ((insn >> 9) & 3) << 8
        | ((insn >> 8) & 1) << 10
        | ((insn >> 7) & 1) << 6
        | ((insn >> 6) & 1) << 7
        | ((insn >> 3) & 7) << 1
        | ((insn >> 2) & 1) << 5;

    // Sign-extend from 12 bits.
    ((imm << 20) as i32) >> 20
}

fn section_data<'a>(file: &object::File<'a>, name: &str) -> Option<&'a [u8]> {
    let data = file.section_by_name(name)?.data().ok()?;

    (!data.is_empty()).then_some(data)
}

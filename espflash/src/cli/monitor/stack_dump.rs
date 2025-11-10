use std::{collections::HashMap, io::Write, rc::Rc};

use gimli::{Section, UnwindSection};
use object::{Object, ObjectSection};

use crate::cli::monitor::symbols::Symbols;

pub(crate) const MARKER: &str = "STACKDUMP: ";

pub(crate) fn backtrace_from_stack_dump(
    line: &str,
    out: &mut dyn Write,
    elfs: &Vec<&[u8]>,
    symbols: &Vec<Symbols<'_>>,
) -> std::io::Result<()> {
    if let Some(remaining) = line.to_string().strip_prefix(MARKER) {
        let mut split = remaining.split(" ");
        let (address, stack) = {
            let first = split.next();
            let second = split.next();

            (first, second)
        };

        if let Some(address) = address {
            if let Some(stack) = stack {
                if stack.len() % 2 != 0 {
                    return Ok(());
                }

                let mut pc = u32::from_str_radix(address, 16).unwrap_or_default();
                let mut stack_bytes = Vec::new();
                for byte_chars in stack.chars().collect::<Vec<char>>().chunks(2) {
                    if byte_chars.len() == 2 {
                        stack_bytes.push(
                            u8::from_str_radix(&format!("{}{}", byte_chars[0], byte_chars[1]), 16)
                                .unwrap_or_default(),
                        );
                    }
                }

                let mut func_info = Vec::new();
                for elf in elfs {
                    func_info.append(&mut get_func_info(elf)?);
                }

                writeln!(out).ok();
                let mut index = 0;
                loop {
                    let func = func_info.iter().find(|f| f.start <= pc && f.end >= pc);
                    if let Some(func) = func {
                        if func.stack_frame_size == 0 {
                            break;
                        }

                        let lookup_pc = pc as u64 - 4;

                        for symbols in symbols {
                            let name = symbols.name(lookup_pc);
                            let location = symbols.location(lookup_pc);
                            if let Some(name) = name {
                                if let Some((file, line_num)) = location {
                                    writeln!(out, "{name}\r\n    at {file}:{line_num}\r\n").ok();
                                } else {
                                    writeln!(out, "{name}\r\n    at ??:??\r\n").ok();
                                }
                            }
                        }

                        if index + func.stack_frame_size as usize > stack_bytes.len() {
                            break;
                        }

                        let next_pc_pos = index + (func.stack_frame_size as usize - 4);

                        pc = u32::from_le_bytes(
                            stack_bytes[next_pc_pos..][..4]
                                .try_into()
                                .unwrap_or_default(),
                        );
                        index += func.stack_frame_size as usize;
                    } else {
                        break;
                    }
                }
                writeln!(out).ok();
            }
        }
    }

    Ok(())
}

fn get_func_info(elf: &[u8]) -> Result<Vec<FuncInfo>, std::io::Error> {
    let debug_file = object::File::parse(elf).expect("parse file");

    let endian = if debug_file.is_little_endian() {
        gimli::RunTimeEndian::Little
    } else {
        gimli::RunTimeEndian::Big
    };

    let eh_frame = gimli::EhFrame::load(|sect_id| {
        let data = debug_file
            .section_by_name(sect_id.name())
            .and_then(|section| section.data().ok());

        if let Some(data) = data {
            Ok::<gimli::EndianReader<gimli::RunTimeEndian, Rc<[u8]>>, ()>(
                gimli::EndianRcSlice::new(Rc::from(data), endian),
            )
        } else {
            Err(())
        }
    })
    .map_err(|_| std::io::Error::other("no eh_frame section"))?;

    process_eh_frame(&debug_file, eh_frame).map_err(|_| std::io::Error::other("eh_frame error"))
}

#[derive(Debug)]
struct FuncInfo {
    start: u32,
    end: u32,
    stack_frame_size: u32,
}

fn process_eh_frame<R: gimli::Reader<Offset = usize>>(
    file: &object::File<'_>,
    mut eh_frame: gimli::EhFrame<R>,
) -> Result<Vec<FuncInfo>, gimli::Error> {
    let mut res = Vec::new();

    let address_size = file
        .architecture()
        .address_size()
        .map(|w| w.bytes())
        .unwrap_or(std::mem::size_of::<usize>() as u8);
    eh_frame.set_address_size(address_size);

    let mut bases = gimli::BaseAddresses::default();
    if let Some(section) = file.section_by_name(".eh_frame") {
        bases = bases.set_eh_frame(section.address());
    }

    let mut cies = HashMap::new();

    let mut entries = eh_frame.entries(&bases);
    loop {
        match entries.next()? {
            None => return Ok(res),
            Some(gimli::CieOrFde::Fde(partial)) => {
                let fde = match partial.parse(|_, bases, o| {
                    cies.entry(o)
                        .or_insert_with(|| eh_frame.cie_from_offset(bases, o))
                        .clone()
                }) {
                    Ok(fde) => fde,
                    Err(_) => {
                        // ignored
                        continue;
                    }
                };

                let mut entry = FuncInfo {
                    start: fde.initial_address() as u32,
                    end: fde.end_address() as u32,
                    stack_frame_size: 0u32,
                };

                let instructions = fde.instructions(&eh_frame, &bases);
                let sfs = estimate_stack_frame_size(instructions)?;
                entry.stack_frame_size = sfs;
                res.push(entry);
            }
            _ => (),
        }
    }
}

fn estimate_stack_frame_size<R: gimli::Reader>(
    mut insns: gimli::CallFrameInstructionIter<'_, R>,
) -> Result<u32, gimli::Error> {
    use gimli::CallFrameInstruction::*;

    let mut sfs = 0;

    loop {
        match insns.next() {
            Err(_e) => {
                break;
            }
            Ok(None) => {
                break;
            }
            Ok(Some(op)) => {
                if let DefCfaOffset { offset } = op {
                    sfs = u32::max(sfs, offset as u32);
                }
            }
        }
    }

    Ok(sfs)
}

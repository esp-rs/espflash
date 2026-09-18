use std::error::Error;

use addr2line::{
    Context,
    LookupResult,
    gimli::{self, Dwarf, EndianSlice, LittleEndian, SectionId},
};
use object::{Object, ObjectSection, ObjectSegment, ObjectSymbol, read::File};

/// A function found at some address, see [`Symbols::frames`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SymbolFrame {
    /// The demangled name of the function, if known.
    pub name: Option<String>,
    /// The file name and line number, if known.
    pub location: Option<(String, u32)>,
}

// Wrapper around addr2line that allows to look up function names and
// locations from a given address.
pub(crate) struct Symbols<'sym> {
    object: File<'sym, &'sym [u8]>,
    ctx: Context<EndianSlice<'sym, LittleEndian>>,
}

impl<'sym> Symbols<'sym> {
    /// Tries to create a new `Symbols` instance from the given ELF file bytes.
    pub fn try_from(bytes: &'sym [u8]) -> Result<Self, Box<dyn Error>> {
        let object = File::parse(bytes)?;
        let dwarf = Dwarf::load(
            |id: SectionId| -> Result<EndianSlice<'sym, LittleEndian>, gimli::Error> {
                let data = object
                    .section_by_name(id.name())
                    .and_then(|section| section.data().ok())
                    .unwrap_or(&[][..]);
                Ok(EndianSlice::new(data, LittleEndian))
            },
        )?;

        let ctx = Context::from_dwarf(dwarf)?;

        Ok(Self { object, ctx })
    }

    /// Returns the name of the function at the given address, if one can be
    /// found.
    pub fn name(&self, addr: u64) -> Option<String> {
        // No need to try an address not contained in any segment:
        if !self.contains(addr) {
            return None;
        }

        // The basic steps here are:
        //   1. Find which frame `addr` is in
        //   2. Look up and demangle the function name
        //   3. If no function name is found, try to look it up in the object
        //      file directly
        //   4. Return a demangled function name, if one was found
        let mut frames = match self.ctx.find_frames(addr) {
            LookupResult::Output(result) => result.unwrap(),
            LookupResult::Load { .. } => unimplemented!(),
        };

        frames
            .next()
            .ok()
            .flatten()
            .and_then(|frame| {
                frame
                    .function
                    .and_then(|name| name.demangle().map(|s| s.into_owned()).ok())
            })
            .or_else(|| self.symbol_table_name(addr))
    }

    /// Returns the chain of functions at the given address, innermost first:
    /// the function containing the address, followed by the functions it was
    /// inlined into. Each entry comes with the location of the address in the
    /// innermost function, resp. of the inlined call in the outer ones.
    ///
    /// Empty if nothing is known about the address.
    pub fn frames(&self, addr: u64) -> Vec<SymbolFrame> {
        if !self.contains(addr) {
            return Vec::new();
        }

        let mut result = Vec::new();

        if let Ok(mut frames) = self.ctx.find_frames(addr).skip_all_loads() {
            while let Ok(Some(frame)) = frames.next() {
                result.push(SymbolFrame {
                    name: frame
                        .function
                        .and_then(|name| name.demangle().map(|s| s.into_owned()).ok()),
                    location: frame
                        .location
                        .and_then(|location| Some((location.file?.to_string(), location.line?))),
                });
            }
        }

        if result.is_empty()
            && let Some(name) = self.symbol_table_name(addr)
        {
            result.push(SymbolFrame {
                name: Some(name),
                location: self.location(addr),
            });
        }

        result
    }

    /// Returns the address of the symbol with the given name, if any.
    pub fn symbol_address(&self, name: &str) -> Option<u64> {
        self.object
            .symbol_by_name(name)
            .map(|symbol| symbol.address())
    }

    /// Whether the address is part of one of the object's segments.
    fn contains(&self, addr: u64) -> bool {
        self.object.segments().any(|segment| {
            (segment.address()..(segment.address() + segment.size())).contains(&addr)
        })
    }

    /// Looks up the (demangled) name of the symbol containing the address in
    /// the symbol table.
    fn symbol_table_name(&self, addr: u64) -> Option<String> {
        // Don't use `symbol_map().get(addr)` - it's documentation says
        // "Get the symbol before the given address."
        // which might be totally wrong
        let symbol = self.object.symbols().find(|symbol| {
            (symbol.address()..=(symbol.address() + symbol.size())).contains(&addr)
        })?;

        match symbol.name() {
            Ok(name) if !name.is_empty() => {
                Some(addr2line::demangle_auto(std::borrow::Cow::Borrowed(name), None).to_string())
            }
            _ => None,
        }
    }

    /// Returns the file name and line number of the function at the given
    /// address, if one can be.
    pub fn location(&self, addr: u64) -> Option<(String, u32)> {
        // Find the location which `addr` is in. If we can dedetermine a file
        // name and line number for this function we will return them
        // both in a tuple.
        self.ctx.find_location(addr).ok()?.map(|location| {
            let file = location.file.map(|f| f.to_string());
            let line = location.line;

            match (file, line) {
                (Some(file), Some(line)) => Some((file, line)),
                _ => None,
            }
        })?
    }
}

use std::error::Error;

use addr2line::{
    gimli::{EndianRcSlice, RunTimeEndian},
    object::{read::File, Object},
    Context,
};

// Wrapper around addr2line that allows to look up function names and
// locations from a given address.
pub(crate) struct Symbols<'sym> {
    file: File<'sym, &'sym [u8]>,
    ctx: Context<EndianRcSlice<RunTimeEndian>>,
}

impl<'sym> Symbols<'sym> {
    pub fn try_from(bytes: &'sym [u8]) -> Result<Self, Box<dyn Error>> {
        let file = File::parse(bytes)?;
        let ctx = Context::new(&file)?;

        Ok(Self { file, ctx })
    }
    /// Returns the name of the function at the given address, if one can be found.
    pub fn get_name(&self, addr: u64) -> Option<String> {
        // The basic steps here are:
        //   1. find which frame `addr` is in
        //   2. look up and demangle the function name
        //   3. if no function name is found, try to look it up in the object file
        //      directly
        //   4. return a demangled function name, if one was found
        self.ctx
            .find_frames(addr)
            .ok()
            .and_then(|mut frames| {
                frames.next().ok().flatten().and_then(|frame| {
                    frame
                        .function
                        .and_then(|name| name.demangle().map(|s| s.into_owned()).ok())
                })
            })
            .or_else(|| {
                self.file
                    .symbol_map()
                    .get(addr)
                    .map(|sym| sym.name().to_string())
            })
    }

    /// Returns the file name and line number of the function at the given address, if one can be.
    pub fn get_location(&self, addr: u64) -> Option<(String, u32)> {
        // Find the location which `addr` is in. If we can dedetermine a file name and
        // line number for this function we will return them both in a tuple.
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

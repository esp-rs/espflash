use std::io::Write;

use crate::cli::monitor::parser::InputParser;

#[derive(Debug)]
pub struct Serial;

impl InputParser for Serial {
    fn feed(&mut self, bytes: &[u8], out: &mut dyn Write) {
        out.write_all(bytes).unwrap();
    }
}

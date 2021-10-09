use std::io::Write;

const END: u8 = 0xC0;
const ESC: u8 = 0xDB;
const ESC_END: u8 = 0xDC;
const ESC_ESC: u8 = 0xDD;

pub struct SlipEncoder<'a, W: Write> {
    writer: &'a mut W,
    len: usize,
}

impl<'a, W: Write> SlipEncoder<'a, W> {
    /// Creates a new encoder context
    pub fn new(writer: &'a mut W) -> std::io::Result<Self> {
        let len = writer.write(&[END])?;
        Ok(Self { writer, len })
    }

    pub fn finish(mut self) -> std::io::Result<usize> {
        self.len += self.writer.write(&[END])?;
        Ok(self.len)
    }
}

impl<'a, W: Write> Write for SlipEncoder<'a, W> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        for value in buf.iter() {
            match *value {
                END => {
                    self.len += self.writer.write(&[ESC, ESC_END])?;
                }
                ESC => {
                    self.len += self.writer.write(&[ESC, ESC_ESC])?;
                }
                _ => {
                    self.len += self.writer.write(&[*value])?;
                }
            }
        }

        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.writer.flush()
    }
}

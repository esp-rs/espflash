pub mod encoder {
    use core::fmt::Debug;

    use embedded_io::Write;

    const END: u8 = 0xC0;
    const ESC: u8 = 0xDB;
    const ESC_END: u8 = 0xDC;
    const ESC_ESC: u8 = 0xDD;

    #[derive(Debug)]
    pub struct SlipEncoder<'a, W: Write> {
        writer: &'a mut W,
        len: usize,
    }

    impl<'a, W: Write> SlipEncoder<'a, W> {
        /// Creates a new encoder context
        pub fn new(writer: &'a mut W) -> Result<Self, W::Error> {
            let len = writer.write(&[END])?;
            Ok(Self { writer, len })
        }

        pub fn finish(mut self) -> Result<usize, W::Error> {
            self.len += self.writer.write(&[END])?;
            Ok(self.len)
        }
    }

    impl<W: Write> embedded_io::ErrorType for SlipEncoder<'_, W> {
        type Error = W::Error;
    }

    impl<W: Write> Write for SlipEncoder<'_, W> {
        /// Writes the given buffer replacing the END and ESC bytes
        ///
        /// See https://docs.espressif.com/projects/esptool/en/latest/esp32c3/advanced-topics/serial-protocol.html#low-level-protocol
        fn write(&mut self, buf: &[u8]) -> Result<usize, W::Error> {
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

        fn flush(&mut self) -> Result<(), W::Error> {
            self.writer.flush()
        }
    }
}

pub mod decoder {
    /// SLIP end of packet token
    const END: u8 = 0xC0;

    /// SLIP escape token
    const ESC: u8 = 0xDB;

    /// SLIP escaped 0xC0 token
    const ESC_END: u8 = 0xDC;

    /// SLIP escaped 0xDB token
    const ESC_ESC: u8 = 0xDD;

    /// Recommended maximum SLIP packet size per RFC 1055
    #[allow(dead_code)]
    const MAX_PACKET_SIZE: usize = 1006;
    use core::convert::Infallible;

    use embedded_io::{Read, Write};

    /// SLIP decoder error type
    #[derive(Debug)]
    pub enum SlipError<RE, WE> {
        FramingError,
        OversizedPacket,
        EndOfStream,
        ReadError(RE),
        WriteError(WE),
    }

    pub type SlipResult<RE, WE> = core::result::Result<usize, self::SlipError<RE, WE>>;

    #[derive(Debug)]
    enum State {
        Normal,
        Error,
        Escape,
    }

    /// SLIP decoder context
    #[derive(Debug)]
    pub struct SlipDecoder {
        count: usize,
        state: State,
    }

    // Unfortunately even the never type doesn't auto-coerce
    fn coerce_infallible<WE, RE>(err: SlipError<Infallible, WE>) -> SlipError<RE, WE> {
        match err {
            SlipError::ReadError(_) => {
                unreachable!()
            }
            SlipError::WriteError(e) => SlipError::WriteError(e),
            SlipError::FramingError => SlipError::FramingError,
            SlipError::OversizedPacket => SlipError::OversizedPacket,
            SlipError::EndOfStream => SlipError::EndOfStream,
        }
    }

    impl SlipDecoder {
        /// Creates a new context with the given maximum buffer size.
        pub fn new() -> Self {
            Self {
                count: 0usize,
                state: State::Normal,
            }
        }

        fn push<W: Write>(
            &mut self,
            sink: &mut W,
            value: u8,
        ) -> self::SlipResult<Infallible, W::Error> {
            match sink.write(&[value]) {
                Ok(len) => {
                    if len != 1 {
                        Err(SlipError::OversizedPacket)
                    } else {
                        self.count += 1;
                        Ok(1usize)
                    }
                }
                Err(error) => Err(SlipError::WriteError(error)),
            }
        }

        /// Attempts to decode a single SLIP frame from the given source.
        ///
        /// # Arguments
        ///
        /// * `source` - Encoded SLIP data source implementing the std::io::Read
        ///   trait
        ///
        /// Returns a Vec<u8> containing a decoded message or an empty Vec<u8>
        /// if of the source data was reached.
        pub fn decode<W: Write, R: Read>(
            &mut self,
            source: &mut R,
            sink: &mut W,
        ) -> self::SlipResult<R::Error, W::Error> {
            loop {
                let mut buf = [0u8; 16];
                let read_amount = source.read(&mut buf).map_err(SlipError::ReadError)?;

                for value in buf[0..read_amount].iter().cloned() {
                    match self.state {
                        State::Normal => match value {
                            END => {
                                if self.count > 0 {
                                    let len = self.count;

                                    self.count = 0usize;

                                    return Ok(len);
                                }
                            }
                            ESC => {
                                self.state = State::Escape;
                            }
                            _ => {
                                self.push(sink, value).map_err(coerce_infallible)?;
                            }
                        },
                        State::Error => {
                            if value == END {
                                self.count = 0usize;
                                self.state = State::Normal;
                            }
                        }
                        State::Escape => match value {
                            ESC_END => {
                                self.push(sink, END).map_err(coerce_infallible)?;
                                self.state = State::Normal;
                            }
                            ESC_ESC => {
                                self.push(sink, ESC).map_err(coerce_infallible)?;
                                self.state = State::Normal;
                            }
                            _ => {
                                self.state = State::Error;

                                return Err(SlipError::FramingError);
                            }
                        },
                    }
                }

                if read_amount < buf.len() {
                    Err(SlipError::EndOfStream)?
                }
            }
        }
    }

    impl Default for SlipDecoder {
        fn default() -> Self {
            Self::new()
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn empty_decode() {
            const INPUT: [u8; 2] = [0xc0, 0xc0];

            let mut slip = SlipDecoder::new();
            let mut buf: Vec<u8> = Vec::new();
            let res = slip.decode(&mut INPUT.as_ref(), &mut buf);
            assert!(res.is_err());
            assert!(buf.is_empty());
        }

        #[test]
        fn simple_decode() {
            const INPUT: [u8; 7] = [0xc0, 0x01, 0x02, 0x03, 0x04, 0x05, 0xc0];
            const DATA: [u8; 5] = [0x01, 0x02, 0x03, 0x04, 0x05];

            let mut slip = SlipDecoder::new();
            let mut buf = [0u8; DATA.len()];
            let len = slip.decode(&mut INPUT.as_ref(), &mut buf.as_mut()).unwrap();
            assert_eq!(DATA.len(), len);
            assert_eq!(DATA.len(), buf.len());
            assert_eq!(&DATA, &buf);
        }

        /// Ensure that [ESC, ESC_END] -> [END]
        #[test]
        fn decode_esc_then_esc_end_sequence() {
            const INPUT: [u8; 6] = [0xc0, 0x01, 0xdb, 0xdc, 0x03, 0xc0];
            const DATA: [u8; 3] = [0x01, 0xc0, 0x03];

            let mut slip = SlipDecoder::new();
            let mut buf: Vec<u8> = Vec::new();
            let len = slip.decode(&mut INPUT.as_ref(), &mut buf).unwrap();
            assert_eq!(DATA.len(), len);
            assert_eq!(DATA.len(), buf.len());
            assert_eq!(&DATA, buf.as_slice());
        }

        /// Ensure that [ESC, ESC_ESC] -> [ESC]
        #[test]
        fn decode_esc_then_esc_esc_sequence() {
            const INPUT: [u8; 6] = [0xc0, 0x01, 0xdb, 0xdd, 0x03, 0xc0];
            const DATA: [u8; 3] = [0x01, 0xdb, 0x03];

            let mut slip = SlipDecoder::new();
            let mut buf: Vec<u8> = Vec::new();
            let len = slip.decode(&mut INPUT.as_ref(), &mut buf).unwrap();
            assert_eq!(DATA.len(), len);
            assert_eq!(DATA.len(), buf.len());
            assert_eq!(&DATA, buf.as_slice());
        }

        #[test]
        fn multi_part_decode() {
            const INPUT_1: [u8; 6] = [0xc0, 0x01, 0x02, 0x03, 0x04, 0x05];
            const INPUT_2: [u8; 6] = [0x05, 0x06, 0x07, 0x08, 0x09, 0xc0];
            const DATA: [u8; 10] = [0x01, 0x02, 0x03, 0x04, 0x05, 0x05, 0x06, 0x07, 0x08, 0x09];

            let mut slip = SlipDecoder::new();
            let mut buf: Vec<u8> = Vec::new();

            {
                let res = slip.decode(&mut INPUT_1.as_ref(), &mut buf);
                assert!(res.is_err());
                assert_eq!(5, buf.len());
            }

            {
                let len = slip.decode(&mut INPUT_2.as_ref(), &mut buf).unwrap();
                assert_eq!(DATA.len(), len);
                assert_eq!(DATA.len(), buf.len());
                assert_eq!(&DATA, buf.as_slice());
            }
        }

        #[test]
        fn compound_decode() {
            const INPUT: [u8; 13] = [
                0xc0, 0x01, 0x02, 0x03, 0x04, 0x05, 0xc0, 0x05, 0x06, 0x07, 0x08, 0x09, 0xc0,
            ];
            const DATA_1: [u8; 5] = [0x01, 0x02, 0x03, 0x04, 0x05];
            const DATA_2: [u8; 5] = [0x05, 0x06, 0x07, 0x08, 0x09];

            let mut slip = SlipDecoder::new();
            let reader = &mut INPUT.as_ref();

            {
                let mut buf: Vec<u8> = Vec::new();
                let len = slip.decode(reader, &mut buf).unwrap();
                assert_eq!(DATA_1.len(), len);
                assert_eq!(DATA_1.len(), buf.len());
                assert_eq!(&DATA_1, buf.as_slice());
            }

            {
                let mut buf: Vec<u8> = Vec::new();
                let len = slip.decode(reader, &mut buf).unwrap();
                assert_eq!(DATA_2.len(), len);
                assert_eq!(DATA_2.len(), buf.len());
                assert_eq!(&DATA_2, buf.as_slice());
            }
        }
    }
}

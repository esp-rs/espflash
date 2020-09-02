use crate::encoder::SlipEncoder;
use crate::error::RomError;
use crate::Error;
use binread::io::Cursor;
use binread::{BinRead, BinReaderExt};
use serial::SerialPort;
use slip_codec::Decoder;
use std::io::Write;
use std::mem::size_of;
use std::thread::sleep;
use std::time::Duration;

pub struct Connection {
    serial: Box<dyn SerialPort>,
    decoder: Decoder,
}

#[derive(Debug, Copy, Clone, BinRead)]
pub struct CommandResponse<Data: BinRead<Args = ()>> {
    pub resp: u8,
    pub return_op: u8,
    pub return_length: u16,
    pub value: u32,
    pub status: u8,
    pub error: u8,
    pub data: Data,
}

impl Connection {
    pub fn new(serial: impl SerialPort + 'static) -> Self {
        Connection {
            serial: Box::new(serial),
            decoder: Decoder::new(1024),
        }
    }

    pub fn reset_to_flash(&mut self) -> Result<(), Error> {
        self.serial.set_dtr(false)?;
        self.serial.set_rts(true)?;

        sleep(Duration::from_millis(100));

        self.serial.set_dtr(true)?;
        self.serial.set_rts(false)?;

        sleep(Duration::from_millis(50));

        self.serial.set_dtr(true)?;

        Ok(())
    }

    pub fn read_response<Return: BinRead<Args = ()>>(
        &mut self,
        timeout: u64,
    ) -> Result<Option<CommandResponse<Return>>, Error> {
        let response = self.read(timeout)?;
        if response.len() < 10 + size_of::<Return>() {
            dbg!(response);
            return Ok(None);
        }

        let mut cursor = Cursor::new(response);
        let header = cursor.read_le()?;

        Ok(Some(header))
    }

    pub fn write_command(
        &mut self,
        command: u8,
        data: impl LazyBytes<Box<dyn SerialPort>>,
        check: u32,
    ) -> Result<(), Error> {
        let mut encoder = SlipEncoder::new(&mut self.serial)?;
        encoder.write(&[0])?;
        encoder.write(&[command])?;
        encoder.write(&(data.length().to_le_bytes()))?;
        encoder.write(&(check.to_le_bytes()))?;
        data.write(&mut encoder)?;
        encoder.finish()?;
        Ok(())
    }

    pub fn command<'a, Return: BinRead<Args = ()>, Data: LazyBytes<Box<dyn SerialPort>>>(
        &mut self,
        command: u8,
        data: Data,
        check: u32,
        timeout: u64,
    ) -> Result<(Return, u32), Error> {
        self.write_command(command, data, check)?;

        match self.read_response(timeout)? {
            Some(response) if response.return_op == command as u8 => {
                if response.status == 1 {
                    Err(Error::RomError(RomError::from(response.error)))
                } else {
                    Ok((response.data, response.value))
                }
            }
            _ => Err(Error::ConnectionFailed),
        }
    }

    fn read(&mut self, timeout: u64) -> Result<Vec<u8>, Error> {
        self.serial
            .set_timeout(Duration::from_millis(timeout))
            .unwrap();
        Ok(self.decoder.decode(&mut self.serial)?)
    }

    pub fn flush(&mut self) -> Result<(), Error> {
        self.serial.flush()?;
        Ok(())
    }
}

pub trait LazyBytes<W: Write> {
    fn write(self, encoder: &mut SlipEncoder<W>) -> Result<(), Error>;

    fn length(&self) -> u16;
}

impl<W: Write> LazyBytes<W> for &[u8] {
    fn write(self, encoder: &mut SlipEncoder<W>) -> Result<(), Error> {
        encoder.write(self)?;
        Ok(())
    }

    fn length(&self) -> u16 {
        self.len() as u16
    }
}

impl<W: Write, F: Fn(&mut SlipEncoder<W>) -> Result<(), Error>> LazyBytes<W> for (u16, F) {
    fn write(self, encoder: &mut SlipEncoder<W>) -> Result<(), Error> {
        self.1(encoder)
    }

    fn length(&self) -> u16 {
        self.0
    }
}

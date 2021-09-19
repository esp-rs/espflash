use std::io::Write;
use std::thread::sleep;
use std::time::Duration;

use crate::encoder::SlipEncoder;
use crate::error::{ConnectionError, Error, RomError};
use binread::io::Cursor;
use binread::{BinRead, BinReaderExt};
use serial::{BaudRate, SerialPort, SerialPortSettings, SystemPort};
use slip_codec::Decoder;

pub struct Connection {
    serial: SystemPort,
    decoder: Decoder,
}

#[derive(Debug, Copy, Clone, BinRead)]
pub struct CommandResponse {
    pub resp: u8,
    pub return_op: u8,
    pub return_length: u16,
    pub value: u32,
    pub status: u8,
    pub error: u8,
}

impl Connection {
    pub fn new(serial: SystemPort) -> Self {
        Connection {
            serial,
            decoder: Decoder::new(),
        }
    }

    pub fn reset(&mut self) -> Result<(), Error> {
        sleep(Duration::from_millis(100));

        self.serial.set_dtr(false)?;
        self.serial.set_rts(true)?;

        sleep(Duration::from_millis(100));

        self.serial.set_rts(false)?;

        Ok(())
    }

    pub fn reset_to_flash(&mut self) -> Result<(), Error> {
        self.serial.set_dtr(false)?;
        self.serial.set_rts(true)?;

        sleep(Duration::from_millis(100));

        self.serial.set_dtr(true)?;
        self.serial.set_rts(false)?;

        sleep(Duration::from_millis(50));

        self.serial.set_dtr(false)?;

        Ok(())
    }

    pub fn set_timeout(&mut self, timeout: Duration) -> Result<(), Error> {
        self.serial.set_timeout(timeout)?;
        Ok(())
    }

    pub fn set_baud(&mut self, speed: BaudRate) -> Result<(), Error> {
        self.serial
            .reconfigure(&|setup: &mut dyn SerialPortSettings| setup.set_baud_rate(speed))?;
        Ok(())
    }

    pub fn with_timeout<T, F: FnMut(&mut Connection) -> Result<T, Error>>(
        &mut self,
        timeout: Duration,
        mut f: F,
    ) -> Result<T, Error> {
        let old_timeout = self.serial.timeout();
        self.serial.set_timeout(timeout)?;
        let result = f(self);
        self.serial.set_timeout(old_timeout)?;
        result
    }

    pub fn read_response(&mut self) -> Result<Option<CommandResponse>, Error> {
        let response = self.read()?;
        if response.len() < 10 {
            return Ok(None);
        }

        let mut cursor = Cursor::new(response);
        let header = cursor.read_le()?;

        Ok(Some(header))
    }

    pub fn write_command(
        &mut self,
        command: u8,
        data: impl LazyBytes<SystemPort>,
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

    pub fn command<Data: LazyBytes<SystemPort>>(
        &mut self,
        command: u8,
        data: Data,
        check: u32,
    ) -> Result<u32, Error> {
        self.write_command(command, data, check)?;

        for _ in 0..100 {
            match self.read_response()? {
                Some(response) if response.return_op == command as u8 => {
                    if response.status == 1 {
                        let _error = self.flush();
                        return Err(Error::RomError(RomError::from(response.error)));
                    } else {
                        return Ok(response.value);
                    }
                }
                _ => {
                    continue;
                }
            }
        }
        Err(Error::Connection(ConnectionError::ConnectionFailed))
    }

    fn read(&mut self) -> Result<Vec<u8>, Error> {
        let mut output = Vec::with_capacity(1024);
        self.decoder.decode(&mut self.serial, &mut output)?;
        Ok(output)
    }

    pub fn flush(&mut self) -> Result<(), Error> {
        self.serial.flush()?;
        Ok(())
    }

    pub fn into_serial(self) -> SystemPort {
        self.serial
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

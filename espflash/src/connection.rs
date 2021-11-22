use std::{io::Write, thread::sleep, time::Duration};

use binread::{io::Cursor, BinRead, BinReaderExt};
use bytemuck::{Pod, Zeroable};
use serialport::SerialPort;
use slip_codec::SlipDecoder;

use crate::{
    command::{Command, CommandType},
    encoder::SlipEncoder,
    error::{ConnectionError, Error, ResultExt, RomError, RomErrorKind},
};

#[derive(Debug, Copy, Clone, BinRead)]
pub struct CommandResponse {
    pub resp: u8,
    pub return_op: u8,
    pub return_length: u16,
    pub value: u32,
    pub status: u8,
    pub error: u8,
}

pub struct Connection {
    serial: Box<dyn SerialPort>,
    decoder: SlipDecoder,
}

#[derive(Zeroable, Pod, Copy, Clone, Debug)]
#[repr(C)]
struct WriteRegParams {
    addr: u32,
    value: u32,
    mask: u32,
    delay_us: u32,
}

impl Connection {
    pub fn new(serial: Box<dyn SerialPort>) -> Self {
        Connection {
            serial,
            decoder: SlipDecoder::new(),
        }
    }

    pub fn reset(&mut self) -> Result<(), Error> {
        sleep(Duration::from_millis(100));

        self.serial.write_data_terminal_ready(false)?;
        self.serial.write_request_to_send(true)?;

        sleep(Duration::from_millis(100));

        self.serial.write_request_to_send(false)?;

        Ok(())
    }

    pub fn reset_to_flash(&mut self, extra_delay: bool) -> Result<(), Error> {
        self.serial.write_data_terminal_ready(false)?;
        self.serial.write_request_to_send(true)?;

        sleep(Duration::from_millis(100));

        self.serial.write_data_terminal_ready(true)?;
        self.serial.write_request_to_send(false)?;

        let millis = if extra_delay { 500 } else { 50 };
        sleep(Duration::from_millis(millis));

        self.serial.write_data_terminal_ready(false)?;

        Ok(())
    }

    pub fn set_timeout(&mut self, timeout: Duration) -> Result<(), Error> {
        self.serial.set_timeout(timeout)?;
        Ok(())
    }

    pub fn set_baud(&mut self, speed: u32) -> Result<(), Error> {
        self.serial.set_baud_rate(speed)?;

        Ok(())
    }

    pub fn get_baud(&self) -> Result<u32, Error> {
        Ok(self.serial.baud_rate()?)
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

    pub fn write_command(&mut self, command: Command) -> Result<(), Error> {
        let mut encoder = SlipEncoder::new(&mut self.serial)?;
        command.write(&mut encoder)?;
        encoder.finish()?;
        Ok(())
    }

    pub fn command(&mut self, command: Command) -> Result<u32, Error> {
        let ty = command.command_type();
        self.write_command(command).for_command(ty)?;

        for _ in 0..100 {
            match self.read_response().for_command(ty)? {
                Some(response) if response.return_op == ty as u8 => {
                    return if response.status == 1 {
                        let _error = self.flush();
                        Err(Error::RomError(RomError::new(
                            command.command_type(),
                            RomErrorKind::from(response.error),
                        )))
                    } else {
                        Ok(response.value)
                    }
                }
                _ => {
                    continue;
                }
            }
        }
        Err(Error::Connection(ConnectionError::ConnectionFailed))
    }

    pub fn read_reg(&mut self, reg: u32) -> Result<u32, Error> {
        self.with_timeout(CommandType::ReadReg.timeout(), |connection| {
            connection.command(Command::ReadReg { address: reg })
        })
    }

    pub fn write_reg(&mut self, addr: u32, value: u32, mask: Option<u32>) -> Result<(), Error> {
        self.with_timeout(CommandType::WriteReg.timeout(), |connection| {
            connection.command(Command::WriteReg {
                address: addr,
                value,
                mask,
            })
        })?;

        Ok(())
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

    pub fn into_serial(self) -> Box<dyn SerialPort> {
        self.serial
    }
}

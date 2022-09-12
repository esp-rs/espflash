use std::{
    io::{BufWriter, Write},
    thread::sleep,
    time::Duration,
};

use binread::{io::Cursor, BinRead, BinReaderExt};
use bytemuck::{Pod, Zeroable};
use serialport::{SerialPort, UsbPortInfo};
use slip_codec::SlipDecoder;

use crate::{
    command::{Command, CommandType},
    encoder::SlipEncoder,
    error::{ConnectionError, Error, ResultExt, RomError, RomErrorKind},
};

const DEFAULT_CONNECT_ATTEMPTS: usize = 7;
pub const USB_SERIAL_JTAG_PID: u16 = 0x1001;

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
    port_info: UsbPortInfo,
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
    pub fn new(serial: Box<dyn SerialPort>, port_info: UsbPortInfo) -> Self {
        Connection {
            serial,
            port_info,
            decoder: SlipDecoder::new(),
        }
    }

    pub fn begin(&mut self) -> Result<(), Error> {
        let mut extra_delay = false;
        for i in 0..DEFAULT_CONNECT_ATTEMPTS {
            if self.connect_attempt(extra_delay).is_err() {
                extra_delay = !extra_delay;

                let delay_text = if extra_delay { "extra" } else { "default" };
                println!("Unable to connect, retrying with {} delay...", delay_text);
            } else {
                // Print a blank line if more than one connection attempt was made to visually
                // separate the status text and whatever comes next.
                if i > 0 {
                    println!();
                }
                return Ok(());
            }
        }

        Err(Error::Connection(ConnectionError::ConnectionFailed))
    }

    fn connect_attempt(&mut self, extra_delay: bool) -> Result<(), Error> {
        self.reset_to_flash(extra_delay)?;

        for _ in 0..5 {
            self.flush()?;
            if self.sync().is_ok() {
                return Ok(());
            }
        }

        Err(Error::Connection(ConnectionError::ConnectionFailed))
    }

    pub(crate) fn sync(&mut self) -> Result<(), Error> {
        self.with_timeout(CommandType::Sync.timeout(), |connection| {
            connection.command(Command::Sync)?;
            connection.flush()?;
            sleep(Duration::from_millis(10));
            for _ in 0..7 {
                match connection.read_response()? {
                    Some(response) if response.return_op == CommandType::Sync as u8 => {
                        if response.status == 1 {
                            let _error = connection.flush();
                            return Err(Error::RomError(RomError::new(
                                CommandType::Sync,
                                RomErrorKind::from(response.error),
                            )));
                        }
                    }
                    _ => {
                        return Err(Error::RomError(RomError::new(
                            CommandType::Sync,
                            RomErrorKind::InvalidMessage,
                        )))
                    }
                }
            }

            Ok(())
        })?;

        Ok(())
    }

    pub fn reset(&mut self) -> Result<(), Error> {
        let pid = self.port_info.pid;
        Ok(reset_after_flash(&mut *self.serial, pid)?)
    }

    pub fn reset_to_flash(&mut self, extra_delay: bool) -> Result<(), Error> {
        if self.port_info.pid == USB_SERIAL_JTAG_PID {
            self.serial.write_data_terminal_ready(false)?;
            self.serial.write_request_to_send(false)?;

            sleep(Duration::from_millis(100));

            self.serial.write_data_terminal_ready(true)?;
            self.serial.write_request_to_send(false)?;

            sleep(Duration::from_millis(100));

            self.serial.write_request_to_send(true)?;
            self.serial.write_data_terminal_ready(false)?;
            self.serial.write_request_to_send(true)?;

            sleep(Duration::from_millis(100));

            self.serial.write_data_terminal_ready(false)?;
            self.serial.write_request_to_send(false)?;
        } else {
            self.serial.write_data_terminal_ready(false)?;
            self.serial.write_request_to_send(true)?;

            sleep(Duration::from_millis(100));

            self.serial.write_data_terminal_ready(true)?;
            self.serial.write_request_to_send(false)?;

            let millis = if extra_delay { 500 } else { 50 };
            sleep(Duration::from_millis(millis));

            self.serial.write_data_terminal_ready(false)?;
        }

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
        match self.read(10)? {
            None => Ok(None),
            Some(response) => {
                let mut cursor = Cursor::new(response);
                let header = cursor.read_le()?;
                Ok(Some(header))
            }
        }
    }

    pub fn write_command(&mut self, command: Command) -> Result<(), Error> {
        self.serial.clear(serialport::ClearBuffer::Input)?;
        let mut writer = BufWriter::new(&mut self.serial);
        let mut encoder = SlipEncoder::new(&mut writer)?;
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

    pub(crate) fn read(&mut self, len: usize) -> Result<Option<Vec<u8>>, Error> {
        let mut tmp = Vec::with_capacity(1024);
        loop {
            self.decoder.decode(&mut self.serial, &mut tmp)?;
            if tmp.len() >= len {
                return Ok(Some(tmp));
            }
        }
    }

    pub fn flush(&mut self) -> Result<(), Error> {
        self.serial.flush()?;
        Ok(())
    }

    pub fn into_serial(self) -> Box<dyn SerialPort> {
        self.serial
    }

    pub fn get_usb_pid(&self) -> Result<u16, Error> {
        Ok(self.port_info.pid)
    }
}

pub fn reset_after_flash(serial: &mut dyn SerialPort, pid: u16) -> Result<(), serialport::Error> {
    sleep(Duration::from_millis(100));

    if pid == USB_SERIAL_JTAG_PID {
        serial.write_data_terminal_ready(false)?;

        sleep(Duration::from_millis(100));

        serial.write_request_to_send(true)?;
        serial.write_data_terminal_ready(false)?;
        serial.write_request_to_send(true)?;

        sleep(Duration::from_millis(100));

        serial.write_request_to_send(false)?;
    } else {
        serial.write_request_to_send(true)?;

        sleep(Duration::from_millis(100));

        serial.write_request_to_send(false)?;
    }

    Ok(())
}

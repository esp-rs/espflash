//! Establish a connection with a target device
//!
//! The [Connection] struct abstracts over the serial connection and
//! sending/decoding of commands, and provides higher-level operations with the
//! device.

use std::{io::BufWriter, thread::sleep, time::Duration};

use binrw::{io::Cursor, BinRead, BinReaderExt};
use log::info;
use serialport::UsbPortInfo;
use slip_codec::SlipDecoder;

use self::encoder::SlipEncoder;
use crate::{
    command::{Command, CommandType},
    error::{ConnectionError, Error, ResultExt, RomError, RomErrorKind},
    interface::Interface,
};

const DEFAULT_CONNECT_ATTEMPTS: usize = 7;
pub(crate) const USB_SERIAL_JTAG_PID: u16 = 0x1001;

/// A response from a target device following a command
#[derive(Debug, Copy, Clone, BinRead)]
pub struct CommandResponse {
    pub resp: u8,
    pub return_op: u8,
    pub return_length: u16,
    pub value: u32,
    pub error: u8,
    pub status: u8,
}

/// An established connection with a target device
pub struct Connection {
    serial: Interface,
    port_info: UsbPortInfo,
    decoder: SlipDecoder,
}

impl Connection {
    pub fn new(serial: Interface, port_info: UsbPortInfo) -> Self {
        Connection {
            serial,
            port_info,
            decoder: SlipDecoder::new(),
        }
    }

    /// Initialize a connection with a device
    pub fn begin(&mut self) -> Result<(), Error> {
        let mut extra_delay = false;
        for _ in 0..DEFAULT_CONNECT_ATTEMPTS {
            if self.connect_attempt(extra_delay).is_err() {
                extra_delay = !extra_delay;

                info!(
                    "Unable to connect, retrying with {} delay...",
                    if extra_delay { "extra" } else { "default" }
                );
            } else {
                return Ok(());
            }
        }

        Err(Error::Connection(ConnectionError::ConnectionFailed))
    }

    /// Try to connect to a device
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

    /// Try to sync with the device for a given timeout
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

    // Reset the device
    pub fn reset(&mut self) -> Result<(), Error> {
        let pid = self.port_info.pid;
        Ok(reset_after_flash(&mut self.serial, pid)?)
    }

    // Reset the device to flash mode
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

    /// Set timeout for the serial port
    pub fn set_timeout(&mut self, timeout: Duration) -> Result<(), Error> {
        self.serial.serial_port_mut().set_timeout(timeout)?;
        Ok(())
    }

    /// Set baud rate for the serial port
    pub fn set_baud(&mut self, speed: u32) -> Result<(), Error> {
        self.serial.serial_port_mut().set_baud_rate(speed)?;

        Ok(())
    }

    /// Get the current baud rate of the serial port
    pub fn get_baud(&self) -> Result<u32, Error> {
        Ok(self.serial.serial_port().baud_rate()?)
    }

    /// Run a command with a timeout defined by the command type
    pub fn with_timeout<T, F: FnMut(&mut Connection) -> Result<T, Error>>(
        &mut self,
        timeout: Duration,
        mut f: F,
    ) -> Result<T, Error> {
        let old_timeout = {
            let serial = self.serial.serial_port_mut();
            let old_timeout = serial.timeout();
            serial.set_timeout(timeout)?;
            old_timeout
        };

        let result = f(self);

        self.serial.serial_port_mut().set_timeout(old_timeout)?;

        result
    }

    /// Read the response from a serial port
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

    /// Write a command to the serial port
    pub fn write_command(&mut self, command: Command) -> Result<(), Error> {
        let serial = self.serial.serial_port_mut();

        serial.clear(serialport::ClearBuffer::Input)?;
        let mut writer = BufWriter::new(serial);
        let mut encoder = SlipEncoder::new(&mut writer)?;
        command.write(&mut encoder)?;
        encoder.finish()?;
        Ok(())
    }

    ///  Write a command and reads the response
    pub fn command(&mut self, command: Command) -> Result<u32, Error> {
        let ty = command.command_type();
        self.write_command(command).for_command(ty)?;

        for _ in 0..100 {
            match self.read_response().for_command(ty)? {
                Some(response) if response.return_op == ty as u8 => {
                    return if response.error != 0 {
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

    /// Read a register command with a timeout
    pub fn read_reg(&mut self, reg: u32) -> Result<u32, Error> {
        self.with_timeout(CommandType::ReadReg.timeout(), |connection| {
            connection.command(Command::ReadReg { address: reg })
        })
    }

    /// Write a register command with a timeout
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

    /// Flush the serial port
    pub fn flush(&mut self) -> Result<(), Error> {
        self.serial.serial_port_mut().flush()?;
        Ok(())
    }

    /// Turn a serial port into a Interface
    pub fn into_interface(self) -> Interface {
        self.serial
    }

    /// Get the USB PID of the serial port
    pub fn get_usb_pid(&self) -> Result<u16, Error> {
        Ok(self.port_info.pid)
    }
}

/// Reset the target device when flashing has completed
pub fn reset_after_flash(serial: &mut Interface, pid: u16) -> Result<(), serialport::Error> {
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

mod encoder {
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
}

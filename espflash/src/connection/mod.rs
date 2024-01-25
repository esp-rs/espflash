//! Establish a connection with a target device
//!
//! The [Connection] struct abstracts over the serial connection and
//! sending/decoding of commands, and provides higher-level operations with the
//! device.

use std::{
    io::{BufWriter, Write},
    iter::zip,
    thread::sleep,
    time::Duration,
};

use log::debug;
use regex::Regex;
use serialport::UsbPortInfo;
use slip_codec::SlipDecoder;

#[cfg(unix)]
use self::reset::UnixTightReset;
use self::{
    encoder::SlipEncoder,
    reset::{
        construct_reset_strategy_sequence, ClassicReset, HardReset, ResetAfterOperation,
        ResetBeforeOperation, ResetStrategy, UsbJtagSerialReset,
    },
};
use crate::{
    command::{Command, CommandType},
    connection::reset::soft_reset,
    error::{ConnectionError, Error, ResultExt, RomError, RomErrorKind},
    interface::Interface,
    targets::Chip,
};

pub mod reset;

const MAX_CONNECT_ATTEMPTS: usize = 7;
const MAX_SYNC_ATTEMPTS: usize = 5;
pub(crate) const USB_SERIAL_JTAG_PID: u16 = 0x1001;

#[derive(Debug, Copy, Clone)]
pub enum CommandResponseValue {
    ValueU32(u32),
    ValueU128(u128),
}

impl TryInto<u32> for CommandResponseValue {
    type Error = crate::error::Error;

    fn try_into(self) -> Result<u32, Self::Error> {
        match self {
            CommandResponseValue::ValueU32(value) => Ok(value),
            CommandResponseValue::ValueU128(_) => Err(crate::error::Error::InternalError),
        }
    }
}

impl TryInto<u128> for CommandResponseValue {
    type Error = crate::error::Error;

    fn try_into(self) -> Result<u128, Self::Error> {
        match self {
            CommandResponseValue::ValueU32(_) => Err(crate::error::Error::InternalError),
            CommandResponseValue::ValueU128(value) => Ok(value),
        }
    }
}

/// A response from a target device following a command
#[derive(Debug, Copy, Clone)]
pub struct CommandResponse {
    pub resp: u8,
    pub return_op: u8,
    pub return_length: u16,
    pub value: CommandResponseValue,
    pub error: u8,
    pub status: u8,
}

/// An established connection with a target device
pub struct Connection {
    serial: Interface,
    port_info: UsbPortInfo,
    decoder: SlipDecoder,
    after_operation: ResetAfterOperation,
    before_operation: ResetBeforeOperation,
}

impl Connection {
    pub fn new(
        serial: Interface,
        port_info: UsbPortInfo,
        after_operation: ResetAfterOperation,
        before_operation: ResetBeforeOperation,
    ) -> Self {
        Connection {
            serial,
            port_info,
            decoder: SlipDecoder::new(),
            after_operation,
            before_operation,
        }
    }

    /// Initialize a connection with a device
    pub fn begin(&mut self) -> Result<(), Error> {
        let port_name = self.serial.serial_port().name().unwrap_or_default();
        let reset_sequence = construct_reset_strategy_sequence(
            &port_name,
            self.port_info.pid,
            self.before_operation,
        );

        for (_, reset_strategy) in zip(0..MAX_CONNECT_ATTEMPTS, reset_sequence.iter().cycle()) {
            match self.connect_attempt(reset_strategy) {
                Ok(_) => {
                    return Ok(());
                }
                Err(e) => {
                    debug!("Failed to reset, error {:#?}, retrying", e);
                }
            }
        }

        Err(Error::Connection(ConnectionError::ConnectionFailed))
    }

    /// Try to connect to a device
    #[allow(clippy::borrowed_box)]
    fn connect_attempt(&mut self, reset_strategy: &Box<dyn ResetStrategy>) -> Result<(), Error> {
        // If we're doing no_sync, we're likely communicating as a pass through
        // with an intermediate device to the ESP32
        if self.before_operation == ResetBeforeOperation::NoResetNoSync {
            return Ok(());
        }
        let mut download_mode: bool = false;
        let mut boot_mode: &str = "";
        let mut boot_log_detected = false;
        let mut buff: Vec<u8>;
        if self.before_operation != ResetBeforeOperation::NoReset {
            // Reset the chip to bootloader (download mode)
            reset_strategy.reset(&mut self.serial)?;

            let available_bytes = self.serial.serial_port_mut().bytes_to_read()?;
            buff = vec![0; available_bytes as usize];
            let read_bytes = self.serial.serial_port_mut().read(&mut buff)? as u32;

            if read_bytes != available_bytes {
                return Err(Error::Connection(ConnectionError::ReadMissmatch(
                    available_bytes,
                    read_bytes,
                )));
            }

            let read_slice = std::str::from_utf8(&buff[..read_bytes as usize]).unwrap();

            let pattern = Regex::new(r"boot:(0x[0-9a-fA-F]+)(.*waiting for download)?").unwrap();

            // Search for the pattern in the read data
            if let Some(data) = pattern.captures(read_slice) {
                boot_log_detected = true;
                // Boot log detected
                boot_mode = data.get(1).map(|m| m.as_str()).unwrap_or_default();
                download_mode = data.get(2).is_some();

                // Further processing or printing the results
                debug!("Boot Mode: {}", boot_mode);
                debug!("Download Mode: {}", download_mode);
            };
        }

        for _ in 0..MAX_SYNC_ATTEMPTS {
            self.flush()?;

            if self.sync().is_ok() {
                return Ok(());
            }
        }

        if boot_log_detected {
            if download_mode {
                return Err(Error::Connection(ConnectionError::NoSyncReply));
            } else {
                return Err(Error::Connection(ConnectionError::WrongBootMode(
                    boot_mode.to_string(),
                )));
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

            for _ in 0..MAX_CONNECT_ATTEMPTS {
                match connection.read_response()? {
                    Some(response) if response.return_op == CommandType::Sync as u8 => {
                        if response.status == 1 {
                            connection.flush().ok();
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
        reset_after_flash(&mut self.serial, self.port_info.pid)?;

        Ok(())
    }

    // Reset the device taking into account the reset after argument
    pub fn reset_after(&mut self, is_stub: bool, chip: Chip) -> Result<(), Error> {
        match self.after_operation {
            ResetAfterOperation::HardReset => HardReset.reset(&mut self.serial),
            ResetAfterOperation::SoftReset => {
                println!("Soft resetting");
                soft_reset(self, false, is_stub, chip)?;
                Ok(())
            }
            ResetAfterOperation::NoReset => {
                println!("Staying in bootloader");
                soft_reset(self, true, is_stub, chip)?;

                Ok(())
            }
            ResetAfterOperation::NoResetNoStub => {
                println!("Staying in flasher stub");
                Ok(())
            }
        }
    }

    // Reset the device to flash mode
    pub fn reset_to_flash(&mut self, extra_delay: bool) -> Result<(), Error> {
        if self.port_info.pid == USB_SERIAL_JTAG_PID {
            UsbJtagSerialReset.reset(&mut self.serial)
        } else {
            #[cfg(unix)]
            if UnixTightReset::new(extra_delay)
                .reset(&mut self.serial)
                .is_ok()
            {
                return Ok(());
            }

            ClassicReset::new(extra_delay).reset(&mut self.serial)
        }
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
    pub fn with_timeout<T, F>(&mut self, timeout: Duration, mut f: F) -> Result<T, Error>
    where
        F: FnMut(&mut Connection) -> Result<T, Error>,
    {
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
                // here is what esptool does: https://github.com/espressif/esptool/blob/master/esptool/loader.py#L458
                // from esptool: things are a bit weird here, bear with us

                // we rely on the known and expected response sizes which should be fine for now - if that changes we need to pass the command type
                // we are parsing the response for
                // for most commands the response length is 10 (for the stub) or 12 (for ROM code)
                // the MD5 command response is 44 for ROM loader, 26 for the stub
                // see https://docs.espressif.com/projects/esptool/en/latest/esp32/advanced-topics/serial-protocol.html?highlight=md5#response-packet
                // see https://docs.espressif.com/projects/esptool/en/latest/esp32/advanced-topics/serial-protocol.html?highlight=md5#status-bytes
                // see https://docs.espressif.com/projects/esptool/en/latest/esp32/advanced-topics/serial-protocol.html?highlight=md5#verifying-uploaded-data
                let status_len = if response.len() == 10 || response.len() == 26 {
                    2
                } else {
                    4
                };

                let value = match response.len() {
                    10 | 12 => CommandResponseValue::ValueU32(u32::from_le_bytes(
                        response[4..][..4].try_into().unwrap(),
                    )),
                    44 => {
                        // MD5 is in ASCII
                        CommandResponseValue::ValueU128(
                            u128::from_str_radix(
                                std::str::from_utf8(&response[8..][..32]).unwrap(),
                                16,
                            )
                            .unwrap(),
                        )
                    }
                    26 => {
                        // MD5 is BE bytes
                        CommandResponseValue::ValueU128(u128::from_be_bytes(
                            response[8..][..16].try_into().unwrap(),
                        ))
                    }
                    _ => {
                        return Err(Error::InternalError);
                    }
                };

                let header = CommandResponse {
                    resp: response[0],
                    return_op: response[1],
                    return_length: u16::from_le_bytes(response[2..][..2].try_into().unwrap()),
                    value,
                    error: response[response.len() - status_len],
                    status: response[response.len() - status_len + 1],
                };

                Ok(Some(header))
            }
        }
    }

    /// Write a command to the serial port
    pub fn write_command(&mut self, command: Command) -> Result<(), Error> {
        debug!("Writing command: {:?}", command);
        let serial = self.serial.serial_port_mut();

        serial.clear(serialport::ClearBuffer::Input)?;
        let mut writer = BufWriter::new(serial);
        let mut encoder = SlipEncoder::new(&mut writer)?;
        command.write(&mut encoder)?;
        encoder.finish()?;
        writer.flush()?;
        Ok(())
    }

    ///  Write a command and reads the response
    pub fn command(&mut self, command: Command) -> Result<CommandResponseValue, Error> {
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
        .map(|v| v.try_into().unwrap())
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

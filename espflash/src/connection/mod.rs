//! Establish a connection with a target device
//!
//! The [Connection] struct abstracts over the serial connection and
//! sending/decoding of commands, and provides higher-level operations with the
//! device.

use std::{
    io::{BufWriter, Read, Write},
    iter::zip,
    thread::sleep,
    time::Duration,
};

use log::{debug, info};
use regex::Regex;
use reset::wdt_reset;
use serialport::{SerialPort, UsbPortInfo};
use slip_codec::SlipDecoder;

#[cfg(unix)]
use self::reset::UnixTightReset;
use self::{
    command::{Command, CommandType},
    encoder::SlipEncoder,
    reset::{
        construct_reset_strategy_sequence,
        hard_reset,
        reset_after_flash,
        soft_reset,
        ClassicReset,
        ResetAfterOperation,
        ResetBeforeOperation,
        ResetStrategy,
        UsbJtagSerialReset,
    },
};
use crate::{
    error::{ConnectionError, Error, ResultExt, RomError, RomErrorKind},
    targets::{esp32p4, esp32s2, esp32s3, Chip},
};

pub(crate) mod command;
pub(crate) mod reset;

const MAX_CONNECT_ATTEMPTS: usize = 7;
const MAX_SYNC_ATTEMPTS: usize = 5;
const USB_SERIAL_JTAG_PID: u16 = 0x1001;

#[cfg(unix)]
pub type Port = serialport::TTYPort;
#[cfg(windows)]
pub type Port = serialport::COMPort;

#[derive(Debug, Clone)]
pub enum CommandResponseValue {
    ValueU32(u32),
    ValueU128(u128),
    Vector(Vec<u8>),
}

impl TryInto<u32> for CommandResponseValue {
    type Error = Error;

    fn try_into(self) -> Result<u32, Self::Error> {
        match self {
            CommandResponseValue::ValueU32(value) => Ok(value),
            CommandResponseValue::ValueU128(_) => Err(Error::InvalidResponse(
                "expected `u32` but found `u128`".into(),
            )),
            CommandResponseValue::Vector(_) => Err(Error::InvalidResponse(
                "expected `u32` but found `Vec`".into(),
            )),
        }
    }
}

impl TryInto<u128> for CommandResponseValue {
    type Error = Error;

    fn try_into(self) -> Result<u128, Self::Error> {
        match self {
            CommandResponseValue::ValueU32(_) => Err(Error::InvalidResponse(
                "expected `u128` but found `u32`".into(),
            )),
            CommandResponseValue::ValueU128(value) => Ok(value),
            CommandResponseValue::Vector(_) => Err(Error::InvalidResponse(
                "expected `u128` but found `Vec`".into(),
            )),
        }
    }
}

impl TryInto<Vec<u8>> for CommandResponseValue {
    type Error = Error;

    fn try_into(self) -> Result<Vec<u8>, Self::Error> {
        match self {
            CommandResponseValue::ValueU32(_) => Err(Error::InvalidResponse(
                "expected `Vec` but found `u32`".into(),
            )),
            CommandResponseValue::ValueU128(_) => Err(Error::InvalidResponse(
                "expected `Vec` but found `u128`".into(),
            )),
            CommandResponseValue::Vector(value) => Ok(value),
        }
    }
}

/// A response from a target device following a command
#[derive(Debug, Clone)]
pub struct CommandResponse {
    pub resp: u8,
    pub return_op: u8,
    pub return_length: u16,
    pub value: CommandResponseValue,
    pub error: u8,
    pub status: u8,
}

/// An established connection with a target device
#[derive(Debug)]
pub struct Connection {
    serial: Port,
    port_info: UsbPortInfo,
    decoder: SlipDecoder,
    after_operation: ResetAfterOperation,
    before_operation: ResetBeforeOperation,
}

impl Connection {
    pub fn new(
        serial: Port,
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
        let port_name = self.serial.name().unwrap_or_default();
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
        let mut boot_mode = String::new();
        let mut boot_log_detected = false;
        let mut buff: Vec<u8>;
        if self.before_operation != ResetBeforeOperation::NoReset {
            // Reset the chip to bootloader (download mode)
            reset_strategy.reset(&mut self.serial)?;

            let available_bytes = self.serial.bytes_to_read()?;
            buff = vec![0; available_bytes as usize];
            let read_bytes = self.serial.read(&mut buff)? as u32;

            if read_bytes != available_bytes {
                return Err(Error::Connection(ConnectionError::ReadMissmatch(
                    available_bytes,
                    read_bytes,
                )));
            }

            let read_slice = String::from_utf8_lossy(&buff[..read_bytes as usize]).into_owned();

            let pattern =
                Regex::new(r"boot:(0x[0-9a-fA-F]+)([\s\S]*waiting for download)?").unwrap();

            // Search for the pattern in the read data
            if let Some(data) = pattern.captures(&read_slice) {
                boot_log_detected = true;
                // Boot log detected
                boot_mode = data
                    .get(1)
                    .map(|m| m.as_str())
                    .unwrap_or_default()
                    .to_string();
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
        let pid = self.usb_pid();

        match self.after_operation {
            ResetAfterOperation::HardReset => hard_reset(&mut self.serial, pid),
            ResetAfterOperation::NoReset => {
                info!("Staying in bootloader");
                soft_reset(self, true, is_stub)?;

                Ok(())
            }
            ResetAfterOperation::NoResetNoStub => {
                info!("Staying in flasher stub");
                Ok(())
            }
            ResetAfterOperation::WatchdogReset => {
                info!("Resetting device with watchdog");

                match chip {
                    Chip::Esp32c3 => {
                        if pid == USB_SERIAL_JTAG_PID {
                            wdt_reset(chip, self)?;
                        }
                    }
                    Chip::Esp32p4 => {
                        // Check if the connection is USB OTG
                        if self.is_using_usb_otg(chip)? {
                            wdt_reset(chip, self)?;
                        }
                    }
                    Chip::Esp32s2 => {
                        let esp32s2 = esp32s2::Esp32s2;
                        // Check if the connection is USB OTG
                        if self.is_using_usb_otg(chip)? {
                            // Check the strapping register to see if we can perform RTC WDT
                            // reset
                            if esp32s2.can_wtd_reset(self)? {
                                wdt_reset(chip, self)?;
                            }
                        }
                    }
                    Chip::Esp32s3 => {
                        let esp32s3 = esp32s3::Esp32s3;
                        if pid == USB_SERIAL_JTAG_PID || self.is_using_usb_otg(chip)? {
                            // Check the strapping register to see if we can perform RTC WDT
                            // reset
                            if esp32s3.can_wtd_reset(self)? {
                                wdt_reset(chip, self)?;
                            }
                        }
                    }
                    _ => {
                        return Err(Error::UnsupportedFeature {
                            chip,
                            feature: "watchdog reset".into(),
                        })
                    }
                }

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
        self.serial.set_timeout(timeout)?;
        Ok(())
    }

    /// Set baud rate for the serial port
    pub fn set_baud(&mut self, speed: u32) -> Result<(), Error> {
        self.serial.set_baud_rate(speed)?;

        Ok(())
    }

    /// Get the current baud rate of the serial port
    pub fn get_baud(&self) -> Result<u32, Error> {
        Ok(self.serial.baud_rate()?)
    }

    /// Run a command with a timeout defined by the command type
    pub fn with_timeout<T, F>(&mut self, timeout: Duration, mut f: F) -> Result<T, Error>
    where
        F: FnMut(&mut Connection) -> Result<T, Error>,
    {
        let old_timeout = {
            let mut binding = Box::new(&mut self.serial);
            let serial = binding.as_mut();
            let old_timeout = serial.timeout();
            serial.set_timeout(timeout)?;
            old_timeout
        };

        let result = f(self);

        self.serial.set_timeout(old_timeout)?;

        result
    }

    /// Read the response from a serial port
    pub fn read_flash_response(&mut self) -> Result<Option<CommandResponse>, Error> {
        let mut response = Vec::new();

        self.decoder.decode(&mut self.serial, &mut response)?;

        if response.is_empty() {
            return Ok(None);
        }
        let value = CommandResponseValue::Vector(response.clone());

        let header = CommandResponse {
            resp: 1_u8,
            return_op: CommandType::ReadFlash as u8,
            return_length: response.len() as u16,
            value,
            error: 0_u8,
            status: 0_u8,
        };

        Ok(Some(header))
    }

    /// Read the response from a serial port
    pub fn read_response(&mut self) -> Result<Option<CommandResponse>, Error> {
        match self.read(10)? {
            None => Ok(None),
            Some(response) => {
                // Here is what esptool does: https://github.com/espressif/esptool/blob/81b2eaee261aed0d3d754e32c57959d6b235bfed/esptool/loader.py#L518
                // from esptool: things are a bit weird here, bear with us

                // We rely on the known and expected response sizes which should be fine for now
                // - if that changes we need to pass the command type we are parsing the
                // response for.
                //
                // For most commands the response length is 10 (for the stub) or 12 (for ROM
                // code). The MD5 command response is 44 for ROM loader, 26 for the stub.
                //
                // See:
                // - https://docs.espressif.com/projects/esptool/en/latest/esp32/advanced-topics/serial-protocol.html?highlight=md5#response-packet
                // - https://docs.espressif.com/projects/esptool/en/latest/esp32/advanced-topics/serial-protocol.html?highlight=md5#status-bytes
                // - https://docs.espressif.com/projects/esptool/en/latest/esp32/advanced-topics/serial-protocol.html?highlight=md5#verifying-uploaded-data
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
                    _ => CommandResponseValue::Vector(response.clone()),
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

    /// Write raw data to the serial port
    pub fn write_raw(&mut self, data: u32) -> Result<(), Error> {
        let mut binding = Box::new(&mut self.serial);
        let serial = binding.as_mut();
        serial.clear(serialport::ClearBuffer::Input)?;
        let mut writer = BufWriter::new(serial);
        let mut encoder = SlipEncoder::new(&mut writer)?;
        encoder.write_all(&data.to_le_bytes())?;
        encoder.finish()?;
        writer.flush()?;
        Ok(())
    }

    /// Write a command to the serial port
    pub fn write_command(&mut self, command: Command<'_>) -> Result<(), Error> {
        debug!("Writing command: {:02x?}", command);
        let mut binding = Box::new(&mut self.serial);
        let serial = binding.as_mut();

        serial.clear(serialport::ClearBuffer::Input)?;
        let mut writer = BufWriter::new(serial);
        let mut encoder = SlipEncoder::new(&mut writer)?;
        command.write(&mut encoder)?;
        encoder.finish()?;
        writer.flush()?;
        Ok(())
    }

    ///  Write a command and reads the response
    pub fn command(&mut self, command: Command<'_>) -> Result<CommandResponseValue, Error> {
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
                        // Check if the response is a Vector and strip header (first 8 bytes)
                        // https://github.com/espressif/esptool/blob/749d1ad/esptool/loader.py#L481
                        let modified_value = match response.value {
                            CommandResponseValue::Vector(mut vec) if vec.len() >= 8 => {
                                vec = vec[8..][..response.return_length as usize].to_vec();
                                CommandResponseValue::Vector(vec)
                            }
                            _ => response.value, // If not Vector, return as is
                        };

                        Ok(modified_value)
                    };
                }
                _ => continue,
            }
        }
        Err(Error::Connection(ConnectionError::ConnectionFailed))
    }

    /// Read a register command with a timeout
    pub fn read_reg(&mut self, reg: u32) -> Result<u32, Error> {
        let resp = self.with_timeout(CommandType::ReadReg.timeout(), |connection| {
            connection.command(Command::ReadReg { address: reg })
        })?;

        resp.try_into()
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
        self.serial.flush()?;
        Ok(())
    }

    /// Turn a serial port into a [Port]
    pub fn into_serial(self) -> Port {
        self.serial
    }

    /// Get the USB PID of the serial port
    pub fn usb_pid(&self) -> u16 {
        self.port_info.pid
    }

    pub(crate) fn is_using_usb_serial_jtag(&self) -> bool {
        self.port_info.pid == USB_SERIAL_JTAG_PID
    }

    #[cfg(feature = "serialport")]
    /// Check if the connection is USB OTG
    pub(crate) fn is_using_usb_otg(&mut self, chip: Chip) -> Result<bool, Error> {
        let (buf_no, no_usb_otg) = match chip {
            Chip::Esp32p4 => (esp32p4::UARTDEV_BUF_NO, esp32p4::UARTDEV_BUF_NO_USB_OTG),
            Chip::Esp32s2 => (esp32s2::UARTDEV_BUF_NO, esp32s2::UARTDEV_BUF_NO_USB_OTG),
            Chip::Esp32s3 => (esp32s3::UARTDEV_BUF_NO, esp32s3::UARTDEV_BUF_NO_USB_OTG),
            _ => unreachable!(),
        };

        Ok(self.read_reg(buf_no)? == no_usb_otg)
    }
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

    impl<W: Write> Write for SlipEncoder<'_, W> {
        /// Writes the given buffer replacing the END and ESC bytes
        ///
        /// See https://docs.espressif.com/projects/esptool/en/latest/esp32c3/advanced-topics/serial-protocol.html#low-level-protocol
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

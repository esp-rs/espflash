//! Establish a connection with a target device.
//!
//! The [Connection] struct abstracts over the serial connection and
//! sending/decoding of commands, and provides higher-level operations with the
//! device.

use std::{
    collections::HashMap,
    fmt,
    io::{self, Read, Write},
    iter::zip,
    thread::sleep,
    time::{Duration, Instant},
};

use log::{debug, info};
use regex::Regex;
use serde::{Deserialize, Serialize};
use serialport::{SerialPort, UsbPortInfo};
use slip_codec::SlipDecoder;

#[cfg(unix)]
use self::reset::UnixTightReset;
use self::{
    encoder::SlipEncoder,
    reset::{
        ClassicReset,
        ResetStrategy,
        UsbJtagSerialReset,
        construct_reset_strategy_sequence,
        hard_reset,
        reset_after_flash,
        soft_reset,
    },
};
use crate::{
    command::{Command, CommandResponse, CommandResponseValue, CommandType, DEFAULT_MAX_LEN},
    error::{ConnectionError, Error, ResultExt, RomError, RomErrorKind},
    flasher::stubs::CHIP_DETECT_MAGIC_REG_ADDR,
    target::Chip,
};

pub(crate) mod reset;

pub use reset::{ResetAfterOperation, ResetBeforeOperation};

const MAX_CONNECT_ATTEMPTS: usize = 7;
const MAX_SYNC_ATTEMPTS: usize = 5;
const USB_SERIAL_JTAG_PID: u16 = 0x1001;

#[cfg(unix)]
/// Alias for the serial TTYPort.
pub type Port = serialport::TTYPort;
#[cfg(windows)]
/// Alias for the serial COMPort.
pub type Port = serialport::COMPort;

/// Security Info Response containing chip security information
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize, Serialize)]
pub struct SecurityInfo {
    /// 32 bits flags
    pub flags: u32,
    /// 1 byte flash_crypt_cnt
    pub flash_crypt_cnt: u8,
    /// 7 bytes key purposes
    pub key_purposes: [u8; 7],
    /// 32-bit word chip id
    pub chip_id: Option<u32>,
    /// 32-bit word eco version
    pub eco_version: Option<u32>,
}

impl SecurityInfo {
    fn security_flag_map() -> HashMap<&'static str, u32> {
        HashMap::from([
            ("SECURE_BOOT_EN", 1 << 0),
            ("SECURE_BOOT_AGGRESSIVE_REVOKE", 1 << 1),
            ("SECURE_DOWNLOAD_ENABLE", 1 << 2),
            ("SECURE_BOOT_KEY_REVOKE0", 1 << 3),
            ("SECURE_BOOT_KEY_REVOKE1", 1 << 4),
            ("SECURE_BOOT_KEY_REVOKE2", 1 << 5),
            ("SOFT_DIS_JTAG", 1 << 6),
            ("HARD_DIS_JTAG", 1 << 7),
            ("DIS_USB", 1 << 8),
            ("DIS_DOWNLOAD_DCACHE", 1 << 9),
            ("DIS_DOWNLOAD_ICACHE", 1 << 10),
        ])
    }

    pub(crate) fn security_flag_status(&self, flag_name: &str) -> bool {
        if let Some(&flag) = Self::security_flag_map().get(flag_name) {
            (self.flags & flag) != 0
        } else {
            false
        }
    }
}

impl TryFrom<&[u8]> for SecurityInfo {
    type Error = Error;

    fn try_from(bytes: &[u8]) -> Result<Self, Self::Error> {
        let esp32s2 = bytes.len() == 12;

        if bytes.len() < 12 {
            return Err(Error::InvalidResponse(format!(
                "expected response of at least 12 bytes, received {} bytes",
                bytes.len()
            )));
        }

        // Parse response bytes
        let flags = u32::from_le_bytes(bytes[0..4].try_into()?);
        let flash_crypt_cnt = bytes[4];
        let key_purposes: [u8; 7] = bytes[5..12].try_into()?;

        let (chip_id, eco_version) = if esp32s2 {
            (None, None) // ESP32-S2 doesn't have these values
        } else {
            if bytes.len() < 20 {
                return Err(Error::InvalidResponse(format!(
                    "expected response of at least 20 bytes, received {} bytes",
                    bytes.len()
                )));
            }
            let chip_id = u32::from_le_bytes(bytes[12..16].try_into()?);
            let eco_version = u32::from_le_bytes(bytes[16..20].try_into()?);
            (Some(chip_id), Some(eco_version))
        };

        Ok(SecurityInfo {
            flags,
            flash_crypt_cnt,
            key_purposes,
            chip_id,
            eco_version,
        })
    }
}

impl fmt::Display for SecurityInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let key_purposes_str = self
            .key_purposes
            .iter()
            .map(|b| format!("{b}"))
            .collect::<Vec<_>>()
            .join(", ");

        writeln!(f, "\nSecurity Information:")?;
        writeln!(f, "=====================")?;
        writeln!(f, "Flags: {:#010x} ({:b})", self.flags, self.flags)?;
        writeln!(f, "Key Purposes: [{key_purposes_str}]")?;

        // Only print Chip ID if it's Some(value)
        if let Some(chip_id) = self.chip_id {
            writeln!(f, "Chip ID: {chip_id}")?;
        }

        // Only print API Version if it's Some(value)
        if let Some(api_version) = self.eco_version {
            writeln!(f, "API Version: {api_version}")?;
        }

        // Secure Boot
        if self.security_flag_status("SECURE_BOOT_EN") {
            writeln!(f, "Secure Boot: Enabled")?;
            if self.security_flag_status("SECURE_BOOT_AGGRESSIVE_REVOKE") {
                writeln!(f, "Secure Boot Aggressive key revocation: Enabled")?;
            }

            let revoked_keys: Vec<_> = [
                "SECURE_BOOT_KEY_REVOKE0",
                "SECURE_BOOT_KEY_REVOKE1",
                "SECURE_BOOT_KEY_REVOKE2",
            ]
            .iter()
            .enumerate()
            .filter(|(_, key)| self.security_flag_status(key))
            .map(|(i, _)| format!("Secure Boot Key{i} is Revoked"))
            .collect();

            if !revoked_keys.is_empty() {
                writeln!(
                    f,
                    "Secure Boot Key Revocation Status:\n  {}",
                    revoked_keys.join("\n  ")
                )?;
            }
        } else {
            writeln!(f, "Secure Boot: Disabled")?;
        }

        // Flash Encryption
        if !self.flash_crypt_cnt.count_ones().is_multiple_of(2) {
            writeln!(f, "Flash Encryption: Enabled")?;
        } else {
            writeln!(f, "Flash Encryption: Disabled")?;
        }

        let crypt_cnt_str = "SPI Boot Crypt Count (SPI_BOOT_CRYPT_CNT)";
        writeln!(f, "{}: 0x{:x}", crypt_cnt_str, self.flash_crypt_cnt)?;

        // Cache Disabling
        if self.security_flag_status("DIS_DOWNLOAD_DCACHE") {
            writeln!(f, "Dcache in UART download mode: Disabled")?;
        }
        if self.security_flag_status("DIS_DOWNLOAD_ICACHE") {
            writeln!(f, "Icache in UART download mode: Disabled")?;
        }

        // JTAG Status
        if self.security_flag_status("HARD_DIS_JTAG") {
            writeln!(f, "JTAG: Permanently Disabled")?;
        } else if self.security_flag_status("SOFT_DIS_JTAG") {
            writeln!(f, "JTAG: Software Access Disabled")?;
        }

        // USB Access
        if self.security_flag_status("DIS_USB") {
            writeln!(f, "USB Access: Disabled")?;
        }

        Ok(())
    }
}

/// An established connection with a target device.
#[derive(Debug)]
pub struct Connection {
    serial: Port,
    port_info: UsbPortInfo,
    decoder: SlipDecoder,
    after_operation: ResetAfterOperation,
    before_operation: ResetBeforeOperation,
    pub(crate) secure_download_mode: bool,
    pub(crate) baud: u32,
}

impl Connection {
    /// Creates a new connection with a target device.
    pub fn new(
        serial: Port,
        port_info: UsbPortInfo,
        after_operation: ResetAfterOperation,
        before_operation: ResetBeforeOperation,
        baud: u32,
    ) -> Self {
        Connection {
            serial,
            port_info,
            decoder: SlipDecoder::new(),
            after_operation,
            before_operation,
            secure_download_mode: false,
            baud,
        }
    }

    /// Initializes a connection with a device.
    pub fn begin(&mut self) -> Result<(), Error> {
        let port_name = self.serial.name().unwrap_or_default();
        let reset_sequence = construct_reset_strategy_sequence(
            &port_name,
            self.port_info.pid,
            self.before_operation,
        );

        for (_, reset_strategy) in zip(0..MAX_CONNECT_ATTEMPTS, reset_sequence.iter().cycle()) {
            match self.connect_attempt(reset_strategy.as_ref()) {
                Ok(_) => {
                    return Ok(());
                }
                Err(e) => {
                    debug!("Failed to reset, error {e:#?}, retrying");
                }
            }
        }

        Err(Error::Connection(Box::new(
            ConnectionError::ConnectionFailed,
        )))
    }

    /// Connects to a device.
    fn connect_attempt(&mut self, reset_strategy: &dyn ResetStrategy) -> Result<(), Error> {
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

            // S2 in USB download mode responds with 0 available bytes here
            let available_bytes = self.serial.bytes_to_read()?;

            buff = vec![0; available_bytes as usize];
            let read_bytes = if available_bytes > 0 {
                let read_bytes = self.serial.read(&mut buff)? as u32;

                if read_bytes != available_bytes {
                    return Err(Error::Connection(Box::new(ConnectionError::ReadMismatch(
                        available_bytes,
                        read_bytes,
                    ))));
                }
                read_bytes
            } else {
                0
            };

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
                debug!("Boot Mode: {boot_mode}");
                debug!("Download Mode: {download_mode}");
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
                return Err(Error::Connection(Box::new(ConnectionError::NoSyncReply)));
            } else {
                return Err(Error::Connection(Box::new(ConnectionError::WrongBootMode(
                    boot_mode.to_string(),
                ))));
            }
        }

        Err(Error::Connection(Box::new(
            ConnectionError::ConnectionFailed,
        )))
    }

    /// Syncs with a device.
    pub(crate) fn sync(&mut self) -> Result<(), Error> {
        self.with_timeout(CommandType::Sync.timeout(), |connection| {
            connection.command(Command::Sync)?;
            connection.flush()?;

            sleep(Duration::from_millis(10));

            for _ in 0..MAX_CONNECT_ATTEMPTS {
                match connection.read_response_for_command(CommandType::Sync)? {
                    Some(response) if response.return_op == CommandType::Sync as u8 => {
                        if response.status == 1 {
                            connection.flush().ok();
                            return Err(Error::RomError(Box::new(RomError::new(
                                CommandType::Sync,
                                RomErrorKind::from(response.error),
                            ))));
                        }
                    }
                    _ => {
                        return Err(Error::RomError(Box::new(RomError::new(
                            CommandType::Sync,
                            RomErrorKind::InvalidMessage,
                        ))));
                    }
                }
            }

            Ok(())
        })?;

        Ok(())
    }

    /// Resets the device.
    pub fn reset(&mut self) -> Result<(), Error> {
        reset_after_flash(&mut self.serial, self.port_info.pid)?;

        Ok(())
    }

    /// Resets the device taking into account the reset after argument.
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
                    Chip::Esp32c3 | Chip::Esp32c5 => {
                        if self.is_using_usb_serial_jtag() {
                            chip.rtc_wdt_reset(self)?;
                        }
                    }
                    Chip::Esp32p4 => {
                        // Check if the connection is USB OTG
                        if chip.is_using_usb_otg(self)? {
                            chip.rtc_wdt_reset(self)?;
                        }
                    }
                    Chip::Esp32s2 => {
                        // Check if the connection is USB OTG
                        if chip.is_using_usb_otg(self)? {
                            // Check the strapping register to see if we can
                            // perform RTC WDT reset
                            if chip.can_rtc_wdt_reset(self)? {
                                chip.rtc_wdt_reset(self)?;
                            }
                        }
                    }
                    Chip::Esp32s3 => {
                        if self.is_using_usb_serial_jtag() || chip.is_using_usb_otg(self)? {
                            // Check the strapping register to see if we can
                            // perform RTC WDT reset
                            if chip.can_rtc_wdt_reset(self)? {
                                chip.rtc_wdt_reset(self)?;
                            }
                        }
                    }
                    _ => {
                        return Err(Error::UnsupportedFeature {
                            chip,
                            feature: "watchdog reset".into(),
                        });
                    }
                }

                Ok(())
            }
        }
    }

    /// Resets the device to flash mode.
    pub fn reset_to_flash(&mut self, extra_delay: bool) -> Result<(), Error> {
        if self.is_using_usb_serial_jtag() {
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

    /// Sets the timeout for the serial port.
    pub fn set_timeout(&mut self, timeout: Duration) -> Result<(), Error> {
        self.serial.set_timeout(timeout)?;
        Ok(())
    }

    /// Sets the baud rate for the serial port.
    pub fn set_baud(&mut self, baud: u32) -> Result<(), Error> {
        self.serial.set_baud_rate(baud)?;
        self.baud = baud;
        Ok(())
    }

    /// Returns the current baud rate of the serial port.
    pub fn baud(&self) -> Result<u32, Error> {
        Ok(self.serial.baud_rate()?)
    }

    /// Runs a command with a timeout defined by the command type.
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

    /// Runs `f` with a deadline one serial timeout from now.
    fn with_deadline<T>(
        &mut self,
        f: impl FnOnce(&mut Connection, Instant) -> Result<T, Error>,
    ) -> Result<T, Error> {
        let timeout = self.serial.timeout();
        let result = f(self, Instant::now() + timeout);
        if result.is_err() {
            // Output the device has not taken would otherwise stay queued and
            // block later writes and closing the port. Keep the original error.
            let _error = self.serial.clear(serialport::ClearBuffer::Output);
        }
        // `DeadlinePort` shortens the serial timeout as the deadline approaches
        self.serial.set_timeout(timeout)?;

        result
    }

    /// Reads the response from a serial port.
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

    /// Reads the response from a serial port for a [`CommandType`].
    pub fn read_response_for_command(
        &mut self,
        ty: CommandType,
    ) -> Result<Option<CommandResponse>, Error> {
        self.with_deadline(|connection, deadline| {
            connection.read_response_bounded(ty.max_response_len(), deadline)
        })
        .for_command(ty)
    }

    /// Reads the response from a serial port.
    #[deprecated = "May halt on unexpected input from the port --please use `read_response_for_command` instead. Deprecated in https://github.com/esp-rs/espflash/pull/1007"]
    pub fn read_response(&mut self) -> Result<Option<CommandResponse>, Error> {
        // don't know the command to expect a response for -- use the default
        // max length (the entire flash size)
        self.with_deadline(|connection, deadline| {
            connection.read_response_bounded(DEFAULT_MAX_LEN, deadline)
        })
    }

    fn read_response_bounded(
        &mut self,
        max_len: u64,
        deadline: Instant,
    ) -> Result<Option<CommandResponse>, Error> {
        match self.read_bounded(10, max_len, deadline)? {
            None => Ok(None),
            Some(response) => {
                // Here is what esptool does: https://github.com/espressif/esptool/blob/81b2eaee261aed0d3d754e32c57959d6b235bfed/esptool/loader.py#L518
                // from esptool: things are a bit weird here, bear with us

                // We rely on the known and expected response sizes which should
                // be fine for now
                // - if that changes we need to pass the command type we are
                //   parsing the
                // response for.
                //
                // For most commands the response length is 10 (for the stub) or
                // 12 (for ROM code). The MD5 command response
                // is 44 for ROM loader, 26 for the stub.
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
                        response[4..][..4].try_into()?,
                    )),
                    // MD5 is in ASCII
                    44 => CommandResponseValue::ValueU128(u128::from_str_radix(
                        std::str::from_utf8(&response[8..][..32])?,
                        16,
                    )?),
                    // MD5 is BE bytes
                    26 => CommandResponseValue::ValueU128(u128::from_be_bytes(
                        response[8..][..16].try_into()?,
                    )),
                    _ => CommandResponseValue::Vector(response.clone()),
                };

                let header = CommandResponse {
                    resp: response[0],
                    return_op: response[1],
                    return_length: u16::from_le_bytes(response[2..][..2].try_into()?),
                    value,
                    error: response[response.len() - status_len + 1],
                    status: response[response.len() - status_len],
                };

                Ok(Some(header))
            }
        }
    }

    /// Writes raw data to the serial port.
    pub fn write_raw(&mut self, data: u32) -> Result<(), Error> {
        self.with_deadline(|connection, deadline| {
            connection.write_frame(&data.to_le_bytes(), deadline)
        })
    }

    /// Writes a command to the serial port.
    pub fn write_command(&mut self, command: Command<'_>) -> Result<(), Error> {
        self.with_deadline(|connection, deadline| connection.write_command_until(command, deadline))
    }

    fn write_command_until(
        &mut self,
        command: Command<'_>,
        deadline: Instant,
    ) -> Result<(), Error> {
        debug!("Writing command: {command:02x?}");
        let mut payload = Vec::new();
        command.write(&mut payload)?;
        self.write_frame(&payload, deadline)
    }

    /// Writes `payload` as a SLIP frame and waits until it has been sent.
    fn write_frame(&mut self, payload: &[u8], deadline: Instant) -> Result<(), Error> {
        let mut frame = Vec::with_capacity(payload.len() + 2);
        let mut encoder = SlipEncoder::new(&mut frame)?;
        encoder.write_all(payload)?;
        encoder.finish()?;

        self.serial.clear(serialport::ClearBuffer::Input)?;
        let mut port = DeadlinePort {
            serial: &mut self.serial,
            deadline,
        };
        port.write_all(&frame)
            .and_then(|()| port.flush())
            .map_err(|err| Error::Connection(Box::new(ConnectionError::from(err))))
    }

    /// Writes a command and reads the response.
    ///
    /// The whole exchange must complete within the serial timeout.
    pub fn command(&mut self, command: Command<'_>) -> Result<CommandResponseValue, Error> {
        let ty = command.command_type();
        let response = self.with_deadline(|connection, deadline| {
            connection
                .write_command_until(command, deadline)
                .for_command(ty)?;
            for _ in 0..100 {
                match connection
                    .read_response_bounded(ty.max_response_len(), deadline)
                    .for_command(ty)?
                {
                    Some(response) if response.return_op == ty as u8 => return Ok(response),
                    _ => continue,
                }
            }
            Err(Error::Connection(Box::new(
                ConnectionError::ConnectionFailed,
            )))
        })?;

        if response.status != 0 {
            let _error = self.flush();
            Err(Error::RomError(Box::new(RomError::new(
                ty,
                RomErrorKind::from(response.error),
            ))))
        } else {
            // Check if the response is a Vector and strip header
            // (first 8 bytes) https://github.com/espressif/esptool/blob/749d1ad/esptool/loader.py#L481
            let modified_value = match response.value {
                CommandResponseValue::Vector(mut vec) if vec.len() >= 8 => {
                    vec = vec[8..][..response.return_length as usize].to_vec();
                    CommandResponseValue::Vector(vec)
                }
                _ => response.value, // If not Vector, return as is
            };

            Ok(modified_value)
        }
    }

    /// Reads a register command with a timeout.
    pub fn read_reg(&mut self, addr: u32) -> Result<u32, Error> {
        let resp = self.with_timeout(CommandType::ReadReg.timeout(), |connection| {
            connection.command(Command::ReadReg { address: addr })
        })?;

        resp.try_into()
    }

    /// Writes a register command with a timeout.
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

    /// Updates a register by applying the new value to the masked out portion
    /// of the old value.
    pub(crate) fn update_reg(&mut self, addr: u32, mask: u32, new_value: u32) -> Result<(), Error> {
        let masked_new_value = new_value.checked_shl(mask.trailing_zeros()).unwrap_or(0) & mask;

        let masked_old_value = self.read_reg(addr)? & !mask;

        self.write_reg(addr, masked_old_value | masked_new_value, None)
    }

    /// Reads a register command.
    pub(crate) fn read(&mut self, len: usize) -> Result<Option<Vec<u8>>, Error> {
        self.with_deadline(|connection, deadline| connection.read_bounded(len, u64::MAX, deadline))
    }

    /// Reads a register command at most `max_len` bytes long.
    fn read_bounded(
        &mut self,
        len: usize,
        max_len: u64,
        deadline: Instant,
    ) -> Result<Option<Vec<u8>>, Error> {
        let mut tmp = Vec::with_capacity(1024);
        let mut serial = DeadlinePort {
            serial: &mut self.serial,
            deadline,
        }
        .take(max_len);
        loop {
            self.decoder.decode(&mut serial, &mut tmp)?;
            if tmp.len() >= len {
                return Ok(Some(tmp));
            }
        }
    }

    /// Waits until all pending output has been sent, for at most the serial
    /// timeout.
    pub fn flush(&mut self) -> Result<(), Error> {
        self.with_deadline(|connection, deadline| {
            DeadlinePort {
                serial: &mut connection.serial,
                deadline,
            }
            .flush()
            .map_err(|err| Error::Connection(Box::new(ConnectionError::from(err))))
        })
    }

    /// Turns a serial port into a [Port].
    pub fn into_serial(self) -> Port {
        self.serial
    }

    /// Returns the USB PID of the serial port.
    pub fn usb_pid(&self) -> u16 {
        self.port_info.pid
    }

    /// Returns if the connection is using USB serial JTAG.
    pub(crate) fn is_using_usb_serial_jtag(&self) -> bool {
        self.port_info.pid == USB_SERIAL_JTAG_PID
    }

    /// Returns the reset after operation.
    pub fn after_operation(&self) -> ResetAfterOperation {
        self.after_operation
    }

    /// Returns the reset before operation.
    pub fn before_operation(&self) -> ResetBeforeOperation {
        self.before_operation
    }

    /// Gets security information from the chip.
    #[cfg(feature = "serialport")]
    pub fn security_info(&mut self, use_stub: bool) -> Result<SecurityInfo, crate::error::Error> {
        self.with_timeout(CommandType::GetSecurityInfo.timeout(), |connection| {
            let response = connection.command(Command::GetSecurityInfo)?;
            // Extract raw bytes and convert them into `SecurityInfo`
            if let crate::command::CommandResponseValue::Vector(data) = response {
                // The payload ends with the status bytes: 2 for the stub, 4 for
                // the ROM loader.
                let status_len = if use_stub { 2 } else { 4 };
                SecurityInfo::try_from(&data[..data.len().saturating_sub(status_len)])
            } else {
                Err(Error::InvalidResponse(
                    "response was not a vector of bytes".into(),
                ))
            }
        })
    }

    /// Detects which chip is connected to this connection.
    #[cfg(feature = "serialport")]
    pub fn detect_chip(
        &mut self,
        use_stub: bool,
    ) -> Result<crate::target::Chip, crate::error::Error> {
        match self.security_info(use_stub) {
            Ok(info) if info.chip_id.is_some() => {
                let chip_id = info.chip_id.unwrap() as u16;
                let chip = Chip::try_from(chip_id)?;

                Ok(chip)
            }
            _ => {
                // Fall back to reading the magic value from the chip
                let magic = if use_stub {
                    self.with_timeout(CommandType::ReadReg.timeout(), |connection| {
                        connection.command(Command::ReadReg {
                            address: CHIP_DETECT_MAGIC_REG_ADDR,
                        })
                    })?
                    .try_into()?
                } else {
                    self.read_reg(CHIP_DETECT_MAGIC_REG_ADDR)?
                };
                debug!("Read chip magic value: 0x{magic:08x}");
                Chip::from_magic(magic)
            }
        }
    }
}

impl From<Connection> for Port {
    fn from(conn: Connection) -> Self {
        conn.into_serial()
    }
}

/// Serial port I/O that fails once `deadline` has passed.
///
/// The serial timeout alone bounds each read or write call rather than a
/// whole command, so a device that keeps sending unrelated bytes keeps a read
/// alive indefinitely, and `Port::flush` blocks in `tcdrain` or
/// `FlushFileBuffers` for as long as a USB device does not accept data.
struct DeadlinePort<'a> {
    serial: &'a mut Port,
    deadline: Instant,
}

impl DeadlinePort<'_> {
    /// Shortens the serial timeout to the time left until the deadline.
    fn limit_timeout(&mut self) -> io::Result<()> {
        let remaining = self.deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err(io::ErrorKind::TimedOut.into());
        }
        self.serial.set_timeout(remaining)?;
        Ok(())
    }
}

impl Read for DeadlinePort<'_> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.limit_timeout()?;
        self.serial.read(buf)
    }
}

impl Write for DeadlinePort<'_> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.limit_timeout()?;
        self.serial.write(buf)
    }

    /// Waits until the OS has sent all pending output to the device.
    fn flush(&mut self) -> io::Result<()> {
        while self.serial.bytes_to_write()? > 0 {
            if Instant::now() >= self.deadline {
                return Err(io::ErrorKind::TimedOut.into());
            }
            sleep(Duration::from_millis(1));
        }
        Ok(())
    }
}

mod encoder {
    use std::io::Write;

    use serde::Serialize;

    const END: u8 = 0xC0;
    const ESC: u8 = 0xDB;
    const ESC_END: u8 = 0xDC;
    const ESC_ESC: u8 = 0xDD;

    /// Encoder for the SLIP protocol.
    #[derive(Debug, PartialEq, Eq, Serialize, Hash)]
    pub struct SlipEncoder<'a, W: Write> {
        writer: &'a mut W,
        len: usize,
    }

    impl<'a, W: Write> SlipEncoder<'a, W> {
        /// Creates a new encoder context.
        pub fn new(writer: &'a mut W) -> std::io::Result<Self> {
            let len = writer.write(&[END])?;
            Ok(Self { writer, len })
        }

        /// Finishes the encoding.
        pub fn finish(mut self) -> std::io::Result<usize> {
            self.len += self.writer.write(&[END])?;
            Ok(self.len)
        }
    }

    impl<W: Write> Write for SlipEncoder<'_, W> {
        /// Writes the given buffer replacing the END and ESC bytes.
        ///
        /// See <https://docs.espressif.com/projects/esptool/en/latest/esp32c3/advanced-topics/serial-protocol.html#low-level-protocol>
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

#[cfg(all(test, unix))]
mod tests {
    use std::{sync::mpsc, thread};

    use super::*;

    const TIMEOUT: Duration = Duration::from_millis(200);
    /// END, 8-byte header, 4-byte address, END
    const READ_REG_FRAME_LEN: usize = 14;

    /// Sends `ReadReg` to a pseudo-terminal device driven by `device` and
    /// returns the result and how long it took, including closing the port.
    fn read_reg(device: impl FnOnce(&mut Port) + Send + 'static) -> (Result<u32, Error>, Duration) {
        let (mut device_port, host_port) = Port::pair().unwrap();
        let port_info = UsbPortInfo {
            vid: 0,
            pid: 0,
            serial_number: None,
            manufacturer: None,
            product: None,
        };
        let mut connection = Connection::new(
            host_port,
            port_info,
            ResetAfterOperation::HardReset,
            ResetBeforeOperation::NoReset,
            115_200,
        );
        // Closing the device end discards unread output, so the thread returns
        // its port to keep it open until the command has finished
        let _device = thread::spawn(move || {
            device(&mut device_port);
            device_port
        });

        // Run on another thread so a hang fails the test instead of stalling it
        let (tx, rx) = mpsc::channel();
        thread::spawn(move || {
            let start = Instant::now();
            let result = connection.with_timeout(TIMEOUT, |connection| {
                connection
                    .command(Command::ReadReg { address: 0 })?
                    .try_into()
            });
            // Closing the port must not wait for output the device never took
            drop(connection);
            tx.send((result, start.elapsed())).ok();
        });
        rx.recv_timeout(TIMEOUT * 10)
            .expect("command did not return within its timeout")
    }

    /// A ROM loader response frame for `ty` carrying `value`.
    fn response(ty: CommandType, value: u32) -> Vec<u8> {
        let mut frame = vec![0xc0, 0x01, ty as u8, 0x04, 0x00];
        frame.extend(value.to_le_bytes());
        frame.extend([0, 0, 0, 0, 0xc0]);
        frame
    }

    /// Writes `data` to the device port every `TIMEOUT / 4`, which keeps every
    /// single read within the serial timeout, for `TIMEOUT * 20`.
    fn stream(device: &mut Port, data: &[u8]) {
        device.read_exact(&mut [0; READ_REG_FRAME_LEN]).unwrap();
        let start = Instant::now();
        while start.elapsed() < TIMEOUT * 20 {
            device.write_all(data).unwrap();
            thread::sleep(TIMEOUT / 4);
        }
    }

    fn assert_timed_out((result, elapsed): (Result<u32, Error>, Duration)) {
        assert!(
            matches!(
                &result,
                Err(Error::Connection(err))
                    if matches!(err.downcast_ref(), Some(ConnectionError::Timeout(_)))
            ),
            "expected a timeout, got {result:?}"
        );
        assert!(elapsed < TIMEOUT * 2, "command took {elapsed:?}");
    }

    #[test]
    fn command_returns_matching_response() {
        let (result, _) = read_reg(|device| {
            device.read_exact(&mut [0; READ_REG_FRAME_LEN]).unwrap();
            device
                .write_all(&response(CommandType::ReadReg, 0x1234_5678))
                .unwrap();
        });
        assert_eq!(result.unwrap(), 0x1234_5678);
    }

    #[test]
    fn command_times_out_when_device_never_reads() {
        assert_timed_out(read_reg(|_| {}));
    }

    #[test]
    fn command_times_out_on_unrelated_output() {
        assert_timed_out(read_reg(|device| {
            stream(device, b"Restarting in 5 seconds...\n")
        }));
    }

    #[test]
    fn command_times_out_on_responses_to_other_commands() {
        assert_timed_out(read_reg(|device| {
            stream(device, &response(CommandType::Sync, 0))
        }));
    }
}

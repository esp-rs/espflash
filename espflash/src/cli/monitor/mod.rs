//! Serial monitor utility
//!
//! While simple, this serial monitor does provide some nice features such as:
//!
//! - Keyboard shortcut for resetting the device (Ctrl-R)
//! - Decoding of function addresses in serial output
//!
//! While some serial monitors buffer output until a newline is encountered,
//! that is not the case here. With other monitors the output of a `print!()`
//! call are not displayed until `println!()` is subsequently called, where as
//! in our monitor the output is displayed immediately upon reading.

use std::{
    io::{stdout, ErrorKind, Read, Write},
    time::Duration,
};

use crossterm::{
    event::{poll, read, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers},
    terminal::{disable_raw_mode, enable_raw_mode},
};
use external_processors::ExternalProcessors;
use log::{debug, error, warn};
use miette::{IntoDiagnostic, Result};
#[cfg(feature = "serialport")]
use serialport::SerialPort;
use strum::{Display, EnumIter, EnumString, VariantNames};

use crate::{
    cli::{
        monitor::{
            parser::{InputParser, ResolvingPrinter},
            symbols::Symbols,
        },
        MonitorConfigArgs,
    },
    connection::{reset::reset_after_flash, Port},
};

pub mod external_processors;
pub mod parser;

mod line_endings;
mod symbols;

#[cfg_attr(feature = "cli", derive(clap::ValueEnum))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Display, EnumIter, EnumString, VariantNames)]
#[non_exhaustive]
#[strum(serialize_all = "lowercase")]
pub enum LogFormat {
    /// defmt
    Defmt,
    /// serial
    Serial,
}

/// Type that ensures that raw mode is disabled when dropped.
struct RawModeGuard;

impl RawModeGuard {
    pub fn new() -> Result<Self> {
        enable_raw_mode().into_diagnostic()?;
        Ok(RawModeGuard)
    }
}

impl Drop for RawModeGuard {
    fn drop(&mut self) {
        if let Err(e) = disable_raw_mode() {
            error!("Failed to disable raw_mode: {:#}", e)
        }
    }
}

/// Open a serial monitor on the given serial port, using the given input
/// parser.
pub fn monitor(
    mut serial: Port,
    elf: Option<&[u8]>,
    pid: u16,
    monitor_args: MonitorConfigArgs,
) -> miette::Result<()> {
    if !monitor_args.non_interactive {
        println!("Commands:");
        println!("    CTRL+R    Reset chip");
        println!("    CTRL+C    Exit");
        println!();
    } else if !monitor_args.no_reset {
        reset_after_flash(&mut serial, pid).into_diagnostic()?;
    }

    let baud = monitor_args.monitor_baud;
    debug!("Opening serial monitor with baudrate: {}", baud);

    // Explicitly set the baud rate when starting the serial monitor, to allow using
    // different rates for flashing.
    serial.set_baud_rate(baud).into_diagnostic()?;
    serial
        .set_timeout(Duration::from_millis(5))
        .into_diagnostic()?;

    // We are in raw mode until `_raw_mode` is dropped (ie. this function returns).
    let _raw_mode = RawModeGuard::new();

    let stdout = stdout();
    let mut stdout = ResolvingPrinter::new(elf, stdout.lock());

    let mut parser: Box<dyn InputParser> = match monitor_args
        .log_format
        .unwrap_or_else(|| deduce_log_format(elf))
    {
        LogFormat::Defmt => Box::new(parser::esp_defmt::EspDefmt::new(elf)?),
        LogFormat::Serial => Box::new(parser::serial::Serial),
    };

    let mut external_processors =
        ExternalProcessors::new(monitor_args.processors, monitor_args.elf)?;

    let mut buff = [0; 1024];
    loop {
        let read_count = match serial.read(&mut buff) {
            Ok(count) => Ok(count),
            Err(e) if e.kind() == ErrorKind::TimedOut => Ok(0),
            Err(e) if e.kind() == ErrorKind::Interrupted => continue,
            err => err.into_diagnostic(),
        }?;

        let processed = external_processors.process(&buff[0..read_count]);
        parser.feed(&processed, &mut stdout);

        // Don't forget to flush the writer!
        stdout.flush().ok();

        if !monitor_args.non_interactive && poll(Duration::from_secs(0)).into_diagnostic()? {
            if let Event::Key(key) = read().into_diagnostic()? {
                if key.kind == KeyEventKind::Press {
                    if key.modifiers.contains(KeyModifiers::CONTROL) {
                        match key.code {
                            KeyCode::Char('c') => break,
                            KeyCode::Char('r') => {
                                reset_after_flash(&mut serial, pid).into_diagnostic()?;
                                continue;
                            }
                            _ => {}
                        }
                    }

                    if let Some(bytes) = handle_key_event(key) {
                        serial.write_all(&bytes).into_diagnostic()?;
                        serial.flush().into_diagnostic()?;
                    }
                }
            }
        }
    }

    Ok(())
}

fn deduce_log_format(elf: Option<&[u8]>) -> LogFormat {
    let Some(elf) = elf else {
        return LogFormat::Serial;
    };

    let Ok(symbols) = Symbols::try_from(elf) else {
        return LogFormat::Serial;
    };

    let Some(format_symbol) =
        symbols.get_symbol_data(Some(".espressif.metadata"), b"espflash.LOG_FORMAT")
    else {
        return LogFormat::Serial;
    };

    match format_symbol {
        b"defmt-espflash" => LogFormat::Defmt,
        b"serial" => LogFormat::Serial,
        other => {
            warn!(
                "Unknown log format symbol: {}. Defaulting to serial.",
                String::from_utf8_lossy(other),
            );
            LogFormat::Serial
        }
    }
}

// Converts key events from crossterm into appropriate character/escape
// sequences which are then sent over the serial connection.
//
// Adapted from: https://github.com/dhylands/serial-monitor
fn handle_key_event(key_event: KeyEvent) -> Option<Vec<u8>> {
    // The following escape sequences come from the MicroPython codebase.
    //
    //  Up      ESC [A
    //  Down    ESC [B
    //  Right   ESC [C
    //  Left    ESC [D
    //  Home    ESC [H  or ESC [1~
    //  End     ESC [F  or ESC [4~
    //  Del     ESC [3~
    //  Insert  ESC [2~

    let mut buf = [0; 4];

    let key_str: Option<&[u8]> = match key_event.code {
        KeyCode::Backspace => Some(b"\x08"),
        KeyCode::Enter => Some(b"\r"),
        KeyCode::Left => Some(b"\x1b[D"),
        KeyCode::Right => Some(b"\x1b[C"),
        KeyCode::Home => Some(b"\x1b[H"),
        KeyCode::End => Some(b"\x1b[F"),
        KeyCode::Up => Some(b"\x1b[A"),
        KeyCode::Down => Some(b"\x1b[B"),
        KeyCode::Tab => Some(b"\x09"),
        KeyCode::Delete => Some(b"\x1b[3~"),
        KeyCode::Insert => Some(b"\x1b[2~"),
        KeyCode::Esc => Some(b"\x1b"),
        KeyCode::Char(ch) => {
            if key_event.modifiers & KeyModifiers::CONTROL == KeyModifiers::CONTROL {
                buf[0] = ch as u8;

                if ch.is_ascii_lowercase() || (ch == ' ') {
                    buf[0] &= 0x1f;
                    Some(&buf[0..1])
                } else if ('4'..='7').contains(&ch) {
                    // crossterm returns Control-4 thru 7 for \x1c thru \x1f
                    buf[0] = (buf[0] + 8) & 0x1f;
                    Some(&buf[0..1])
                } else {
                    Some(ch.encode_utf8(&mut buf).as_bytes())
                }
            } else {
                Some(ch.encode_utf8(&mut buf).as_bytes())
            }
        }
        _ => None,
    };

    key_str.map(|slice| slice.into())
}

use std::{
    io::{stdout, ErrorKind, Read, Write},
    thread::sleep,
    time::Duration,
};

use crossterm::{
    event::{poll, read, Event, KeyCode, KeyEvent, KeyModifiers},
    terminal::{disable_raw_mode, enable_raw_mode},
};
use miette::{IntoDiagnostic, Result};
use serialport::SerialPort;

use super::line_endings::normalized;

/// Converts key events from crossterm into appropriate character/escape
/// sequences which are then sent over the serial connection.
///
/// Adapted from https://github.com/dhylands/serial-monitor
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
                if ('a'..='z').contains(&ch) || (ch == ' ') {
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
            eprintln!("{:#}", e)
        }
    }
}

pub fn monitor(mut serial: Box<dyn SerialPort>) -> serialport::Result<()> {
    println!("Commands:");
    println!("    CTRL+R    Reset chip");
    println!("    CTRL+C    Exit");
    println!();

    // Explicitly set the baud rate when starting the serial monitor, to allow using
    // different rates for flashing.
    serial.set_baud_rate(115_200)?;
    serial.set_timeout(Duration::from_millis(5))?;

    let _raw_mode = RawModeGuard::new();

    let stdout = stdout();
    let mut stdout = stdout.lock();

    let mut buff = [0; 128];
    loop {
        let read_count = match serial.read(&mut buff) {
            Ok(count) => Ok(count),
            Err(e) if e.kind() == ErrorKind::TimedOut => Ok(0),
            Err(e) if e.kind() == ErrorKind::Interrupted => continue,
            err => err,
        }?;

        if read_count > 0 {
            let data: Vec<u8> = normalized(buff[0..read_count].iter().copied()).collect();
            let data = String::from_utf8_lossy(&data);

            stdout.write_all(data.as_bytes()).ok();
            stdout.flush()?;
        }

        if poll(Duration::from_secs(0))? {
            if let Event::Key(key) = read()? {
                if key.modifiers.contains(KeyModifiers::CONTROL) {
                    match key.code {
                        KeyCode::Char('c') => break,
                        KeyCode::Char('r') => {
                            serial.write_data_terminal_ready(false)?;
                            serial.write_request_to_send(true)?;

                            sleep(Duration::from_millis(100));

                            serial.write_request_to_send(false)?;
                            continue;
                        }
                        _ => {}
                    }
                }

                if let Some(bytes) = handle_key_event(key) {
                    serial.write_all(&bytes)?;
                    serial.flush()?;
                }
            }
        }
    }

    Ok(())
}

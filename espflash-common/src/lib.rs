use crate::clap::ConnectArgs;
use crate::config::Config;




pub mod clap;
pub mod config;
mod line_endings;
pub mod monitor;

pub fn get_serial_port(matches: &ConnectArgs, config: &Config) -> Option<String> {
    // The serial port must be specified, either as a command-line argument or in
    // the cargo configuration file. In the case that both have been provided the
    // command-line argument will take precedence.
    if let Some(serial) = &matches.serial {
        Some(serial.to_string())
    } else if let Some(serial) = &config.connection.serial {
        Some(serial.into())
    } else {
        None
    }
}

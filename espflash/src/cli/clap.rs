use std::path::PathBuf;

use clap::Parser;

#[derive(Parser)]
pub struct ConnectOpts {
    /// Serial port connected to target device
    pub serial: Option<String>,
    /// Baud rate at which to flash target device
    #[clap(long)]
    pub speed: Option<u32>,
}

#[derive(Parser)]
pub struct FlashOpts {
    /// Load the application to RAM instead of Flash
    #[clap(long)]
    pub ram: bool,
    /// Path to a binary (.bin) bootloader file
    #[clap(long)]
    pub bootloader: Option<PathBuf>,
    /// Path to a CSV file containing partition table
    #[clap(long)]
    pub partition_table: Option<PathBuf>,
    /// Open a serial monitor after flashing
    #[clap(long)]
    pub monitor: bool,
    /// Encrypt the flash contents
    #[clap(long, conflicts_with = "ram")]
    pub encrypt: bool,
    /// Encryption key to encrypt the flash contents with
    #[clap(long, requires = "encrypt")]
    pub encryption_key: Option<String>,
}

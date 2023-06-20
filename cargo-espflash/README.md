# cargo-espflash

[![Crates.io](https://img.shields.io/crates/v/cargo-espflash?labelColor=1C2C2E&color=C96329&logo=Rust&style=flat-square)](https://crates.io/crates/cargo-espflash)
![MSRV](https://img.shields.io/badge/MSRV-1.65-blue?labelColor=1C2C2E&logo=Rust&style=flat-square)
![Crates.io](https://img.shields.io/crates/l/cargo-espflash?labelColor=1C2C2E&style=flat-square)

Cross-compiler and Cargo extension for flashing Espressif devices.

Supports the **ESP32**, **ESP32-C2/C3/C6**, **ESP32-H2**, **ESP32-S2/S3**, and **ESP8266**.

## Table of Contents

- [Installation](#installation)
- [Usage](#usage)
  - [Permissions on Linux](#permissions-on-linux)
  - [Windows Subsystem for Linux](#windows-subsystem-for-linux)
- [Bootloader and Partition Table](#bootloader-and-partition-table)
- [Package Metadata](#package-metadata)
- [Configuration File](#configuration-file)

## Installation

If you are installing `cargo-espflash` from source (ie. using `cargo install`) then you must have `rustc>=1.65.0` installed on your system.

If you are running **macOS** or **Linux** then [libuv] must also be installed; this is available via most popular package managers. If you are running **Windows** you can ignore this step.

```bash
# macOS
brew install libuv
# Debian/Ubuntu/etc.
apt-get install libuv-dev
# Fedora
dnf install systemd-devel
```

To install:

```bash
cargo install cargo-espflash
```

Alternatively, you can use [cargo-binstall] to download pre-compiled artifacts from the [releases] and use them instead:

```bash
cargo binstall cargo-espflash
```

If you would like to flash from a Raspberry Pi using the built-in UART peripheral, you can enable the `raspberry` feature (note that this is not available if using [cargo-binstall]):

```bash
cargo install cargo-espflash --features=raspberry
```

[libuv]: (https://libuv.org/)
[cargo-binstall]: (https://github.com/cargo-bins/cargo-binstall)
[releases]: https://github.com/esp-rs/espflash/releases

## Usage

```text
Cargo subcommand for flashing Espressif devices

Usage: cargo espflash <COMMAND>

Commands:
  board-info       Establish a connection with a target device
  completions      Generate completions for the given shell
  flash            Build and flash an application to a target device
  monitor          Open the serial monitor without flashing
  partition-table  Operations for partitions tables
  save-image       Save the image to disk instead of flashing to device
  help             Print this message or the help of the given subcommand(s)

Options:
  -h, --help     Print help
  -V, --version  Print version
```

### Permissions on Linux

In Linux, when using any of the commands that requires using a serial port, the current user may not have access to serial ports and a "Permission Denied" or "Port doesn’t exist" errors may appear.

On most Linux distributions, the solution is to add the user to the `dialout` group (check e.g. `ls -l /dev/ttyUSB0` to find the group) with a command like `sudo usermod -a -G dialout $USER`. You can call `su - $USER` to enable read and write permissions for the serial port without having to log out and back in again.

Check your Linux distribution’s documentation for more information.

### Windows Subsystem for Linux

It is _not_ currently possible to use `cargo-espflash` from within WSL1. There are no plans to add support for WSL1 at this time.

It is also _not_ possible to flash chips using the built-in `USB_SERIAL_JTAG` peripheral when using WSL2, because resetting also resets `USB_SERIAL_JTAG` peripheral, which then disconnects the chip from WSL2. Chips _can_ be flashed via UART using WSL2, however.

## Bootloader and Partition Table

`cargo-espflash` is able to detect if the package being built and flashed depends on [esp-idf-sys]; if it does, then the bootloader and partition table built by the `esp-idf-sys` build script will be used, otherwise the bundled bootloader and partition tables will be used instead.

If the `--bootloader` and/or `--partition-table` options are provided then these will be used regardless of whether or not the package depends on `esp-idf-sys`.

[esp-idf-sys]: https://github.com/esp-rs/esp-idf-sys

## Package Metadata

You're able to specify paths to bootloader and partition table files ands image format in your package's Cargo metadata for per-project configuration:

```toml
[package.metadata.espflash]
bootloader      = "bootloader.bin" # Must be a binary file
partition_table = "partitions.csv" # Supports CSV and binary formats
format          = "direct-boot"    # Can be 'esp-bootloader' or 'direct-boot'
```

## Configuration File

It's possible to specify a serial port and/or USB VID/PID values by setting them in a configuration file. The location of this file differs based on your operating system:

| Operating System | Configuration Path                                                |
| :--------------- | :---------------------------------------------------------------- |
| Linux            | `$HOME/.config/espflash/espflash.toml`                            |
| macOS            | `$HOME/Library/Application Support/rs.esp.espflash/espflash.toml` |
| Windows          | `%APPDATA%\esp\espflash\espflash.toml`                            |

### Configuration examples

You can either configure the serial port name like so:

```
[connection]
serial = "/dev/ttyUSB0"
```

Or specify one or more USB `vid`/`pid` couple:

```
[[usb_device]]
vid = "303a"
pid = "1001"
```

## License

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](../LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
- MIT license ([LICENSE-MIT](../LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in
the work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without
any additional terms or conditions.

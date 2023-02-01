# espflash

[![Crates.io](https://img.shields.io/crates/v/espflash?labelColor=1C2C2E&color=C96329&logo=Rust&style=flat-square)](https://crates.io/crates/espflash)
[![docs.rs](https://img.shields.io/docsrs/espflash?labelColor=1C2C2E&color=C96329&logo=rust&style=flat-square)](https://docs.rs/espflash)
![MSRV](https://img.shields.io/badge/MSRV-1.66-blue?labelColor=1C2C2E&logo=Rust&style=flat-square)
![Crates.io](https://img.shields.io/crates/l/espflash?labelColor=1C2C2E&style=flat-square)

A library and command-line tool for flashing Espressif devices over serial.

Supports the **ESP32**, **ESP32-C2/C3/C6**, **ESP32-S2/S3**, and **ESP8266**.

## Installation

If you are installing `espflash` from source (ie. using `cargo install`) then you must have `rustc>=1.66.0` installed on your system.

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
cargo install espflash
```

Alternatively, you can use [cargo-binstall] to download pre-compiled artifacts from the [releases] and use them instead:

```bash
cargo binstall espflash
```

If you would like to flash from a Raspberry Pi using the built-in UART peripheral, you can enable the `raspberry` feature (note that this is not available if using [cargo-binstall]):

```bash
cargo install espflash --features=raspberry
```

[libuv]: (https://libuv.org/)
[cargo-binstall]: (https://github.com/cargo-bins/cargo-binstall)
[releases]: https://github.com/esp-rs/espflash/releases

## Usage

```text
A command-line tool for flashing Espressif devices over serial

Usage: espflash <COMMAND>

Commands:
  board-info       Display information about the connected board and exit without flashing
  flash            Flash an application to a target device
  monitor          Open the serial monitor without flashing
  partition-table  Operations for partitions tables
  save-image       Save the image to disk instead of flashing to device
  write-bin        Writes a binary file to a specific address in the chip's flash
  help             Print this message or the help of the given subcommand(s)

Options:
  -h, --help     Print help information
  -V, --version  Print version information
```

### Cargo Runner

You can also use `espflash` as a Cargo runner by adding the following to your project's `.cargo/config.toml` file, for example:

```toml
[target.'cfg(any(target_arch = "riscv32", target_arch = "xtensa"))']
runner = "espflash flash --baud=921600 --monitor /dev/ttyUSB0"
```

With this configuration you can flash and monitor you application using `cargo run`.

## Configuration

It's possible to specify a serial port and/or USB VID/PID values by setting them in a configuration file. The location of this file differs based on your operating system:

| Operating System | Configuration Path                                                |
| :--------------- | :---------------------------------------------------------------- |
| Linux            | `$HOME/.config/espflash/espflash.toml`                            |
| macOS            | `$HOME/Library/Application Support/rs.esp.espflash/espflash.toml` |
| Windows          | `%APPDATA%\esp\espflash\espflash.toml`                            |

## Windows Subsystem for Linux

It is not currently possible to use `espflash` from within WSL1.

It is not possible to flash chips using the built-in `USB_SERIAL_JTAG` when using WSL2, because the reset also resets `USB_SERIAL_JTAG` peripheral which then disconnects the chip from WSL2. Chips _can_ be flashed via UART using WSL2, however.

## License

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](../LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
- MIT license ([LICENSE-MIT](../LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in
the work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without
any additional terms or conditions.

# cargo-espflash

[![Crates.io](https://img.shields.io/crates/v/cargo-espflash?labelColor=1C2C2E&color=C96329&logo=Rust&style=flat-square)](https://crates.io/crates/cargo-espflash)
![MSRV](https://img.shields.io/badge/MSRV-1.60-blue?labelColor=1C2C2E&logo=Rust&style=flat-square)
![Crates.io](https://img.shields.io/crates/l/cargo-espflash?labelColor=1C2C2E&style=flat-square)

Cross-compiler and Cargo extension for flashing Espressif devices over serial.

Supports the **ESP32**, **ESP32-C2**, **ESP32-C3**, **ESP32-S2**, **ESP32-S3**, and **ESP8266**.

## Installation

If you are installing `cargo-espflash` from source (ie. using `cargo install`) then you must have `rustc>=1.60.0` installed on your system. Additionally [libuv] must be installed; this is available via most popular package managers.

```bash
$ # macOS
$ brew install libuv
$ # Debian/Ubuntu/etc.
$ apt-get install libuv-dev
$ # Fedora
$ dnf install systemd-devel
```

To install:

```bash
$ cargo install cargo-espflash
```

Alternatively, you can use [cargo-binstall] to download pre-compiled artifacts from the [releases] and use them instead:

```bash
$ cargo binstall cargo-espflash
```

If you would like to flash from a Raspberry Pi using the built-in UART peripheral, you can enable the `raspberry` feature (note that this is not available if using [cargo-binstall]):

```bash
$ cargo install cargo-espflash --features=raspberry
```

[libuv]: (https://libuv.org/)
[cargo-binstall]: (https://github.com/cargo-bins/cargo-binstall)
[releases]: https://github.com/esp-rs/espflash/releases

## Usage

```text
A cargo extension for flashing Espressif devices

Usage: cargo espflash <COMMAND>

Commands:
  board-info       Display information about the connected board and exit without flashing
  flash            Flash an application to a target device
  monitor          Open the serial monitor without flashing
  partition-table  Operations for partitions tables
  save-image       Save the image to disk instead of flashing to device
  help             Print this message or the help of the given subcommand(s)

Options:
  -h, --help     Print help information
  -V, --version  Print version information
```

## Package Metadata

You're able to specify paths to bootloader and partition table files ands image format in your package's Cargo metadata for per-project configuration:

```toml
[package.metadata.espflash]
bootloader      = "bootloader.bin" # Must be a binary file
partition_table = "partitions.csv" # Supports CSV and binary formats
format          = "direct-boot"    # Can be 'esp-bootloader' or 'direct-boot'
```

## Configuration

It's possible to specify a serial port and/or USB VID/PID values by setting them in a configuration file. The location of this file differs based on your operating system:

| Operating System | Configuration Path                                                |
| :--------------- | :---------------------------------------------------------------- |
| Linux            | `$HOME/.config/espflash/espflash.toml`                            |
| macOS            | `$HOME/Library/Application Support/rs.esp.espflash/espflash.toml` |
| Windows          | `%APPDATA%\esp\espflash\espflash.toml`                            |

## WSL2

It is not possible to flash chips using the built-in `USB_SERIAL_JTAG` when using WSL2, because the reset also resets `USB_SERIAL_JTAG` peripheral which then disconnects the chip from WSL2.

## License

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](../LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
- MIT license ([LICENSE-MIT](../LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in
the work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without
any additional terms or conditions.

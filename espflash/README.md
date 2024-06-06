<!-- omit in toc -->
# espflash

[![Crates.io](https://img.shields.io/crates/v/espflash?labelColor=1C2C2E&color=C96329&logo=Rust&style=flat-square)](https://crates.io/crates/espflash)
[![docs.rs](https://img.shields.io/docsrs/espflash?labelColor=1C2C2E&color=C96329&logo=rust&style=flat-square)](https://docs.rs/espflash)
![MSRV](https://img.shields.io/badge/MSRV-1.74-blue?labelColor=1C2C2E&logo=Rust&style=flat-square)
![Crates.io](https://img.shields.io/crates/l/espflash?labelColor=1C2C2E&style=flat-square)

A library and command-line tool for flashing Espressif devices.

Supports the **ESP32**, **ESP32-C2/C3/C6**, **ESP32-H2**, **ESP32-P4**, and **ESP32-S2/S3**.

<!-- omit in toc -->
## Table of Contents

- [Installation](#installation)
- [Usage](#usage)
  - [Permissions on Linux](#permissions-on-linux)
  - [Windows Subsystem for Linux](#windows-subsystem-for-linux)
  - [Cargo Runner](#cargo-runner)
- [Using `espflash` as a Library](#using-espflash-as-a-library)
- [Configuration File](#configuration-file)
  - [Configuration precedence](#configuration-precedence)
- [Logging Format](#logging-format)
- [License](#license)
  - [Contribution](#contribution)

## Installation

If you are installing `espflash` from source (ie. using `cargo install`) then you must have `rustc>=1.76.0` installed on your system.

If you are running **Linux** then [libudev] must also be installed; this is available via most popular package managers. If you are running **Windows** or **macOS** you can ignore this step.

```bash
# Debian/Ubuntu/etc.
apt-get install libudev-dev
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

[libudev]: https://www.freedesktop.org/software/systemd/man/latest/libudev.html
[cargo-binstall]: https://github.com/cargo-bins/cargo-binstall
[releases]: https://github.com/esp-rs/espflash/releases

## Usage

```text
A command-line tool for flashing Espressif devices

Usage: espflash <COMMAND>

Commands:
  board-info       Print information about a connected target device
  completions      Generate completions for the given shell
  erase-flash      Erase Flash entirely
  erase-parts      Erase specified partitions
  erase-region     Erase specified region
  flash            Flash an application in ELF format to a connected target device
  hold-in-reset    Hold the target device in reset
  monitor          Open the serial monitor without flashing the connected target device
  partition-table  Convert partition tables between CSV and binary format
  read-flash       Read SPI flash content
  reset            Reset the target device
  save-image       Generate a binary application image and save it to a local disk
  write-bin        Write a binary file to a specific address in a target device's flash
  checksum-md5     Calculate the MD5 checksum of the given region
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

It is _not_ currently possible to use `espflash` from within WSL1. There are no plans to add support for WSL1 at this time.

It is also _not_ possible to flash chips using the built-in `USB_SERIAL_JTAG` peripheral when using WSL2, because resetting also resets `USB_SERIAL_JTAG` peripheral, which then disconnects the chip from WSL2. Chips _can_ be flashed via UART using WSL2, however.

### Cargo Runner

You can also use `espflash` as a Cargo runner by adding the following to your project's `.cargo/config.toml` file, for example:

```toml
[target.'cfg(any(target_arch = "riscv32", target_arch = "xtensa"))']
runner = "espflash flash --baud=921600 --monitor /dev/ttyUSB0"
```

With this configuration you can flash and monitor you application using `cargo run`.

## Using `espflash` as a Library

`espflash` can be used as a library in other applications:

```toml
espflash = { version = "2.1", default-features = false }
```

or `cargo add espflash --no-default-features`

> **Warning**
> Note that the `cli` module does not provide SemVer guarantees.

We disable the `default-features` to opt-out the `cli` feature, which is enabled by default; you likely will not need any of these types or functions in your application so there’s no use pulling in the extra dependencies.

## Configuration File

The configuration file allows you to define various parameters for your application:

- Serial port:
  - By name:
    ```toml
    [connection]
    serial = "/dev/ttyUSB0"
    ```
  - By USB VID/PID values:
    ```toml
    [[usb_device]]
    vid = "303a"
    pid = "1001"
    ```
- Baudrate:
  ```toml
  baudrate = 460800
  ```
- Bootloader:
  ```toml
  bootloader = "path/to/custom/bootloader.bin"
  ```
- Partition table
  ```toml
  partition_table = "path/to/custom/partition-table.bin"
  ```
- Flash settings
  ```toml
  [flash]
  mode = "qio"
  size = "8MB"
  frequency = "80MHz"
  ```

You can have a local and/or a global configuration file:

- For local configurations, store the file under the current working directory or in the parent directory (to support Cargo workspaces) with the name `espflash.toml`
- Global file location differs based on your operating system:
  - Linux: `$HOME/.config/espflash/espflash.toml`
  - macOS: `$HOME/Library/Application Support/rs.esp.espflash/espflash.toml`
  - Windows: `%APPDATA%\esp\espflash\espflash.toml`

### Configuration precedence

1. Environment variables: If `ESPFLASH_PORT` or `ESPFLASH_BAUD` are set, the will be used instead of the config file value.
2. Local configuration file
3. Global configuration file

## Logging Format

`espflash` `flash` and `monitor` subcommands support several logging formats using the `-L/--log-format` argument:

- `serial`: Default logging format
- `defmt`: Uses [`defmt`] logging framework. With logging format, logging strings have framing bytes to indicate that they are `defmt` messages.
  - See [`defmt` section] of `esp-println` readme.
  - For a detailed guide on how to use `defmt` in the `no_std` ecosystem, see [`defmt` project] of Embedded Rust (no_std) on Espressif book.

[`defmt` section]: https://github.com/esp-rs/esp-println?tab=readme-ov-file#defmt
[`defmt` project]: https://esp-rs.github.io/no_std-training/03_6_defmt.html

## License

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](../LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
- MIT license ([LICENSE-MIT](../LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in
the work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without
any additional terms or conditions.

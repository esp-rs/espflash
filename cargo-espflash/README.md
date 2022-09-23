# cargo-espflash

Cross-compiler and serial flasher cargo subcommand for Espressif SoCs and modules.

Currently supports the **ESP32**, **ESP32-C3**, **ESP32-S2**, **ESP32-S3**, and **ESP8266**.

Prior to flashing, the project is built using the `build-std` unstable Cargo feature. Please refer to the [cargo documentation] for more information.

[cargo documentation]: https://doc.rust-lang.org/cargo/reference/unstable.html#build-std

## Installation

```shell
$ cargo install cargo-espflash
```

Alternatively, you can use [cargo-binstall] to install pre-compiled binaries on any supported system. Please check the [releases] to see which architectures and operating systems have pre-compiled binaries.

```shell
$ cargo install cargo-binstall
$ cargo binstall cargo-espflash
```

[cargo-binstall]: https://github.com/ryankurte/cargo-binstall
[releases]: https://github.com/esp-rs/espflash/releases

## Usage

```text
cargo-espflash 2.0.0-dev
Cargo subcommand for flashing Espressif devices over serial

USAGE:
    cargo espflash <SUBCOMMAND>

OPTIONS:
    -h, --help       Print help information
    -V, --version    Print version information

SUBCOMMANDS:
    board-info         Display information about the connected board and exit without flashing
    flash              Flash an application to a target device
    help               Print this message or the help of the given subcommand(s)
    monitor            Open the serial monitor without flashing
    partition-table    Operations for partitions tables
    save-image         Save the image to disk instead of flashing to device
```

## Configuration

You can also specify the serial port and/or expected VID/PID values by setting them in the configuration file. This file is in different locations depending on your operating system:

| Operating System | Configuration Path                                                       |
| :--------------- | :----------------------------------------------------------------------- |
| **Linux:**       | `/home/alice/.config/espflash/espflash.toml`                             |
| **Windows:**     | `C:\Users\Alice\AppData\Roaming\esp\espflash\espflash.toml`              |
| **macOS:**       | `/Users/Alice/Library/Application Support/rs.esp.espflash/espflash.toml` |

An example configuration file may look as follows (note that TOML does _not_ support hexadecimal literals):

```toml
[connection]
serial = "/dev/ttyUSB0"

[[usb_device]]
vid = "303A"
pid = "8000"
```

## WSL2

It is not possible to flash `usb-serial-jtag` chips with `WSL2` because the reset also resets `serial-jtag-peripheral` which disconnects the chip from WSL2.

## Package Metadata

You can specify the bootloader, partition table, or image format for a project in the package metadata in `Cargo.toml`:

```toml
[package.metadata.espflash]
partition_table = "partitions.csv"
bootloader = "bootloader.bin"
format = "direct-boot"
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

# espflash

Serial flasher utility for Espressif SoCs and modules.

Currently supports the **ESP32**, **ESP32-C3**, **ESP32-S2**, **ESP32-S3**, and **ESP8266**.

## Installation

```shell
$ cargo install espflash
```

Alternatively, you can use [cargo-binstall] to install pre-compiled binaries on any supported system. Please check the [releases] to see which architectures and operating systems have pre-compiled binaries.

```shell
$ cargo install cargo-binstall
$ cargo binstall espflash
```

[cargo-binstall]: https://github.com/ryankurte/cargo-binstall
[releases]: https://github.com/esp-rs/espflash/releases

## Usage

```text
espflash 2.0.0-dev
A command-line tool for flashing Espressif devices over serial

USAGE:
    espflash <SUBCOMMAND>

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
    write-bin          Writes a binary file to a specific address in the chip's flash
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

## Use as a Cargo Runner

You can also use `espflash` as a Cargo runner by adding the followin to your project's `.cargo/config` file:

```
[target.'cfg(all(target_arch = "xtensa", target_os = "none"))']
runner = "espflash --ram /dev/ttyUSB0"
```

This then allows you to run your project using `cargo run`.

## License

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](../LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
- MIT license ([LICENSE-MIT](../LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in
the work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without
any additional terms or conditions.

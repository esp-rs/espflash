# espflash

[![Actions Status](https://github.com/esp-rs/espflash/workflows/CI/badge.svg)](https://github.com/esp-rs/espflash/actions?query=workflow%3A"CI")
![Crates.io](https://img.shields.io/crates/l/espflash)

Serial flasher utility for Espressif SoCs and modules based on [esptool.py].

Currently supports the **ESP32**, **ESP32-C3**, **ESP32-S2**, **ESP32-S3**, and **ESP8266**.

### cargo-espflash

[cargo-espflash] is a subcommand for Cargo which utilizes the [espflash] library. This tool integrates with your Cargo projects and handles compilation, flashing, and monitoring of target devices.

Please see the [cargo-espflash README] for more information.

#### Example

```shell
$ cargo espflash --release --example=blinky /dev/ttyUSB0
```

[cargo-espflash readme]: https://github.com/esp-rs/espflash/blob/master/cargo-espflash/README.md

### espflash

[espflash] is a standalone binary and library contained within the same crate. This tool does not integrate with Cargo, but supports all of the same features as [cargo-espflash] which are not related to compilation.

Please see the [espflash README] for more information.

#### Example

```shell
$ espflash /dev/ttyUSB0 target/xtensa-esp32-none-elf/release/examples/blinky
```

[espflash readme]: https://github.com/esp-rs/espflash/blob/master/espflash/README.md
[esptool.py]: https://github.com/espressif/esptool
[cargo-espflash]: https://github.com/esp-rs/espflash/tree/master/cargo-espflash
[espflash]: https://github.com/esp-rs/espflash/tree/master/espflash

## Installation

Either application can be installed using `cargo` as you normally would:

```shell
$ cargo install cargo-espflash
$ cargo install espflash
```

Alternatively, you can use [cargo-binstall] to install pre-compiled binaries on any supported system. Please check the [releases] to see which architectures and operating systems have pre-compiled binaries.

```shell
$ cargo binstall cargo-espflash
$ cargo binstall espflash
```

[cargo-binstall]: https://github.com/ryankurte/cargo-binstall
[releases]: https://github.com/esp-rs/espflash/releases

## Notes on Building

Requires `rustc >= 1.59.0` in order to build either application from source. In addition to the Rust toolchain [libuv](https://libuv.org/) must also be present on your system; this can be installed via most popular package managers.

#### macOS

```bash
$ brew install libuv
```

#### Ubuntu

```bash
$ sudo apt-get install libuv-dev
```

#### Fedora

```bash
$ dnf install systemd-devel
```

## License

Licensed under the GNU General Public License Version 2. See [LICENSE](./espflash/LICENSE) for more details.

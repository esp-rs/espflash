# espflash

[![Actions Status](https://github.com/esp-rs/espflash/workflows/CI/badge.svg)](https://github.com/esp-rs/espflash/actions?query=workflow%3A"CI")
![Crates.io](https://img.shields.io/crates/l/espflash)

Serial flasher utility for Espressif SoCs and modules based on [esptool.py].

Currently supports the **ESP32**, **ESP32-C3**, **ESP32-S2**, **ESP32-S3**, and **ESP8266**.

This repository contains two applications:

| Application      | Description                                                 |
| :--------------- | :---------------------------------------------------------- |
| [cargo-espflash] | Cargo subcommand for espflash                               |
| [espflash]       | Library and `espflash` binary (_without_ Cargo integration) |

> **NOTE:** requires `rustc >= 1.56.0` in order to build either application

## Installation

```shell
$ cargo install cargo-espflash
$ cargo install espflash
```

## cargo-espflash

[cargo-espflash] is a subcommand for Cargo which utilizes the [espflash] library. This tool integrates with your Cargo projects and handles compilation, flashing, and monitoring for target devices.

Please see the [cargo-espflash README] for more information.

### Example

```shell
$ cargo espflash --release --example=blinky /dev/ttyUSB0
```

## espflash

[espflash] is a standalone binary and library contained within the same crate. This tool does not integrate with Cargo, but supports all of the same features as [cargo-espflash] which are not related to compilation.

Please see the [espflash README] for more information.

### Example

```shell
$ espflash /dev/ttyUSB0 target/xtensa-esp32-none-elf/release/examples/blinky
```

## Quickstart - Docker

The `esprs/espflash` Docker image contains all necessary toolchains and tooling (including espflash) to build an application and flash it to a target device.

To clone, build and flash the [esp32-hal] examples run the following:

```shell
$ git clone https://github.com/esp-rs/esp32-hal
$ cd esp32-hal
$ docker run -v "$(pwd):/espflash" --device=/dev/ttyUSB0 -ti esprs/espflash --release --example=blinky /dev/ttyUSB0
```

### Custom Docker Build

```shell
$ git clone --depth 1 https://github.com/esp-rs/espflash.git
$ cd espflash
$ docker build -t esprs/espflash .
```

## License

Licensed under the GNU General Public License Version 2. See [LICENSE](./espflash/LICENSE) for more details.

[esptool.py]: https://github.com/espressif/esptool
[cargo-espflash]: https://github.com/esp-rs/espflash/tree/master/cargo-espflash
[espflash]: https://github.com/esp-rs/espflash/tree/master/espflash
[cargo-espflash readme]: https://github.com/esp-rs/espflash/blob/master/cargo-espflash/README.md
[espflash readme]: https://github.com/esp-rs/espflash/blob/master/espflash/README.md
[esp32-hal]: https://github.com/esp-rs/esp32-hal

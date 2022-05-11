# espflash

Serial flasher utility for Espressif SoCs and modules.

Currently supports the **ESP32**, **ESP32-C3**, **ESP32-S2**, **ESP32-S3**, and **ESP8266**.

[![asciicast](https://asciinema.org/a/UxRaCy4pretvGkghrRO0Qvypm.svg)](https://asciinema.org/a/UxRaCy4pretvGkghrRO0Qvypm)

## Installation

```shell
$ cargo install espflash
```

## Usage

```text
espflash 1.5.1

USAGE:
    espflash [OPTIONS] [ARGS] [SUBCOMMAND]

ARGS:
    <SERIAL>    Serial port connected to target device
    <IMAGE>     ELF image to flash

OPTIONS:
        --bootloader <BOOTLOADER>
            Path to a binary (.bin) bootloader file

    -f, --flash-freq <FREQUENCY>
            Flash frequency [possible values: 20M, 26M, 40M, 80M]

        --format <FORMAT>
            Image format to flash [possible values: bootloader, direct-boot]

    -h, --help
            Print help information

    -m, --flash-mode <MODE>
            Flash mode to use [possible values: QIO, QOUT, DIO, DOUT]

        --monitor
            Open a serial monitor after flashing

        --partition-table <PARTITION_TABLE>
            Path to a CSV file containing partition table

        --ram
            Load the application to RAM instead of Flash

    -s, --flash-size <SIZE>
            Flash size of the target [possible values: 256KB, 512KB, 1MB, 2MB, 4MB, 8MB, 16MB, 32MB,
            64MB, 128MB]

        --speed <SPEED>
            Baud rate at which to flash target device

    -V, --version
            Print version information

SUBCOMMANDS:
    board-info         Display information about the connected board and exit without flashing
    help               Print this message or the help of the given subcommand(s)
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
vid = 12346 # 0x303A
pid = 32768 # 0x8000
```

## Use as a Cargo Runner

You can also use `espflash` as a Cargo runner by adding the followin to your project's `.cargo/config` file:

```
[target.'cfg(all(target_arch = "xtensa", target_os = "none"))']
runner = "espflash --ram /dev/ttyUSB0"
```

This then allows you to run your project using `cargo run`.

## License

Licensed under the GNU General Public License Version 2. See [LICENSE](LICENSE) for more details.

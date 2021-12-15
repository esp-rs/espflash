# cargo-espflash

Cross-compiler and serial flasher cargo subcommand for Espressif SoCs and modules.

Currently supports the **ESP32**, **ESP32-C3**, **ESP32-S2**, **ESP32-S3**, and **ESP8266**.

Prior to flashing, the project is built using the `build-std` unstable Cargo feature. Please refer to the [cargo documentation](https://doc.rust-lang.org/cargo/reference/unstable.html#build-std) for more information.

## Installation

```shell
$ cargo install cargo-espflash
```

## Usage

```text
cargo-espflash 1.2.0

USAGE:
    cargo espflash [OPTIONS] [SERIAL] [SUBCOMMAND]

ARGS:
    <SERIAL>    Serial port connected to target device

OPTIONS:
        --board-info
            Display the connected board's information (deprecated, use the `board-info` subcommand
            instead)

        --bootloader <BOOTLOADER>
            Path to a binary (.bin) bootloader file

        --example <EXAMPLE>
            Example to build and flash

        --features <FEATURES>...
            Comma delimited list of build features

        --format <FORMAT>
            Image format to flash (bootloader/direct-boot)

    -h, --help
            Print help information

        --monitor
            Open a serial monitor after flashing

        --package <PACKAGE>
            Specify a (binary) package within a workspace to be built

        --partition-table <PARTITION_TABLE>
            Path to a CSV file containing partition table

        --ram
            Load the application to RAM instead of Flash

        --release
            Build the application using the release profile

        --speed <SPEED>
            Baud rate at which to flash target device

        --target <TARGET>
            Target to build for

    -V, --version
            Print version information

SUBCOMMANDS:
    board-info    Display the connected board's information
    help          Print this message or the help of the given subcommand(s)
    save-image    Save the image to disk instead of flashing to device
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

[usb_device]
vid = 12346 # 0x303A
pid = 32768 # 0x8000
```

## Package Metadata

You can specify the bootloader, partition table, or image format for a project in the package metadata in `Cargo.toml`:

```toml
[package.metadata.espflash]
partition_table = "partitions.csv"
bootloader = "bootloader.bin"
format = "direct-boot"
```

## License

Licensed under the GNU General Public License Version 2. See [LICENSE](LICENSE) for more details.

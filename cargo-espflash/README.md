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
cargo-espflash 1.5.1

USAGE:
    cargo espflash [OPTIONS] [SERIAL] [SUBCOMMAND]

ARGS:
    <SERIAL>    Serial port connected to target device

OPTIONS:
        --bootloader <BOOTLOADER>
            Path to a binary (.bin) bootloader file

        --example <EXAMPLE>
            Example to build and flash

    -f, --flash-freq <FREQUENCY>
            Flash frequency [possible values: 20M, 26M, 40M, 80M]

        --features <FEATURES>
            Comma delimited list of build features

        --format <FORMAT>
            Image format to flash [possible values: bootloader, direct-boot]

    -h, --help
            Print help information

    -m, --flash-mode <MODE>
            Flash mode to use [possible values: QIO, QOUT, DIO, DOUT]

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

    -s, --flash-size <SIZE>
            Flash size of the target [possible values: 256KB, 512KB, 1MB, 2MB, 4MB, 8MB, 16MB, 32MB,
            64MB, 128MB]

        --speed <SPEED>
            Baud rate at which to flash target device

        --target <TARGET>
            Target to build for

    -V, --version
            Print version information

    -Z <UNSTABLE>
            Unstable (nightly-only) flags to Cargo, see 'cargo -Z help' for details

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
vid = "303A"
pid = "8000"
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

# `cargo-espflash`

Cross-compiler and serial flasher cargo subcommand for Espressif devices. Currently supports __ESP32__, __ESP32-S2__, __ESP32-C3__, and __ESP8266__.

Prior to flashing, the project is built using the `build-std` unstable cargo feature. Please refer to the [cargo documentation](https://doc.rust-lang.org/cargo/reference/unstable.html#build-std) for more information.

## Usage

```text
cargo-espflash 1.0.0
Cargo subcommand for flashing Espressif devices over serial

USAGE:
    cargo espflash [FLAGS] [OPTIONS] [SERIAL]

FLAGS:
        --board-info    Display the connected board's information
    -h, --help          Prints help information
        --monitor       Open a serial monitor after flashing
        --ram           Load the application to RAM instead of Flash
        --release       Build the application using the release profile
    -V, --version       Prints version information

OPTIONS:
        --bootloader <PATH>         Path to a binary (.bin) bootloader file
        --example <EXAMPLE>         Example to build and flash
        --features <FEATURES>       Comma delimited list of build features
        --partition-table <PATH>    Path to a CSV file containing partition table
        --speed <SPEED>             Baud rate at which to flash target device

ARGS:
    <SERIAL>    Serial port connected to target device
```

When the `--ram` option is specified, the provided ELF image will be loaded into ram and executed without touching the flash.

### Config

You can also specify the serial port by setting it in the config file located at `~/.config/espflash/espflash.toml` or Linux
or `%APPDATA%/esp/espflash/espflash.toml` on Windows.

```toml
[connection]
serial = "/dev/ttyUSB0"
```

### Package metadata

You can also specify the bootloader or partition table for a project in the package metadata in `Cargo.toml`

```toml
[package.metadata.espflash]                                                                                                                                                                                                                                    
partition_table = "partitions.csv"
bootloader = "bootloader.bin"
```

### Example

```bash
$ cargo espflash --release --example blinky /dev/ttyUSB0
```

## License

Licensed under the GNU General Public License Version 2. See [LICENSE](LICENSE) for more details.

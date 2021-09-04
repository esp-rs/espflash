# `cargo-espflash`

Cross-compiler and serial flasher cargo subcommand for Espressif devices. Currently supports __ESP32__, __ESP32-C3__, and __ESP8266__.

Prior to flashing, the project is built using the `build-std` unstable cargo feature. Please refer to the [cargo documentation](https://doc.rust-lang.org/cargo/reference/unstable.html#build-std) for more information.

## Usage

```text
cargo-espflash 0.1.4
Cargo subcommand for flashing Espressif devices over serial

USAGE:
    cargo espflash [FLAGS] [OPTIONS] [SERIAL]

FLAGS:
        --board-info    Display the connected board's information
    -h, --help          Prints help information
        --ram           Load the application to RAM instead of Flash
        --release       Build the application using the release profile
    -V, --version       Prints version information

OPTIONS:
        --example <EXAMPLE>      Example to build and flash
        --features <FEATURES>    Comma delimited list of build features
        --speed <SPEED>          Baud rate at which to flash target device

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

### Example

```bash
$ cargo espflash --release --example=blinky /dev/ttyUSB0
```

## License

Licensed under the GNU General Public License Version 2. See [LICENSE](LICENSE) for more details.

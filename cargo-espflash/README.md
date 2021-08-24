# `cargo-espflash`

_ESP8266_ and _ESP32_ cross-compiler and serial flasher cargo subcommand.

Prior to flashing, the project is built using the `build-std` unstable cargo feature. Please refer to the [cargo documentation](https://doc.rust-lang.org/cargo/reference/unstable.html#build-std) for more information.

## Usage

```bash
$ cargo espflash [--board-info] [--ram] [--release] [--example EXAMPLE] [--chip {esp32,esp8266}] <serial>
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

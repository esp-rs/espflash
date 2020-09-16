# `cargo-espflash`

_ESP8266_ and _ESP32_ cross-compiler and serial flasher cargo subcommand.

To build the project before flashing, `cargo-espflash` has a few options, specified with the `--tool TOOL` flag.

 - `--tool xbuild`, build using `cargo xbuild`. Requires [cargo xbuild](https://github.com/rust-osdev/cargo-xbuild) installed on your system. This is the default option.
 - `--tool cargo`, build using the `build-std` unstable cargo feature.
 - `--tool xargo`, build using `xargo`. Requires [xargo](https://github.com/japaric/xargo) installed on your system.

## Usage

```bash
$ cargo espflash [--board-info] [--ram] [--release] [--example EXAMPLE] [--chip {esp32,esp8266}] [--tool {{cargo,xargo,xbuild}}] <serial>
```

When the `--ram` option is specified, the provided ELF image will be loaded into ram and executed without touching the flash.

### Config

You can also specify the serial port or build tool by setting it in the config file located at `~/.config/espflash/espflash.toml` or Linux
or `%APPDATA%/esp/espflash/espflash.toml` on Windows.

```toml
[connection]
serial = "/dev/ttyUSB0"

[build]
tool = "cargo"
```

### Example

```bash
$ cargo espflash --release --example=blinky /dev/ttyUSB0
```

## License

Licensed under the GNU General Public License Version 2. See [LICENSE](LICENSE) for more details.

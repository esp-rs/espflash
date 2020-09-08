# `cargo-espflash`

_ESP8266_ and _ESP32_ cross-compiler and serial flasher cargo subcommand.

Currently, `cargo-espflash` requires that you have [xargo](https://github.com/japaric/xargo) installed on your system.

## Usage

```bash
$ cargo espflash [--ram] [--release] [--example EXAMPLE] --chip {esp32,esp8266} <serial>
```

When the `--ram` option is specified, the provided ELF image will be loaded into ram and executed without touching the flash.

### Example

```bash
$ cargo espflash --release --example=blinky --chip=esp8266 /dev/ttyUSB0
```

## License

Licensed under the GNU General Public License Version 2. See [LICENSE](LICENSE) for more details.

# `espflash`

__ESP32__, __ESP32-S2__, __ESP32-C3__, and __ESP8266__ serial flasher library and CLI application.

[![asciicast](https://asciinema.org/a/367205.svg)](https://asciinema.org/a/367205)

## Installation

```shell
cargo install espflash
```

## Usage

```bash
$ espflash [--board-info] [--ram] <path to serial> <path to elf image>
```

When the `--ram` option is specified, the provided ELF image will be loaded into ram and executed without touching the flash.

When the `--board-info` is specified, instead of flashing anything, the chip type and flash size will be printed.

### Config

You can also specify the serial port by setting it in the config file located at `~/.config/espflash/espflash.toml` or linux
or `%APPDATA%/esp/espflash/espflash.toml` on Windows.

```toml
[connection]
serial = "/dev/ttyUSB0"
```


### As cargo runner

You can also use `espflash` as a cargo runner by setting

```
[target.'cfg(all(target_arch = "xtensa", target_os = "none"))']
runner = "espflash --ram /dev/ttyUSB0"
```

in your `.cargo/config`, which then allows you to run your project using `xargo run`.

## License

Licensed under the GNU General Public License Version 2. See [LICENSE](LICENSE) for more details.

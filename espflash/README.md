# `espflash`

_ESP8266_ and _ESP32_ serial flasher library and CLI application.

## Usage

```bash
$ espflash [--ram] <path to serial> <path to elf image>
```

When the `--ram` option is specified, the provided ELF image will be loaded into ram and executed without touching the flash.

### As cargo runner

You can also use `espflash` as a cargo runner by setting

```
[target.'cfg(all(target_arch = "xtensa", target_os = "none"))']
runner = "espflash --ram /dev/ttyUSB0"
```

in your `.cargo/config`, which then allows you to run your project using `xargo run`.

## License

Licensed under the GNU General Public License Version 2. See [LICENSE](LICENSE) for more details.

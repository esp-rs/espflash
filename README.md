# ESPFlash

ESP8266 and ESP32 serial flasher

## Status

Currently only ESP8266 is supported, ESP32 support will follow.

## Usage

```
espflash [--ram] <path to serial> <path to elf image>
```

when the `--ram` option is specified, the provided elf image will be loaded into ram and executed without touching the flash.

### As cargo runner

You can also use `espflash` as a cargo runner by setting

```
[target.'cfg(all(target_arch = "xtensa", target_os = "none"))']
runner = "espflash --ram /dev/ttyUSB0"
```

in your `.cargo/config`. Which then allows you to run your project using `xargo run`.

## License

Licensed under the GNU General Public License Version 2.
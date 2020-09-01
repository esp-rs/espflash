# ESPFlash

ESP8266 and ESP32 serial flasher

## Status

Currently only ESP8266 is supported, ESP32 support will follow.

## Usage

```
espflash [--ram] <path to serial> <path to elf image>
```

when the `--ram` option is specified, the provided elf image will be loaded into ram and executed without touching the flash.

## License

Licensed under the GNU General Public License Version 2.
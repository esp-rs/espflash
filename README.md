# ESPFlash
[![Actions Status](https://github.com/esp-rs/espflash/workflows/CI/badge.svg)](https://github.com/marcelbuesing/espflash/actions?query=workflow%3A"CI")

_ESP8266_ and _ESP32_ serial flasher based on [esptool.py](https://github.com/espressif/esptool).

* [espflash library & binary](https://github.com/icewind1991/espflash/tree/master/espflash)
* [espflash cargo subcommand](https://github.com/icewind1991/espflash/tree/master/cargo-espflash)

## Status

Flashing _should_ work for both ESP32 and ESP8266.

If you have an ELF file that flashes correctly with `esptool.py` but not with this tool then please open an issue with the ELF in question.

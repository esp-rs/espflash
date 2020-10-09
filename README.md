# ESPFlash

_ESP8266_ and _ESP32_ serial flasher based on [esptool.py](https://github.com/espressif/esptool).

* [espflash library & binary](https://github.com/icewind1991/espflash/tree/master/espflash)
* [espflash cargo subcommand](https://github.com/icewind1991/espflash/tree/master/cargo-espflash)

## Status

Flashing _should_ work for both ESP32 and ESP8266.

If you have an ELF file that flashes correctly with `esptool.py` but not with this tool then please open an issue with the ELF in question.

## Quickstart - Docker

The docker image `esp-rs/espflash` contains all necessary toolchains and tooling including espflash to build and flash.
To clone, build and flash the [xtensa-rust-quickstart](https://github.com/MabezDev/xtensa-rust-quickstart) esp32 example run the following:

```cmd
git clone --depth 1 https://github.com/MabezDev/xtensa-rust-quickstart.git
cd xtensa-rust-quickstart
docker run -v "$(pwd):/espflash" --device=/dev/ttyUSB0 -ti esp-rs/espflash --release --tool=cargo --chip=esp32 --example=esp32 --features="xtensa-lx-rt/lx6 xtensa-lx/lx6 esp32-hal" /dev/ttyUSB0
```

### Custom Docker Build

```cmd
git clone --depth 1 https://github.com/esp-rs/espflash.git
cd espflash
docker build -t esp-rs/espflash .
```

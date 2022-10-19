# Test Resources

This document describes how the test files under `tests/resources` were generated, so that they can be re-generated in the future if needed.

## IDF Bootloader

```bash
$ git clone https://github.com/esp-rs/esp-hal
$ cd esp32-hal/
$ cargo build --release --example=blinky
```

The ELF file is located at `target/xtensa-esp32-none-elf/examples/blinky`

```bash
$ espflash save-image --chip=esp32 esp32_hal_blinky.bin esp32_hal_blinky
```

## ESP8266

```bash
$ git clone https://github.com/esp-rs/esp8266-hal
$ cd esp8266-hal/
$ cargo build --release --example=blinky
```

The ELF file is located at `target/xtensa-esp8266-none-elf/examples/blinky`

```bash
$ espflash save-image --chip=esp8266 esp8266_hal_blinky.bin esp8266_hal_blinky
```

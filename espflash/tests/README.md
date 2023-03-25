# Test Resources

This document describes how the test files under `tests/resources` were generated, so that they can be re-generated in the future if needed.

## Direct Boot

```bash
$ git clone https://github.com/esp-rs/esp-hal
$ cd esp-hal/esp32c3-hal/
$ cargo build --release --features=direct-boot --example=blinky
```

The ELF file is located at `target/riscv32imc-unknown-none-elf/release/examples/blinky`

```bash
$ espflash save-image --format=direct-boot --chip=esp32c3 esp32c3_hal_blinky_db.bin esp32c3_hal_blinky_db
```

## IDF Bootloader

```bash
$ git clone https://github.com/esp-rs/esp-hal
$ cd esp-hal/esp32-hal
$ cargo build --release --example=blinky
```

The ELF file is located at `target/xtensa-esp32-none-elf/release/examples/blinky`

```bash
$ espflash save-image --chip=esp32 esp32_hal_blinky.bin esp32_hal_blinky
```

## ESP8266

```bash
$ git clone https://github.com/esp-rs/esp8266-hal
$ cd esp8266-hal/
$ cargo build --release --example=blinky
```

The ELF file is located at `target/xtensa-esp8266-none-elf/release/examples/blinky`

```bash
$ espflash save-image --chip=esp8266 esp8266_hal_blinky.bin esp8266_hal_blinky
```

# Test Resources

This document describes how the test files under `tests/resources` were generated, so that they can be re-generated in the future if needed.

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

# Espflash Resources

The listed bootloaders from `espressif/esp-idf` were built with `release/v5.5` at commit `d66ebb8`, using default settings:
https://github.com/espressif/esp-idf/tree/release/v5.5

For now, `esp-hal` uses MMU page size as `0x10000` (64k) for ESP32-C2, ESP32-C6 and ESP32-H2, therefore the flash size has to be changed to 64MB. This can be done in `menuconfig` with the `flash size` config or adding the following to the `sdkconfig` file:

```
CONFIG_ESPTOOLPY_FLASHSIZE_64MB=y
CONFIG_ESPTOOLPY_FLASHSIZE="64MB"
```

The `esp32p4-v0-bootloader.bin` was built using `v5.5.3` and the following configs:
```
CONFIG_ESP32P4_SELECTS_REV_LESS_V3=y
CONFIG_ESP32P4_REV_MIN_100=y
```

The flasher stubs are taken from the `espressif/esptool` repository:
https://github.com/espressif/esptool/tree/master/esptool/targets/stub_flasher/1


The roms are taken from the (`esp-rom-elfs`)[https://github.com/espressif/esp-rom-elfs] repository. Expect for:
- ESP32-P4 rev3: Was built from `esp-rom-elfs` gitlab merge request 30.
- `esp32c5_rev100_rom.elf` and `esp32c61_rev100_rom.elf`: taken from release `20260313` of `esp-rom-elfs`: https://github.com/espressif/esp-rom-elfs/releases/tag/20260313

# Espflash Resources

The listed bootloaders from `espressif/esp-idf` were built with `release/v5.5` at commit `d66ebb8`, using default settings:
https://github.com/espressif/esp-idf/tree/release/v5.5

For now, `esp-hal` uses MMU page size as `0x10000` (64k) for ESP32-C2, ESP32-C6 and ESP32-H2, therefore the flash size has to be changed to 64MB. This can be done in `menuconfig` with the `flash size` config or adding the following to the `sdkconfig` file:

```
CONFIG_ESPTOOLPY_FLASHSIZE_64MB=y
CONFIG_ESPTOOLPY_FLASHSIZE="64MB"
```

The flasher stubs are taken from the `espressif/esptool` repository:
https://github.com/espressif/esptool/tree/master/esptool/targets/stub_flasher/1

# Espflash Resources

The listed bootloaders from `espressif/esp-idf` were built with `release/v5.4` at commit `3ad3632`, using default settings:
https://github.com/espressif/esp-idf/tree/release/v5.4

For now, `esp-hal` uses MMU page size as `0x10000` (64k) therefore the flash size has to be changed to 64MB.

The flasher stubs are taken from the `espressif/esptool` repository:
https://github.com/espressif/esptool/tree/master/esptool/targets/stub_flasher/1

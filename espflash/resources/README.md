# Espflash Resources

The listed bootloaders from `espressif/esp-idf` are generated from ESP-IDF `release/v6.1` and the `sdkconfig` fragments in `bootloaders/manifest.yaml`. Manifest entries marked `preview: true` are built with `idf.py --preview`.

To rebuild them, run:

```
cargo build-bootloaders --install-tools
# or: cargo run -p xtask -- build-bootloaders --install-tools
```

Useful options:

- `--only <name>` builds one manifest entry, for example `--only esp32p4-v3`.
- `--esp-idf-path <path>` changes the managed ESP-IDF checkout location.
- `--no-fetch` uses an existing checkout without fetching updates.

ESP-IDF tools are installed and exported with `IDF_TOOLS_PATH` set to `target/esp-idf-tools` by default, so they can be removed with the rest of `target`. Set `IDF_TOOLS_PATH` explicitly to use another location. On Unix this uses `install.sh`/`export.sh`; on Windows it uses `install.bat`/`export.bat`.

For now, `esp-hal` uses MMU page size as `0x10000` (64k) for some chips, therefore those manifest entries set the ESP-IDF flash size to 64MB. ESP32-P4 revision-specific bootloader configs are also captured in the manifest.

The flasher stubs are from `esp-flasher-stub` v1.1.0, as bundled with the
`espressif/esptool` repository:
https://github.com/espressif/esptool/tree/master/esptool/targets/stub_flasher/2

The roms are taken from the [`esp-rom-elfs`](https://github.com/espressif/esp-rom-elfs) repository, except for:
- ESP32-P4 rev3: Was built from `esp-rom-elfs` gitlab merge request 30.
- `esp32c5_rev100_rom.elf` and `esp32c61_rev100_rom.elf`: taken from release `20260313` of `esp-rom-elfs`: https://github.com/espressif/esp-rom-elfs/releases/tag/20260313
- `esp32s31_rev0_rom.elf`: taken from release `20260528` of `esp-rom-elfs`: https://github.com/espressif/esp-rom-elfs/releases/tag/20260528

ESP32-H4 currently has no published ROM ELF in `esp-rom-elfs`.
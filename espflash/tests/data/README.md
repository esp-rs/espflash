The `$CHIP` elf files under this folder have been generated using `esp-generate@0.3.1`, `esp-hal@5c97eaf`, `esp-println@ab18d89`:

```
esp-generate --chip=$CHIP --headless $CHIP
cd $CHIP
cargo build --release
```

The `esp32c6_defmt` elf file under this folder has been generated using `esp-generate@0.3.1`, `esp-hal@5c97eaf`, `esp-println@ab18d89`:

> TODO: this part needs to be updated, and the elf re-created once ESP-HAL beta.1 is out.

```
esp-generate --chip=esp32c6 -o defmt --headless esp32c6_defmt
cd esp32c6_defmt
DEFMT_LOG=info cargo build --release
```

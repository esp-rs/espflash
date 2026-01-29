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


The `esp32c6_backtrace` elf file under this folder has been generated using `esp-generate@f4213a9`:
```
esp-generate --chip=esp32c6 --headless -o esp-backtrace esp32c6_backtrace
cd esp32c6_backtrace
```
Modified the main.rs to panic:
```diff
    let _peripherals = esp_hal::init(config);

+    panic!("test");
+
    loop {
```
And then build the elf file:
```
cargo build --release
```

`esp_hal_binary_with_overlapping_defmt_and_embedded_test_sections` is the ESP-HAL `gpio_unstable` test built for ESP32.
This file is used in a unit test in espflash, and is not flashed as a HIL test.

The `esp32c5` and `esp32p4` elf files under this folder have been generated using `esp-idf@v5.5.2`:
```
 git clone -b v5.5.2 --recursive https://github.com/espressif/esp-idf.git
cd esp-idf/
./install.sh all
cd examples/get-started/hello_world/
idf.py set-target $CHIP
idf.py build
```

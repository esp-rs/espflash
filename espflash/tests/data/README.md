The `$CHIP` elf files under this folder have been generated using `esp-generate@1.4.0`, `esp-hal@1.2.2`, `esp-println@0.18.0`, `esp-bootloader-esp-idf@0.6.0`, for `$CHIP` in `esp32`, `esp32c2`, `esp32c3`, `esp32c5`, `esp32c6`, `esp32c61`, `esp32h2`, `esp32s2`, `esp32s3` and `esp32s31`:

```
esp-generate --headless -o $CHIP -o log $CHIP
cd $CHIP
```

Every test firmware prints to a single console, as the `auto` mode of `esp-println` switches to USB-Serial-JTAG as soon as the host polls the USB port, which loses the output of HIL jobs reading the UART of a board whose USB port is connected too. The console is selected with a feature, so replace the `esp-println` dependency in `Cargo.toml` and add the features:
```diff
-esp-println      = { version = "0.18.0", features = ["$CHIP", "log-04"] }
+esp-println      = { version = "0.18.0", default-features = false, features = ["$CHIP", "log-04", "colors", "critical-section"] }
+
+[features]
+# Serial console the firmware prints to
+uart = ["esp-println/uart"]
+usb  = ["esp-println/jtag-serial"]
```
The `usb` feature only exists for chips with USB-Serial-JTAG, so leave it out for `esp32`, `esp32c2` and `esp32s2`.

And then build the elf files, `$CHIP` printing to UART and `${CHIP}_usb` printing to USB-Serial-JTAG:
```
cargo build --release --features uart
cargo build --release --features usb
```

`xtask run-tests --console <uart|usb>` selects the firmware matching the port under test.

The `esp32c6_defmt` and `esp32c6_defmt_usb` elf files under this folder have been generated using `esp-generate@1.4.0`, `esp-hal@1.2.2`, `esp-println@0.18.0`, `defmt@1.1.1`:

```
esp-generate --headless -o esp32c6 -o defmt esp32c6_defmt
cd esp32c6_defmt
```
Changed `Cargo.toml` as for the `$CHIP` elf files (with the `defmt-espflash` feature instead of `log-04`), and then built the elf files:
```
cargo build --release --features uart
cargo build --release --features usb
```

The `esp32c6_backtrace` and `esp32c6_backtrace_usb` elf files under this folder have been generated using `esp-generate@1.4.0`, `esp-hal@1.2.2`, `esp-println@0.18.0`, `esp-backtrace@0.20.0`:
```
esp-generate --headless -o esp32c6 -o esp-backtrace esp32c6_backtrace
cd esp32c6_backtrace
```
Changed `Cargo.toml` as for the `$CHIP` elf files (without the `log-04` feature), and modified the main.rs to panic:
```diff
    let _peripherals = esp_hal::init(config);

+    panic!("test");
+
    loop {
```
And then build the elf files:
```
cargo build --release --features uart
cargo build --release --features usb
```
The HIL backtrace test checks the addresses of the calls in `main` and `hal_main`, so update them in `xtask/src/test_runner.rs` whenever these elf files are rebuilt.

The `esp32c6_espidf_abort` elf file under this folder has been generated using `esp-idf@v5.5.5`, from `examples/get-started/hello_world` with `idf.py set-target esp32c6` and `idf.py build`, after modifying `main/hello_world_main.c` to crash right after printing:
```diff
     printf("Hello world!\n");
+
+    // Crash on purpose: the panic handler prints a register and stack memory
+    // dump which the espflash monitor decodes into a backtrace.
+    abort();
```
It is used in a HIL test which checks that the monitor decodes the register and stack memory dump printed by the ESP-IDF panic handler into a backtrace, using the `.debug_frame` of the ELF.

`esp_hal_binary_with_overlapping_defmt_and_embedded_test_sections` is the ESP-HAL `gpio_unstable` test built for ESP32.
This file is used in a unit test in espflash, and is not flashed as a HIL test.

The `esp_idf_hello_world_c61.elf` elf file under this folder has been generated using `esp-idf@v5.5.2`:
```
 git clone -b v5.5.2 --recursive https://github.com/espressif/esp-idf.git
cd esp-idf/
./install.sh all
cd examples/get-started/hello_world/
idf.py set-target esp32c61
idf.py build
```
The `esp_hal_backtrace_c6.elf` elf file under this folder has been generated using `esp-generate@f4213a9`, as the `esp32c6_backtrace` elf file.
These files are used in the stack dump unwinding unit tests of the espflash monitor, which rely on their addresses, and are not flashed as HIL tests.

The `esp32p4` elf file under this folder has been generated using `esp-idf@v5.5.2` as above, with `idf.py set-target esp32p4`, as `esp-generate` doesn't support the ESP32-P4.

The `esp32h4` elf file under this folder has been generated using ESP-IDF `release/v6.1` (preview target):
```
idf.py --preview set-target esp32h4
idf.py --preview build
```
from `examples/get-started/hello_world`.

The ESP-IDF elf files print to both UART and USB-Serial-JTAG, so a single elf file is used for both consoles.

## SDM HIL Setup for ESP32C6

The VM running SDM HIL is connected to a ESP32C6. To enable secure download mode, the following command needs to be run:

```
espefuse --port /dev/serial_ports/esp32c6 burn_efuse ENABLE_SECURITY_DOWNLOAD 1
```

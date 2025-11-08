# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

### Changed

### Fixed

- Corrected eFuse BLOCK0 definitions for ESP32-C2, ESP32-C3, and ESP32-S3 (#961)

### Removed

## [4.2.0] - 2025-10-13

### Added

- Add chip detection based on security info, where supported (#953)
- Support for decoding `esp-backtrace`'s RISC-V stack-dump output (#955)

### Changed

- Move `SecurityInfo` to the `connection` module from the `flasher` module (#953)

### Fixed

- Fix a crash with Connection module when connecting to a loopback serial port (#954)

## [4.1.0] - 2025-09-18

### Added

- Add `--no-addresses` flag to `monitor` subcommand (#942)
- Add new ESP32-C5 magic values (#940)

### Changed

- Rename `--check-app-descriptor` to `--ignore-app-descriptor` (#833)

### Fixed

- Fix a crash in monitor when espflash is connected via USB Serial/JTAG, and the user is typing into the monitor but the device is not reading serial input. (#943, #944, #945)
- Fix ESP32-S2 flash size detection issues (#950)
- Images are now automatically padded to 4 bytes before writing by the library (previously this was done in the CLI) (#951)

## [4.0.1] - 2025-07-07

### Changed

- `espflash` now allows wider version ranges on its dependencies(#924)

### Fixed

- `save-image` now checks if the ELF contains the app descriptor (#920)

## [4.0.0] - 2025-07-01

### Added

- Add `non-interactive` flag to `flash` subcommand (#737)
- Add `no-reset` flag to `monitor` subcommands (#737)
- Add an environment variable to set monitoring baudrate (`MONITOR_BAUD`) (#737)
- Add list-ports command to list available serial ports. (#761)
- [cargo-espflash]: Add `write-bin` subcommand (#789)
- Add `--monitor` option to `write-bin`. (#783)
- Add `watchdog-reset` strategy to `--after` subcommand (#779)
- Add `ROM` version of `read-flash` command (#812)
- `espflash` can detect the log format automatically from ESP-HAL metadata. Requires `esp-println` 0.14 (#809)
- Add `--output-format` option to monitor (#818)
- Added chip detection based on security info, where supported (#814)
- `espflash` can detect the chip from ESP-HAL metadata to prevent flashing firmware built for a different device. Requires `esp-hal` 1.0.0-beta.0 (#816)
- `espflash` no longer allows flashing a too-big partition table (#830)
- Allow specifying a partition label for `write-bin`, add `--partition-table`. (#828)
- `--mmu-page-size` parameter for `flash` and `save-image` (#835)
- Run some arguments checks for monitoring flags. (#842)
- Add support for the ESP32-C5 (#863)
- `--after` options now work with `espflash board-info`, `espflash read-flash` and `espflash checksum-md5` (#867)
- Add support for serial port configuration files. (#777, #883)
- Add a `check-app-descriptor` bool option to `ImageArgs` and add the flag to `flash` command (#872)
- `Connection::into_serial` to get the underlying port from the connection (#882)
- All methods on the now removed `Target` & `ReadEFuse`, `UsbOtg` and `RtcWdtReset` traits have been implemented directly on (#891)
- Update checks can now be skipped by setting the `ESPFLASH_SKIP_UPDATE_CHECK` environment variable (#900)
- `flash_write_size` and `max_ram_block_size` functions no longer take a connection parameter and return a Result type (#903)
- `DefaultProgressCallback` which implements `ProgressCallbacks` but all methods are no-ops (#904)
- `ProgressCallbacks` now has a `verifying` method to notify when post-flash checksum checking has begun (#908)
- Implement `From<Connection> for Port` and both `From<Flasher> for Connection` and `Port` conversions (#915)

### Changed

- Split the baudrate for connecting and monitoring in `flash` subcommand (#737)
- Normalized arguments of the CLI commands (#759)
- `board-info` now prints `Security information`. (#758)
- The `command`, `elf` and `error` modules are no longer public (#772)
- `write-bin` now works for files whose lengths are not divisible by 4 (#780, #788)
- `get_usb_pid` is now `usb_pid` and no longer needlessly returns a `Result` (#795)
- `CodeSegment` and `RomSegment` have been merged into a single `Segment` struct (#796)
- `IdfBootloaderFormat` has had its constructor's parameters reduced/simplified (#798)
- Update flash size when creating the app partition (#797)
- `--non-interactive` may now react to key events (user input, Ctrl-C, Ctrl-R) if possible (#819)
- Removed `get_` prefix from any functions which previously had it (#824)
- Take elf data as bytes rather than `ElfFile` struct when creating an image format (#825)
- Updated to Rust 2024 edition (#843)
- Complete rework of reading eFuse field values (#847, #903)
- Updated bootloaders with `release/v5.4` ones from IDF (#857)
- Refactor image formatting to allow supporting more image formats in a backward compatible way (#877)
- Avoid having ESP-IDF format assumptions in the codebase (#877)
- `Flasher` now takes the `Connection` in new, instead of constructing the connection inside `Flasher::connect` (#882, #885)
- `detect_chip` has moved to the `Connection` struct (#882)
- `Flasher::into_serial` has been replaced by `Flasher::into_connection` (#882)
- Automatically migrate `espflash@3` configuration files to the new format (#883)
- Update dependencies to their latest versions (#893)
- `Chip::crystal_freq` has been renamed to `Chip::xtal_frequency` (#891)
- `Chip::chip_revision` has been renamed to `Chip::revision` (also applies to `minor` and `major`) (#891)
- Any reference to `esp_idf` or `EspIdf` has been cut to just `idf` (#891)
- Renamed `targets` module to `target` (#891)
- Test data is now excluded from the crates.io release (#897)
- The command module, and `Command` related structs now exist in a top level module, instead of the `connection` module (#901)
- API's that take `Option<&mut dyn ProgressCallbacks>` now take `&mut dyn ProgressCallbacks` instead (#904)
- `ProgressCallbacks::finish()` now has a `skipped: bool` parameter to indicate if a segment was skipped (#904)
- CLI usage now shows when a segment has been skipped due to already-matching checksum and when a segment is being verified (#908)

### Fixed

- Update the app image SHA in the correct location for padded images (#715)
- Fix `-s` argument collision (#731)
- `address` and `size` in `erase-region` have to be multiples of 4096 (#771)
- Fixed typos in error variant names (#782)
- Fix `read-flash` which didn't work with some lengths (#804)
- espflash can now flash an ESP32-S2 in download mode over USB (#813)
- Fixed a case where espflash transformed the firmware ELF in a way that made it unbootable (#831)
- The app descriptor is now correctly placed in the front of the binary (#835)
- espflash now extracts the MMU page size from the app descriptor (#835)
- `ResetBeforeOperation` & `ResetAfterOperation` are now public, to allow the creation of a `Connection` (#895)
- `Flasher` now respects its internal `verify` and `skip` flags for all methods. (#901)
- Progress is now reported on skipped segments and verification (#904)
- Moved the `non-interactive` flag to `ConnectArgs` so we also avoid asking the user to select a port (#906)

### Removed

- Removed the `libudev` feature (#742)
- Removed the `flasher::parse_partition_table` function (#798)
- The `FirmwareImage` trait has been removed (#802)
- The `elf` module has been removed, and its contents moved to the `image_format` module (#802)
- The `Target` trait, the `ReadEFuse` trait, and `Chip::into_target` (#891)
- The `UsbOtg` and `RtcWdtReset` traits have been removed, along with `Chip::into_rtc_wdt_reset` & `Chip::into_usb_otg` (#891)

## [3.3.0] - 2025-01-13

### Added

- Allow `partition_table_offset` to be specified in the config file. (#699)
- Support external log-processors (#705)
- Make the `libudev` dependency optional with a new - enabled by default - feature: `libudev` (#709)

### Fixed

- Only filter the list of available serial ports if a port has not been specified via CLI option or configuration file (#693)
- Address Clippy lints (#710)

## [3.2.0]

### Added

- Add new chip detect magic value, ability to read chip revision for ESP32-P4 (#686)
- Add skip update check option (#689)

### Fixed

- Fixed `partition-table-offset` argument to accept offsets in hexadecimal (#682)
- espflash defmt log didn't display timestamp, according to [defmt doc](https://defmt.ferrous-systems.com/timestamps). (#680)
- Fixed pattern matching to detect download mode over multiple lines (#685)

## [3.1.1] - 2024-08-15

### Added

- Add `hold-in-reset` and `reset` subcommands (#644)
- [cargo-espflash]: Add `--no-default-features` flag to mirror cargo features behavior (#647)
- Update `cargo` and `bytemuck` dependencies adapting code (#666)

### Fixed

- Downgrade crossterm and update time crates (#659)
- Monitor now only sends key presses on key down events

### Changed

## [3.1.0] - 2024-05-24

### Added

- Support loading flash size, frequency, and mode from the config file (#627)

### Fixed

- Fixed help text for `size` parameter of `read-flash` subcommand
- Fixed port detection on `musl` when detection returns paths starting with `/dev/`
- [cargo-espflash]: Always resolve package_id from metadata when finding bootloader and partition table (#632)
- Fixed behavior of the `--target-app-partition` flag (#634)

### Changed

- Update ESP32, ESP32-C2, ESP32-C3, ESP32-C6, ESP32-H2, ESP32-S2, ESP32-S3 stub (#638)

## [3.0.0] - 2024-03-13

### Fixed

- Fix timeout while changing the baudrate for some ESP32-S3 targets (#607)

### Changed

- Update ESP32, ESP32-C2, ESP32-C3, ESP32-C6, ESP32-H2, ESP32-S2, ESP32-S3 stub (#607, #610)

## [3.0.0-rc.2] - 2024-03-04

### Added

- Add `--list-all-ports` connection argument to avoid serial port filtering (#590)
- Allow config file to live in parent folder (#595)

### Fixed

- Change the `hard_reset` sequence to fix Windows issues (#594)
- Improve resolving non-code addresses (#603)

### Changed

- Non-linux-musl: Only list the available USB Ports by default (#590)
- `FlashData::new` now returns `crate::Error` (#591)
- Moved `reset_after_flash` method to `reset` module (#594)
- The `command` module now requires `serialport`. (#599)

## [3.0.0-rc.1] - 2024-02-16

### Added

- Add reset strategies (#487)
- Read `esp-println` generated `defmt` messages (#466)
- Add `--target-app-partition` argument to flash command (#461)
- Add `--confirm-port` argument to flash command (#455)
- Add `--chip argument` for flash and write-bin commands (#514)
- Add `--partition-table-offset` argument for specifying the partition table offset (#516)
- Add `Serialize` and `Deserialize` to `FlashFrequency`, `FlashMode` and `FlashSize` (#528)
- Add `checksum-md5` command (#536)
- Add verify and skipping of unchanged flash regions - add `--no-verify` and `--no-skip` (#538)
- Add `--min-chip-rev` argument to specify minimum chip revision (#525)
- Add `serialport` feature (#535)
- Add support for 26 MHz bootloader for ESP32 and ESP32-C2 (#553)
- Add CI check to verify that CHANGELOG is updated (#560)
- Add `--before` and `--after` reset arguments (#561)
- Add `read-flash` command (#558)
- Add HIL testing (#596)

### Fixed

- Fix printing panic backtraces when using `esp-println` and `defmt` (#496)
- Fix `defmt` parsing when data is read in parts (#503)
- Use partition table instead of hard-coded values for the location of partitions (#516)
- Fix a missed `flush` call that may be causing communication errors (#521)
- Fix "SHA-256 comparison failed: [...] attempting to boot anyway..." (#567)
- Windows: Update RST/DTR order to avoid issues (#562)
- Tolerate non-utf8 data in boot detection (#573)
- Fix flash/monitoring of 26MHz targets (#584)

### Changed

- Create `FlashData`, `FlashDataBuilder` and `FlashSettings` structs to reduce number of input arguments in some functions (#512, #566)
- `espflash` will now exit with an error if `defmt` is selected but not usable (#524)
- Unify configuration methods (#551)
- Improved symbol resolving (#581)
- Update ESP32-C2 stub (#584)
- MSRV bumped to `1.74.0` (#586)

### Removed

- Remove support for Cargo metadata configuration (#551)
- Remove support for the ESP8266 (#576)
- Remove the direct boot image format (#577)
- Remove support for Raspberry Pi's internal UART peripherals (#585)

## [2.1.0] - 2023-10-03

### Added

- Added erase-flash, erase-region, and erase-parts subcommands (#462)

### Fixed

- Fixed printing UTF-8 sequences that were read in multiple parts. (#468)

### Changed

- Update dependencies to their latest versions (#482)

## [2.0.1] - 2023-07-13

### Added

- Add help text for all subcommands (#441)

### Fixed

- Update `cargo` dependency to 0.72 (#445)

## [2.0.0]

### Fixed

- Explicitly set `bin_name` attribute for `cargo-espflash` (#432)

## [2.0.0-rc.4] - 2023-06-08

### Added

- Add `ESPFLASH_PORT` environment variable (#366)
- Added ESP32-H2 support (#371)
- Generate Shell completions (#388)
- Make the default flashing frequency target specific (#389)
- Add note about permissions on Linux (#391)
- Add a diagnostic to tell the user about the partition table format (#397)
- Add `board-info` command support in Secure Download Mode (#838)

### Fixed

- Fix `espflash::write_bin` (#353)
- Fix ESP32-C3 direct boot (#358)
- Disable watchdog timer before build (#363)
- Restore the cursor when exiting from serial port selection via Ctrl-C (#372)
- Fix chip revision check during flashing for the ESP8266 (#373)
- Fix config file parsing (#382)
- Limit default partition size (#398)
- Fix Windows installation (#399)
- Reword elf too big error (#400)
- Fix handling of serial ports on BSD systems (#415)
- Override the flash size in Flasher if provided via command-line argument (#417)

### Changed

- Simplify and improve errors (#342)
- Make `Interface` constructor public (#354)
- Update stubs from esptool v4.5 (#359)
- Update documentation (#368)
- Update `toml` dependency and fix errors, feature gate `ctrlc` dependency (#378)
- If exactly one port matches, use it (#374)
- Image header improvements and bug fixes (#375)
- Update to the latest version of addr2line and address breaking changes (#412)
- Do not require the `--partition-table` argument when erasing partitions (#413)
- Downgrade `crossterm` to `0.25.0` (#418)
- Update the supported targets for ESP32-C6/H2 (#424)
- Update flasher stubs and bootloaders (#426)

## [2.0.0-rc.3] - 2023-01-12

### Added

- Add support for flashing the ESP32-C6 (#317)
- Add an optional callback trait which can be implemented and provided to most flashing functions (#333)

### Fixed

- Various fixes and improvements relating to crystal frequency and serial monitor for the ESP32-C2 (#314, #315, #330)

### Changed

- Reorder ports so that known ports appear first in CLI (#324)
- Make the flasher return a struct of device information instead of printing directly (#328)
- CLI improvements and dependency updates (#334)
- Use the flasher stub by default (#337)
- Mark public enums as `#[non_exhaustive]` for semver compatibility (#338)
- If a bootloader and/or partition table other than the defaults have been provided, indicate such (#339)

## [2.0.0-rc.2] - 2022-12-07

### Added

- Add option to supply the `ELF` image path in the monitor subcommand (#292)
- Add support for using custom cargo metadata when in a workspace (#300)

### Fixed

- Fix typo in `ImageFormatKind`'s `FromStr` implementation (#308)

### Changed

- Report the image and partition size in the error (#293)
- Allow `SerialPortType::PciPort` during port detection (#295)
- Update dependencies to their latest versions (#299)
- Clean up unused code, optimize comparison in `find_serial_port` (#302)
- Make command module public (#303)
- Display the newer `v{major}.{minor}` chip revision format (#307)

## [2.0.0-rc.1] - 2022-11-07

### Added

- Add support for erasing any partition (#273)

### Fixed

- Various bugfixes, plenty of cleanup and simplification

### Changed

- Redesign of the command-line interface (#239)
- Extract the partition table handling code into a separate package, `esp-idf-part` (#243)
- A bunch of refactoring (#246, #247, #249)
- Updated to `clap@4.0.x` (#251)
- Replace the `espmonitor` dependency with our own home-grown monitor (#254)
- Use logging instead of `println!()` (#256)
- Use newest bootloaders from ESP-IDF (#278)
- Improved documentation and testing

## [1.7.0] - 2022-09-16

## [1.6.0] - 2022-07-11

## [1.5.1] - 2022-05-20

## [1.5.0] - 2022-05-11

## [1.4.1] - 2022-04-16

## [1.4.0] - 2022-04-06

## [1.3.0] - 2022-02-18

## [1.2.0] - 2021-12-15

## [1.1.0] - 2021-10-16

## [1.0.1] - 2021-09-23

## [1.0.0] - 2021-09-21

[Unreleased]: https://github.com/esp-rs/espflash/compare/v4.2.0...HEAD
[4.2.0]: https://github.com/esp-rs/espflash/compare/v4.1.0...v4.2.0
[4.1.0]: https://github.com/esp-rs/espflash/compare/v4.0.1...v4.1.0
[4.0.1]: https://github.com/esp-rs/espflash/compare/v4.0.0...v4.0.1
[4.0.0]: https://github.com/esp-rs/espflash/compare/v3.3.0...v4.0.0
[3.3.0]: https://github.com/esp-rs/espflash/compare/v3.2.0...v3.3.0
[3.2.0]: https://github.com/esp-rs/espflash/compare/v3.1.1...v3.2.0
[3.1.1]: https://github.com/esp-rs/espflash/compare/v3.1.0...v3.1.1
[3.1.0]: https://github.com/esp-rs/espflash/compare/v3.0.0...v3.1.0
[3.0.0]: https://github.com/esp-rs/espflash/compare/v3.0.0-rc.2...v3.0.0
[3.0.0-rc.2]: https://github.com/esp-rs/espflash/compare/v3.0.0-rc.1...v3.0.0-rc.2
[3.0.0-rc.1]: https://github.com/esp-rs/espflash/compare/v2.1.0...v3.0.0-rc.1
[2.1.0]: https://github.com/esp-rs/espflash/compare/v2.0.1...v2.1.0
[2.0.1]: https://github.com/esp-rs/espflash/compare/v2.0.0...v2.0.1
[2.0.0]: https://github.com/esp-rs/espflash/compare/v2.0.0-rc.4...v2.0.0
[2.0.0-rc.4]: https://github.com/esp-rs/espflash/compare/v2.0.0-rc.3...v2.0.0-rc.4
[2.0.0-rc.3]: https://github.com/esp-rs/espflash/compare/v2.0.0-rc.2...v2.0.0-rc.3
[2.0.0-rc.2]: https://github.com/esp-rs/espflash/compare/v2.0.0-rc.1...v2.0.0-rc.2
[2.0.0-rc.1]: https://github.com/esp-rs/espflash/compare/v1.7.0...v2.0.0-rc.1
[1.7.0]: https://github.com/esp-rs/espflash/compare/v1.6.0...v1.7.0
[1.6.0]: https://github.com/esp-rs/espflash/compare/v1.5.1...v1.6.0
[1.5.1]: https://github.com/esp-rs/espflash/compare/v1.5.0...v1.5.1
[1.5.0]: https://github.com/esp-rs/espflash/compare/v1.4.1...v1.5.0
[1.4.1]: https://github.com/esp-rs/espflash/compare/v1.4.0...v1.4.1
[1.4.0]: https://github.com/esp-rs/espflash/compare/v1.3.0...v1.4.0
[1.3.0]: https://github.com/esp-rs/espflash/compare/v1.2.0...v1.3.0
[1.2.0]: https://github.com/esp-rs/espflash/compare/v1.1.0...v1.2.0
[1.1.0]: https://github.com/esp-rs/espflash/compare/v1.0.1...v1.1.0
[1.0.1]: https://github.com/esp-rs/espflash/compare/v1.0.0...v1.0.1
[1.0.0]: https://github.com/esp-rs/espflash/releases/tag/v1.0.0

# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Add `non-interactive` flag to `flash` subcommand (#737)
- Add `no-reset` flag to `monitor` subcommands (#737)
- Add an environment variable to set monitoring baudrate (`MONITOR_BAUD`) (#737)
- Add list-ports command to list available serial ports. (#761)
- [cargo-espflash]: Add `write-bin` subcommand (#789)
- Add `--monitor` option to `write-bin`. (#783)

### Changed

- Split the baudrate for connecting and monitorinig in `flash` subcommand (#737)
- Normalized arguments of the CLI commands (#759)
- `board-info` now prints `Security information`. (#758)
- The `command`, `elf` and `error` modules are no longer public (#772)
- `write-bin` now works for files whose lengths are not divisible by 4 (#780, #788)
- `get_usb_pid` is now `usb_pid` and no longer needlessly returns a `Result` (#795)
- `CodeSegment` and `RomSegment` have been merged into a single `Segment` struct (#796)
- `IdfBootloaderFormat` has had its constructor's parameters reduced/simplified (#798)
- Update flash size when creating the app partition (#797)

### Fixed

- Update the app image SHA in the correct location for padded images (#715)
- Fix `-s` argument collision (#731)
- `address` and `size` in `erase-region` have to be multiples of 4096 (#771)
- Fixed typos in error variant names (#782)
- Fix `read-flash` which didn't work with some lengths (#804)

### Removed

- Removed the `libudev` feature (#742)
- Removed the `flasher::parse_partition_table` function (#798)
- The `FirmwareImage` trait has been removed (#802)
- The `elf` module has been removed, and its contents moved to the `image_format` module (#802)

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

[Unreleased]: https://github.com/esp-rs/espflash/compare/v3.3.0...HEAD
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

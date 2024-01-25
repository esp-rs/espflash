# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

[Unreleased]

### Added

- Added reset strategies (#487)
- Read `esp-println` generated `defmt` messages (#466)
- Add `--target-app-partition` argument to flash command (#461)
- Add `--confirm-port` argument to flash command (#455)
- Add `--chip argument` for flash and write-bin commands (#514)
- Add `--partition-table-offset` argument for specifying the partition table offset (#516)
- Add `Serialize` and `Deserialize` to `FlashFrequency`, `FlashMode` and `FlashSize`. (#528)
- Add `checksum-md5` command (#536)
- Add verify and skipping of unchanged flash regions - add `--no-verify` and `--no-skip` (#538)
- Add `--min-chip-rev` argument to specify minimum chip revision (#525)
- Add `serialport` feature. (#535)
- Add support for 26 MHz bootloader for ESP32 and ESP32-C2 (#553)
- Add `--before` and `--after` reset arguments (#561)

### Fixed

- Fixed printing panic backtraces when using `esp-println` and `defmt` (#496)
- Fixed defmt parsing when data is read in parts (#503)
- Use partition table instead of hard-coded values for the location of partitions (#516)
- Fixed a missed `flush` call that may be causing communication errors (#521)

### Changed
- Created `FlashData` and `FlashSettings` structs to reduce number of input arguments in some functions (#512)

- espflash will now exit with an error if `defmt` is selected but not usable (#524)

### Removed

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

- Various fixesand improvements relating to crystal frequency and serial monitor for the ESP32-C2 (#314, #315, #330)

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

[Unreleased]: https://github.com/esp-rs/espflash/compare/v2.1.0...HEAD
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

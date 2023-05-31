# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Add `ESPFLASH_PORT` environment variable (#366)
- Added ESP32-H2 support (#371)
- Generate Shell completions (#388)
- Make the default flashing frequency target specific (#389)
- Add note about permissions on Linux (#391)
- Add a diagnostic to tell the user about the partition table format (#397)
- Add issue templates (#403)
- Add configuration file examples (#405)
### Fixed

- Fix `espflash::write_bin` (#353)
- Fix ESP32-C3 direct boot (#358)
- Restore the cursor when exiting from serial port selection via Ctrl-C (#372)
- Fix chip revision check during flashing for the ESP8266 (#373)
- Fix Raspberry CI (#377)
- Fix config file parsing (#382)
- Limit default partition size (#398)
- Fix Windows installation (#399)
- Reword elf too big error (#400)

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

### Removed



## [2.0.0-rc.3] - 2023-01-12

## [2.0.0-rc.2] - 2022-12-07

## [2.0.0-rc.1] - 2022-11-07

## [2.0.0-rc.1] - 2022-11-07

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

[unreleased]: https://github.com/esp-rs/espflash/compare/v2.0.0-rc.3...HEAD
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



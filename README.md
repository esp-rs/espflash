# espflash

![GitHub Workflow Status](https://img.shields.io/github/actions/workflow/status/esp-rs/espflash/ci.yml?branch=main&labelColor=1C2C2E&logo=github&style=flat-square)
![Crates.io](https://img.shields.io/crates/l/espflash?labelColor=1C2C2E&style=flat-square)
[![Matrix](https://img.shields.io/matrix/esp-rs:matrix.org?label=join%20matrix&color=BEC5C9&labelColor=1C2C2E&logo=matrix&style=flat-square)](https://matrix.to/#/#esp-rs:matrix.org)

Serial flasher utilities for Espressif devices, based loosely on [esptool.py](https://github.com/espressif/esptool/).

Supports the **ESP32**, **ESP32-C2/C3/C6**, **ESP32-H2**, **ESP32-P4**, **ESP32-S2/S3**, and **ESP8266**.

## [cargo-espflash](./cargo-espflash/)

A cargo extension for flashing Espressif devices.

For more information and installation instructions, please refer to the `cargo-espflash` package's [README](./cargo-espflash/README.md).

## [espflash](./espflash/)

A library and command-line tool for flashing Espressif devices.

For more information and installation instructions, please refer to the `espflash` package's [README](./espflash/README.md).

## Git Hooks

We provide a simple `pre-commit` hook to verify the formatting of each package prior to committing changes. This can be enabled by placing it in the `.git/hooks/` directory:

```bash
$ cp pre-commit .git/hooks/pre-commit
```

When using this hook, you can choose to ignore its failure on a per-commit basis by committing with the `--no-verify` flag; however, you will need to be sure that all packages are formatted when submitting a pull request.

## License

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](./LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
- MIT license ([LICENSE-MIT](./LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in
the work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without
any additional terms or conditions.

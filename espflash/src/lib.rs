//! A library and application for flashing Espressif devices over Serial
//!
//! ## As an application
//!
//! [espflash] can be installed using `cargo install`, and additionally supports installation via [cargo-binstall]:
//!
//! ```bash
//! $ cargo install espflash
//! $ cargo binstall espflash
//! ```
//!
//! ## As a library
//!
//! [espflash] can also be used as a library:
//!
//! ```toml
//! espflash = { version = "2.1", default-features = false }
//! ```
//!
//! We add `default-features` here to disable the `cli` feature, which is
//! enabled by default. Its important to note that the cli module does not
//! provide SemVer guarantees. You likely will not need any of these types or functions
//! in your application so there's no use pulling in the extra dependencies.
//!
//! [espflash]: https://crates.io/crates/espflash
//! [cargo-binstall]: https://github.com/cargo-bins/cargo-binstall

#![cfg_attr(docsrs, feature(doc_cfg))]

#[cfg(feature = "cli")]
#[cfg_attr(docsrs, doc(cfg(feature = "cli")))]
pub mod cli;
#[cfg(feature = "serialport")]
pub mod command;
#[cfg(feature = "serialport")]
#[cfg_attr(docsrs, doc(cfg(feature = "serialport")))]
pub mod connection;
pub mod elf;
pub mod error;
pub mod flasher;
pub mod image_format;
pub mod targets;

/// Logging utilities
#[cfg(feature = "cli")]
#[cfg_attr(docsrs, doc(cfg(feature = "cli")))]
pub mod logging {
    use env_logger::Env;
    use log::LevelFilter;

    /// Initialize the logger with the given [LevelFilter]
    pub fn initialize_logger(filter: LevelFilter) {
        env_logger::Builder::from_env(Env::default().default_filter_or(filter.as_str()))
            .format_target(false)
            .init();
    }
}

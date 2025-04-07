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
//! espflash = { version = "4.0.0-dev", default-features = false }
//! ```
//!
//! We add `default-features` here to disable the `cli` feature, which is
//! enabled by default. Its important to note that the cli module does not
//! provide SemVer guarantees. You likely will not need any of these types or
//! functions in your application so there's no use pulling in the extra
//! dependencies.
//!
//! [espflash]: https://crates.io/crates/espflash
//! [cargo-binstall]: https://github.com/cargo-bins/cargo-binstall
#![cfg_attr(not(feature = "std"), no_std)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![deny(missing_debug_implementations, rust_2018_idioms)]

pub use self::{error::Error, image_format::Segment};

#[cfg(feature = "serialport")]
#[cfg_attr(docsrs, doc(cfg(feature = "serialport")))]
pub mod connection;

pub mod command;
pub mod flasher;
pub mod image_format;
pub mod slip;
pub mod targets;

mod error;

extern crate alloc;

// Command-line interface
#[cfg(feature = "cli")]
pub mod cli;

// Logging utilities
#[cfg(feature = "cli")]
pub mod logging {
    use env_logger::{Builder, Env};
    use log::LevelFilter;

    /// Initialize the logger with the given [LevelFilter]
    pub fn initialize_logger(filter: LevelFilter) {
        Builder::from_env(Env::default().default_filter_or(filter.as_str()))
            .format_target(false)
            .init();
    }
}

// Check for updates
#[cfg(feature = "cli")]
pub mod update {
    use std::time::Duration;

    use log::info;
    use update_informer::{registry::Crates, Check};

    pub fn check_for_update(name: &str, version: &str) {
        // By setting the interval to 0 seconds we invalidate the cache with each
        // invocation and ensure we're getting up-to-date results
        let informer = update_informer::new(Crates, name, version).interval(Duration::from_secs(0));

        if let Some(version) = informer.check_version().ok().flatten() {
            info!("🚀 A new version of {name} is available: {version}");
        }
    }
}

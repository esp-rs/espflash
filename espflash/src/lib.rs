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

/// Check for updates
#[cfg(feature = "cli")]
#[cfg_attr(docsrs, doc(cfg(feature = "cli")))]
pub mod update {
    use std::time::Duration;

    use log::info;
    use update_informer::{registry, Check};

    /// Check crates.io for a new version of the application
    pub fn check_for_update(name: &str, version: &str) {
        // By setting the interval to 0 seconds we invalidate the cache with each
        // invocation and ensure we're getting up-to-date results
        let informer =
            update_informer::new(registry::Crates, name, version).interval(Duration::from_secs(0));

        if let Some(version) = informer.check_version().ok().flatten() {
            info!("🚀 A new version of {name} is available: {version}");
        }
    }
}

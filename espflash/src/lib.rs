#[cfg(feature = "cli")]
pub mod cli;
pub mod connection;
pub mod elf;
pub mod error;
pub mod flasher;
pub mod image_format;
pub mod targets;

mod command;
mod interface;

pub mod logging {
    use env_logger::Env;
    use log::LevelFilter;

    pub fn initialize_logger(filter: LevelFilter) {
        env_logger::Builder::from_env(Env::default().default_filter_or(filter.as_str()))
            .format_target(false)
            .init();
    }
}

#[cfg(feature = "cli")]
pub mod update {
    use std::time::Duration;

    use log::info;
    use update_informer::{registry, Check};

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

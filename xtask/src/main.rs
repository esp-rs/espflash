use std::{env, path::PathBuf};

use clap::Parser;

// Import modules
mod bootloader_builder;
#[cfg(feature = "efuse-generator")]
mod efuse_generator;
mod test_runner;

// Type definition for results
pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

// ----------------------------------------------------------------------------
// Command-line Interface

#[derive(Debug, Parser)]
enum Cli {
    /// Build bundled ESP-IDF bootloader binaries
    #[command(visible_alias = "bootloaders")]
    BuildBootloaders(bootloader_builder::BuildBootloadersArgs),

    /// Generate eFuse field definitions
    #[cfg(feature = "efuse-generator")]
    GenerateEfuseFields(efuse_generator::GenerateEfuseFieldsArgs),

    /// Run espflash tests
    RunTests(test_runner::RunTestsArgs),
}

// ----------------------------------------------------------------------------
// Application

fn main() -> Result<()> {
    env_logger::Builder::new()
        .filter_module("xtask", log::LevelFilter::Info)
        .init();

    // Prefer the checkout containing the current directory. A distributed
    // xtask binary retains its build machine's CARGO_MANIFEST_DIR, and that path
    // can accidentally exist (but refer to a different checkout) on a
    // self-hosted runner.
    let current_dir = env::current_dir()?.canonicalize()?;
    let workspace_from_build = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("`CARGO_MANIFEST_DIR` should always have a parent")
        .to_path_buf();

    let workspace_from_cwd = current_dir
        .ancestors()
        .find(|path| path.join("Cargo.toml").is_file() && path.join("xtask/Cargo.toml").is_file())
        .map(PathBuf::from);
    let workspace = if let Some(workspace) = workspace_from_cwd {
        workspace
    } else if workspace_from_build.exists() {
        workspace_from_build.canonicalize()?
    } else {
        current_dir
    };

    match Cli::parse() {
        Cli::BuildBootloaders(args) => bootloader_builder::build_bootloaders(&workspace, args),
        #[cfg(feature = "efuse-generator")]
        Cli::GenerateEfuseFields(args) => efuse_generator::generate_efuse_fields(&workspace, args),
        Cli::RunTests(args) => test_runner::run_tests(&workspace, args),
    }
}

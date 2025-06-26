use std::path::PathBuf;

use clap::Parser;

// Import modules
mod efuse_generator;
mod test_runner;

// Type definition for results
pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

// ----------------------------------------------------------------------------
// Command-line Interface

#[derive(Debug, Parser)]
enum Cli {
    /// Generate eFuse field definitions
    GenerateEfuseFields(efuse_generator::GenerateEfuseFieldsArgs),

    /// Run espflash tests (replacing bash scripts)
    RunTests(test_runner::RunTestsArgs),
}

// ----------------------------------------------------------------------------
// Application

fn main() -> Result<()> {
    env_logger::Builder::new()
        .filter_module("xtask", log::LevelFilter::Info)
        .init();

    // The directory containing the cargo manifest for the 'xtask' package is a
    // subdirectory within the cargo workspace:
    let workspace = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let workspace = workspace.parent().unwrap().canonicalize()?;

    match Cli::parse() {
        Cli::GenerateEfuseFields(args) => efuse_generator::generate_efuse_fields(&workspace, args),
        Cli::RunTests(args) => test_runner::run_tests(&workspace, args),
    }
}

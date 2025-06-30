use std::{env, path::PathBuf};

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

    // Determine the path to the workspace (i.e. the root of the repository).
    // At compile-time we know where the `xtask` crate lives, but that absolute
    // path may not exist at runtime once the binary is distributed as an
    // artefact and executed on a different machine (e.g. a self-hosted CI
    // runner). Therefore we
    //  1. Try the compile-time location first.
    //  2. Fallback to the current working directory if that fails.

    let workspace_from_build = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("`CARGO_MANIFEST_DIR` should always have a parent")
        .to_path_buf();

    let workspace = if workspace_from_build.exists() {
        workspace_from_build.canonicalize()?
    } else {
        env::current_dir()?.canonicalize()?
    };

    match Cli::parse() {
        Cli::GenerateEfuseFields(args) => efuse_generator::generate_efuse_fields(&workspace, args),
        Cli::RunTests(args) => test_runner::run_tests(&workspace, args),
    }
}

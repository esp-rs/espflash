use std::path::PathBuf;

use cargo_metadata::{MetadataCommand, camino::Utf8PathBuf};
use miette::{IntoDiagnostic, Result};
use serde::Deserialize;

use crate::error::Error;

/// Package metadata.
#[derive(Debug, Default, Clone, Deserialize)]
pub struct PackageMetadata {
    /// Path to the root of the Cargo workspace the package belongs to.
    pub workspace_root: PathBuf,
    /// Path to the directory that contains the `Cargo.toml` for the
    /// selected package.
    pub package_root: PathBuf,
}

impl PackageMetadata {
    /// Load package metadata.
    pub fn load(package_name: &Option<String>) -> Result<Self> {
        // There MUST be a cargo manifest in the executing directory, regardless of
        // whether or not we are in a workspace.
        let manifest_path = PathBuf::from("Cargo.toml");
        if !manifest_path.exists() {
            return Err(Error::NoProject.into());
        }

        let manifest_path = manifest_path.canonicalize().into_diagnostic()?;
        let manifest_path = Utf8PathBuf::from_path_buf(manifest_path).unwrap();

        let metadata = MetadataCommand::new()
            .no_deps()
            .manifest_path(&manifest_path)
            .exec()
            .into_diagnostic()?;

        let maybe_package = if let [package] = metadata.workspace_default_packages()[..] {
            Some(package.to_owned())
        } else {
            metadata
                .packages
                .iter()
                .find(|package| match package_name {
                    Some(name) => package.name.as_str() == *name,
                    None => false,
                })
                .cloned()
        };

        let package = maybe_package.ok_or(Error::NoPackage).into_diagnostic()?;

        let package_metadata = Self {
            workspace_root: metadata.workspace_root.clone().into(),
            package_root: package.manifest_path.parent().unwrap().into(),
        };

        Ok(package_metadata)
    }
}

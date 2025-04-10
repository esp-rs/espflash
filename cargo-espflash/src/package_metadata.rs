use std::path::PathBuf;

use cargo_metadata::{MetadataCommand, camino::Utf8PathBuf};
use miette::{IntoDiagnostic, Result};
use serde::Deserialize;

use crate::error::Error;

#[derive(Debug, Default, Clone, Deserialize)]
pub struct PackageMetadata {
    pub workspace_root: PathBuf,
    pub package_root: PathBuf,
}

impl PackageMetadata {
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
                .find(|package| Some(package.name.clone()) == *package_name)
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

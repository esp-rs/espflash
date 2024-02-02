use std::path::PathBuf;

use cargo::{
    core::{Package, Workspace},
    util::Config,
};
use miette::{IntoDiagnostic, Result};
use serde::Deserialize;

use crate::error::Error;

#[derive(Debug, Default, Clone, Deserialize)]
pub struct PackageMetadata {
    pub workspace_root: PathBuf,
    pub package_root: PathBuf,
}

impl PackageMetadata {
    pub fn load(package_name: &Option<String>) -> Result<PackageMetadata> {
        // There MUST be a cargo manifest in the executing directory, regardless of
        // whether or not we are in a workspace.
        let manifest_path = PathBuf::from("Cargo.toml");
        if !manifest_path.exists() {
            return Err(Error::NoProject.into());
        }

        let manifest_path = manifest_path.canonicalize().into_diagnostic()?;
        let config = Config::default().map_err(|_| Error::InvalidWorkspace)?;

        let workspace =
            Workspace::new(&manifest_path, &config).map_err(|_| Error::InvalidWorkspace)?;

        let package = Self::load_package(&workspace, package_name)?;
        let metadata = Self::load_metadata(&workspace, &package)?;

        Ok(metadata)
    }

    fn load_package(workspace: &Workspace, package_name: &Option<String>) -> Result<Package> {
        // If we are currently in a package (ie. *not* in a workspace) then we can just
        // use this package; otherwise we must try to find a package with the correct
        // name within the current workspace.
        let maybe_package = if let Ok(package) = workspace.current() {
            Some(package)
        } else {
            workspace
                .members()
                .find(|pkg| Some(pkg.name().to_string()) == *package_name)
        };

        match maybe_package {
            Some(package) => Ok(package.to_owned()),
            None => Err(Error::NoPackage.into()),
        }
    }

    fn load_metadata(workspace: &Workspace, package: &Package) -> Result<PackageMetadata> {
        let espflash_meta = PackageMetadata {
            workspace_root: workspace.root_manifest().parent().unwrap().to_path_buf(),
            package_root: package.root().to_path_buf(),
        };

        Ok(espflash_meta)
    }
}

use std::{ffi::OsStr, path::PathBuf, str::FromStr};

use cargo::{
    core::{Package, Workspace},
    util::Config,
};
use espflash::image_format::ImageFormatKind;
use miette::{IntoDiagnostic, Result};
use serde::Deserialize;

use crate::error::Error;

#[derive(Debug, Default, Clone, Deserialize)]
pub struct PackageMetadata {
    pub workspace_root: PathBuf,
    pub package_root: PathBuf,
    pub bootloader: Option<PathBuf>,
    pub format: Option<ImageFormatKind>,
    pub partition_table: Option<PathBuf>,
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

        if let Some(table) = &metadata.partition_table {
            if table.extension() != Some(OsStr::new("csv")) {
                return Err(Error::InvalidPartitionTablePath.into());
            }
        }

        if let Some(bootloader) = &metadata.bootloader {
            if bootloader.extension() != Some(OsStr::new("bin")) {
                return Err(Error::InvalidBootloaderPath.into());
            }
        }

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
        let mut espflash_meta = PackageMetadata {
            workspace_root: workspace.root_manifest().parent().unwrap().to_path_buf(),
            package_root: package.root().to_path_buf(),

            ..PackageMetadata::default()
        };

        match package.manifest().custom_metadata() {
            Some(meta) if meta.is_table() => match meta.as_table().unwrap().get("espflash") {
                Some(meta) if meta.is_table() => {
                    let meta = meta.as_table().unwrap();

                    espflash_meta.bootloader = meta
                        .get("bootloader")
                        .map(|bl| package.root().join(bl.as_str().unwrap()));

                    espflash_meta.format = meta
                        .get("format")
                        .map(|fmt| ImageFormatKind::from_str(fmt.as_str().unwrap()).unwrap());

                    espflash_meta.partition_table = meta
                        .get("partition_table")
                        .map(|pt| package.root().join(pt.as_str().unwrap()));
                }
                _ => {}
            },
            _ => {}
        }

        Ok(espflash_meta)
    }
}

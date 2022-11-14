use std::{
    ffi::OsStr,
    path::{Path, PathBuf},
};

use cargo_toml::Manifest;
use espflash::image_format::ImageFormatKind;
use miette::{IntoDiagnostic, Result, WrapErr};
use serde::Deserialize;

use crate::error::Error;

#[derive(Debug, Default, Clone, Deserialize)]
pub struct CargoEspFlashMeta {
    pub partition_table: Option<PathBuf>,
    pub bootloader: Option<PathBuf>,
    pub format: Option<ImageFormatKind>,
}

#[derive(Debug, Default, Clone, Deserialize)]
pub struct Meta {
    pub espflash: Option<CargoEspFlashMeta>,
}

impl CargoEspFlashMeta {
    pub fn load(package: &Option<String>) -> Result<CargoEspFlashMeta> {
        // If a package was specified and we are building from the root of a workspace,
        // we will instead load the specified package's manifest. Otherwise, load the
        // manifest from the executing directory.
        let cargo_toml = if let Some(package) = package {
            PathBuf::from(package).join("Cargo.toml")
        } else {
            PathBuf::from("Cargo.toml")
        };
        let manifest = Self::load_manifest(&cargo_toml)?;

        let mut meta = manifest
            .package
            .and_then(|pkg| pkg.metadata)
            .unwrap_or_default()
            .espflash
            .unwrap_or_default();

        if let Some(table) = &meta.partition_table {
            if table.extension() != Some(OsStr::new("csv")) {
                return Err(Error::InvalidPartitionTablePath.into());
            }

            // Update the partition table path if we're building a package, but only if it's
            // a relative path.
            if let Some(package) = package {
                if table.is_relative() {
                    meta.partition_table = Some(PathBuf::from(package).join(table));
                }
            }
        }

        if let Some(bootloader) = &meta.bootloader {
            if bootloader.extension() != Some(OsStr::new("bin")) {
                return Err(Error::InvalidBootloaderPath.into());
            }

            // Update the bootloader path if we're building a package, but only if it's a
            // relative path.
            if let Some(package) = package {
                if bootloader.is_relative() {
                    meta.bootloader = Some(PathBuf::from(package).join(bootloader));
                }
            }
        }

        Ok(meta)
    }

    fn load_manifest(cargo_toml: &Path) -> Result<Manifest<Meta>> {
        if !cargo_toml.exists() {
            return Err(Error::NoProject.into());
        }

        let manifest = Manifest::<Meta>::from_path_with_metadata(cargo_toml)
            .into_diagnostic()
            .wrap_err("Failed to parse Cargo.toml")?;

        Ok(manifest)
    }
}

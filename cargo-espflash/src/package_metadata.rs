use std::{
    ffi::OsStr,
    fs::read_to_string,
    path::{Path, PathBuf},
};

use cargo_toml::Manifest;
use espflash::ImageFormatId;
use miette::{IntoDiagnostic, Result, WrapErr};
use serde::Deserialize;

use crate::error::{Error, TomlError};

#[derive(Clone, Debug, Deserialize, Default)]
pub struct CargoEspFlashMeta {
    pub partition_table: Option<PathBuf>,
    pub bootloader: Option<PathBuf>,
    pub format: Option<ImageFormatId>,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct Meta {
    pub espflash: Option<CargoEspFlashMeta>,
}

impl CargoEspFlashMeta {
    pub fn load<P>(manifest: P) -> Result<CargoEspFlashMeta>
    where
        P: AsRef<Path>,
    {
        let manifest = manifest.as_ref();
        if !manifest.exists() {
            return Err(Error::NoProject.into());
        }

        let toml = read_to_string(manifest)
            .into_diagnostic()
            .wrap_err("Failed to read Cargo.toml")?;

        let manifest = Manifest::<Meta>::from_slice_with_metadata(toml.as_bytes())
            .map_err(move |e| TomlError::new(e, toml))
            .wrap_err("Failed to parse Cargo.toml")?;

        let meta = manifest
            .package
            .and_then(|pkg| pkg.metadata)
            .unwrap_or_default()
            .espflash
            .unwrap_or_default();

        if let Some(table) = &meta.partition_table {
            if table.extension() != Some(OsStr::new("csv")) {
                return Err(Error::InvalidPartitionTablePath.into());
            }
        }

        if let Some(bootloader) = &meta.bootloader {
            if bootloader.extension() != Some(OsStr::new("bin")) {
                return Err(Error::InvalidBootloaderPath.into());
            }
        }

        Ok(meta)
    }
}

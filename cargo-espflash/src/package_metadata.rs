use std::{
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
    pub fn load<P: AsRef<Path>>(path: P) -> Result<CargoEspFlashMeta> {
        let path = path.as_ref();
        if !path.exists() {
            return Err(Error::NoProject.into());
        }
        let toml = read_to_string(path)
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
        match meta.partition_table {
            Some(table) if !table.ends_with(".csv") => {
                return Err(Error::InvalidPartitionTablePath.into())
            }
            _ => {}
        }
        match meta.bootloader {
            Some(table) if !table.ends_with(".bin") => {
                return Err(Error::InvalidBootloaderPath.into())
            }
            _ => {}
        }
        Ok(meta)
    }
}

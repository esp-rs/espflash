use std::{
    fs,
    path::{Path, PathBuf},
};

use miette::{Result, WrapErr};
use serde::Deserialize;

use crate::error::TomlError;

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Unstable {
    #[serde(default)]
    build_std: Vec<String>,
}

#[derive(Debug, Default, Deserialize)]
pub struct Build {
    target: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
pub struct CargoConfig {
    #[serde(default)]
    unstable: Unstable,
    #[serde(default)]
    build: Build,
}

impl CargoConfig {
    pub fn load(workspace_root: &Path, package_root: &Path) -> Self {
        // If there is a Cargo configuration file in the current package, we will
        // deserialize and return it.
        // If the package is in a workspace and a Cargo configuration file is present in
        // that workspace we will deserialize and return that one instead.
        // Otherwise, there is no configuration present so we will return `None`.
        if let Ok(Some(package_config)) = load_cargo_config(package_root) {
            package_config
        } else if let Ok(Some(workspace_config)) = load_cargo_config(workspace_root) {
            workspace_config
        } else {
            Self::default()
        }
    }

    pub fn has_build_std(&self) -> bool {
        !self.unstable.build_std.is_empty()
    }

    pub fn target(&self) -> Option<&str> {
        self.build.target.as_deref()
    }
}

fn load_cargo_config(path: &Path) -> Result<Option<CargoConfig>> {
    let config_path = match config_path(path) {
        Some(path) => path,
        None => {
            return Ok(None);
        }
    };

    let content = match fs::read_to_string(&config_path) {
        Ok(content) => content,
        Err(_) => return Ok(None),
    };

    let config = toml::from_str(&content)
        .map_err(move |e| TomlError::new(e, content))
        .wrap_err_with(|| {
            format!(
                "Failed to parse {}",
                &config_path.as_path().to_string_lossy()
            )
        })?;

    Ok(Some(config))
}

fn config_path(path: &Path) -> Option<PathBuf> {
    // Support for the .toml extension was added in version 1.39 and is the
    // preferred form. If both files exist, Cargo will use the file without the
    // extension.
    // https://doc.rust-lang.org/cargo/reference/config.html
    let bare = path.join(".cargo/config");
    if bare.exists() {
        return Some(bare);
    }

    let toml = path.join(".cargo/config.toml");
    if toml.exists() {
        Some(toml)
    } else {
        None
    }
}

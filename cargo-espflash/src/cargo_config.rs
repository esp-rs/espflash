use crate::error::TomlError;
use miette::{Result, WrapErr};
use serde::Deserialize;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Deserialize, Default)]
pub struct CargoConfig {
    #[serde(default)]
    build: Build,
}

impl CargoConfig {
    pub fn target(&self) -> Option<&str> {
        self.build.target.as_deref()
    }
}

#[derive(Debug, Default, Deserialize)]
pub struct Build {
    target: Option<String>,
}

pub fn parse_cargo_config<P: AsRef<Path>>(project_path: P) -> Result<CargoConfig> {
    let config_path = match config_path(project_path.as_ref()) {
        Some(path) => path,
        None => {
            return Ok(CargoConfig::default());
        }
    };
    let content = match fs::read_to_string(&config_path) {
        Ok(content) => content,
        Err(_) => return Ok(CargoConfig::default()),
    };
    toml::from_str(&content)
        .map_err(move |e| TomlError::new(e, content))
        .wrap_err_with(|| {
            format!(
                "Failed to parse {}",
                &config_path.as_path().to_string_lossy()
            )
        })
}

fn config_path(project_path: &Path) -> Option<PathBuf> {
    let bare = project_path.join(".cargo/config");
    if bare.exists() {
        return Some(bare);
    }
    let toml = project_path.join(".cargo/config.toml");
    if toml.exists() {
        Some(toml)
    } else {
        None
    }
}

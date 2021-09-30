use crate::error::TomlError;
use miette::{Result, WrapErr};
use serde::Deserialize;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Deserialize)]
struct CargoConfig {
    #[serde(default)]
    unstable: Unstable,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "kebab-case")]
struct Unstable {
    #[serde(default)]
    build_std: Vec<String>,
}

/// Check if the build-std option seems to be set correctly
pub fn has_build_std<P: AsRef<Path>>(project_path: P) -> Result<bool> {
    let config_path = match config_path(project_path.as_ref()) {
        Some(path) => path,
        None => {
            return Ok(false);
        }
    };
    let content = match fs::read_to_string(&config_path) {
        Ok(content) => content,
        Err(_) => return Ok(false),
    };
    let toml: CargoConfig = toml::from_str(&content)
        .map_err(move |e| TomlError::new(e, content))
        .wrap_err_with(|| {
            format!(
                "Failed to parse {}",
                &config_path.as_path().to_string_lossy()
            )
        })?;

    Ok(!toml.unstable.build_std.is_empty())
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

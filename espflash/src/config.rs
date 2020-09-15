use directories_next::ProjectDirs;
use serde::Deserialize;
use std::fs::read;

#[derive(Debug, Deserialize, Default)]
pub struct Config {
    #[serde(default)]
    pub connection: Connection,
    #[serde(default)]
    pub build: Build,
}

#[derive(Debug, Deserialize, Default)]
pub struct Connection {
    pub serial: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
pub struct Build {
    pub tool: Option<String>,
}

impl Config {
    /// Load the config from config file
    pub fn load() -> Self {
        let dirs = ProjectDirs::from("rs", "esp", "espflash").unwrap();
        let file = dirs.config_dir().join("espflash.toml");
        if let Ok(data) = read(&file) {
            toml::from_slice(&data).unwrap()
        } else {
            Self::default()
        }
    }
}

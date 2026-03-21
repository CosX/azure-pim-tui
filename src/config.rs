use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize)]
pub struct Config {
    #[serde(default = "default_justification")]
    pub default_justification: String,
    #[serde(default = "default_duration_hours")]
    pub default_duration_hours: u32,
    #[serde(default = "default_auto_refresh_secs")]
    pub auto_refresh_secs: u64,
}

fn default_justification() -> String {
    "Local development".to_string()
}

fn default_duration_hours() -> u32 {
    8
}

fn default_auto_refresh_secs() -> u64 {
    60
}

impl Default for Config {
    fn default() -> Self {
        Self {
            default_justification: default_justification(),
            default_duration_hours: default_duration_hours(),
            auto_refresh_secs: default_auto_refresh_secs(),
        }
    }
}

impl Config {
    pub fn config_path() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("azure-pim-tui")
            .join("config.toml")
    }

    pub fn load() -> Result<Self> {
        let path = Self::config_path();
        if path.exists() {
            let content = std::fs::read_to_string(&path)?;
            let config: Config = toml::from_str(&content)?;
            Ok(config)
        } else {
            let config = Config::default();
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let content = toml::to_string_pretty(&config)?;
            std::fs::write(&path, content)?;
            Ok(config)
        }
    }
}

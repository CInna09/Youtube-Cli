/// Config file — ~/.config/rustyoutube-cli/config.toml

use std::path::PathBuf;

use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub socket: Option<String>,
    pub mpv_bin: Option<String>,
    pub ytdlp_bin: Option<String>,
    pub default_volume: Option<u8>,
}

impl Config {
    /// Load config from default path (~/.config/rustyoutube-cli/config.toml).
    /// Returns `None` if file doesn't exist or can't be parsed (silent).
    pub fn load() -> Option<Self> {
        let path = Self::path()?;
        let content = std::fs::read_to_string(path).ok()?;
        toml::from_str(&content).ok()
    }

    fn path() -> Option<PathBuf> {
        let home = std::env::var("HOME").ok()?;
        Some(
            PathBuf::from(home)
                .join(".config")
                .join("rustyoutube-cli")
                .join("config.toml"),
        )
    }
}

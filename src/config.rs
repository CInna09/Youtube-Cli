/// Config file — ~/.config/rustyoutube-cli/config.toml
/// State (volume persist) — ~/.config/rustyoutube-cli/state.toml

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub socket: Option<String>,
    pub mpv_bin: Option<String>,
    pub ytdlp_bin: Option<String>,
    pub default_volume: Option<u8>,
}

/// State yang di-persist otomatis (volume, dll)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct State {
    pub volume: u8,
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
        Some(Self::dir().join("config.toml"))
    }

    fn dir() -> PathBuf {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
        PathBuf::from(home).join(".config").join("rustyoutube-cli")
    }

    /// Load saved state (volume persist).
    pub fn load_state() -> Option<State> {
        let path = Self::dir().join("state.toml");
        let content = std::fs::read_to_string(path).ok()?;
        toml::from_str(&content).ok()
    }

    /// Save state (volume persist). Silent — gak panic kalau gagal.
    pub fn save_state(state: &State) {
        let dir = Self::dir();
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("state.toml");
        if let Ok(content) = toml::to_string(state) {
            let _ = std::fs::write(path, content);
        }
    }
}

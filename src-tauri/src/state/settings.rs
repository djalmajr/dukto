use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

const SETTINGS_FILE: &str = "settings.json";

/// Theme preference.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ThemeMode {
    Light,
    Dark,
    System,
}

impl Default for ThemeMode {
    fn default() -> Self {
        Self::System
    }
}

/// Persisted user settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    pub destination_dir: String,
    #[serde(default)]
    pub theme: ThemeMode,
    #[serde(default)]
    pub peer_order: Vec<String>,
}

impl Default for Settings {
    fn default() -> Self {
        #[cfg(target_os = "ios")]
        let default_dir = dirs::document_dir()
            .or_else(dirs::home_dir)
            .unwrap_or_else(|| PathBuf::from("."));

        #[cfg(not(target_os = "ios"))]
        let default_dir = dirs::download_dir()
            .or_else(dirs::home_dir)
            .unwrap_or_else(|| PathBuf::from("."));

        Self {
            destination_dir: default_dir.to_string_lossy().to_string(),
            theme: ThemeMode::default(),
            peer_order: Vec::new(),
        }
    }
}

impl Settings {
    /// Load settings from data_dir, or return defaults if file doesn't exist.
    pub fn load(data_dir: &Path) -> Self {
        let path = data_dir.join(SETTINGS_FILE);
        match std::fs::read_to_string(&path) {
            Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
            Err(_) => Self::default(),
        }
    }

    /// Save settings to data_dir.
    pub fn save(&self, data_dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
        std::fs::create_dir_all(data_dir)?;
        let path = data_dir.join(SETTINGS_FILE);
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(path, json)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(suffix: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("duto-settings-{}-{}", suffix, uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn default_settings_has_destination_dir() {
        let s = Settings::default();
        assert!(!s.destination_dir.is_empty());
    }

    #[test]
    fn save_and_load_roundtrip() {
        let dir = temp_dir("roundtrip");
        let settings = Settings {
            destination_dir: "/tmp/my-downloads".into(),
            theme: ThemeMode::Dark,
            peer_order: vec!["linux".into(), "mac".into()],
        };

        settings.save(&dir).unwrap();
        let loaded = Settings::load(&dir);
        assert_eq!(loaded.destination_dir, "/tmp/my-downloads");
        assert_eq!(loaded.theme, ThemeMode::Dark);
        assert_eq!(loaded.peer_order, vec!["linux", "mac"]);

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn default_theme_is_system() {
        let s = Settings::default();
        assert_eq!(s.theme, ThemeMode::System);
    }

    #[test]
    fn load_old_settings_without_theme_defaults_to_system() {
        let dir = temp_dir("no-theme");
        std::fs::write(dir.join(SETTINGS_FILE), r#"{"destination_dir":"/tmp"}"#).unwrap();
        let loaded = Settings::load(&dir);
        assert_eq!(loaded.theme, ThemeMode::System);
        assert!(loaded.peer_order.is_empty());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn load_missing_file_returns_defaults() {
        let dir = temp_dir("missing");
        let loaded = Settings::load(&dir);
        assert!(!loaded.destination_dir.is_empty());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn load_corrupt_file_returns_defaults() {
        let dir = temp_dir("corrupt");
        std::fs::write(dir.join(SETTINGS_FILE), "not json!!!").unwrap();
        let loaded = Settings::load(&dir);
        assert!(!loaded.destination_dir.is_empty());
        std::fs::remove_dir_all(&dir).ok();
    }
}

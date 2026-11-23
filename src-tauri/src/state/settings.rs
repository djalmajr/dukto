use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

const SETTINGS_FILE: &str = "settings.json";

/// Theme preference.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum ThemeMode {
    Light,
    Dark,
    #[default]
    System,
}

/// Persisted user settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    pub destination_dir: String,
    #[serde(default)]
    pub display_name: Option<String>,
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

        #[cfg(target_os = "android")]
        let default_dir = dirs::download_dir()
            .or_else(|| Some(PathBuf::from("/storage/emulated/0/Download")))
            .unwrap_or_else(|| PathBuf::from("."));

        #[cfg(not(any(target_os = "ios", target_os = "android")))]
        let default_dir = dirs::download_dir()
            .or_else(dirs::home_dir)
            .unwrap_or_else(|| PathBuf::from("."));

        Self {
            destination_dir: default_dir.to_string_lossy().to_string(),
            display_name: None,
            theme: ThemeMode::default(),
            peer_order: Vec::new(),
        }
    }
}

impl Settings {
    /// Load settings from data_dir, or return defaults if file doesn't exist.
    pub fn load(data_dir: &Path) -> Self {
        let path = data_dir.join(SETTINGS_FILE);
        #[allow(unused_mut)]
        let mut settings = match std::fs::read_to_string(&path) {
            Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
            Err(_) => Self::default(),
        };

        // iOS changes the app-container UUID when it recreates the sandbox.
        // A persisted Documents URL must follow the current container, or the
        // receiver cannot prepare its destination before showing approval.
        #[cfg(target_os = "ios")]
        if let Some(documents) = dirs::document_dir() {
            if let Some(rebased) = rebase_ios_documents_path(&settings.destination_dir, &documents)
            {
                settings.destination_dir = rebased.to_string_lossy().into_owned();
                if let Err(error) = settings.save(data_dir) {
                    tracing::warn!(%error, "Could not persist the current iOS Documents path");
                }
            }
        }

        settings
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

#[cfg(any(target_os = "ios", test))]
fn rebase_ios_documents_path(configured: &str, current_documents: &Path) -> Option<PathBuf> {
    let remainder = [
        "/private/var/mobile/Containers/Data/Application/",
        "/var/mobile/Containers/Data/Application/",
    ]
    .iter()
    .find_map(|prefix| configured.strip_prefix(prefix))?;
    let (container_id, suffix) = remainder.split_once("/Documents")?;
    if container_id.is_empty() || container_id.contains('/') {
        return None;
    }
    if !suffix.is_empty() && !suffix.starts_with('/') {
        return None;
    }
    let rebased = if let Some(relative) = suffix.strip_prefix('/') {
        let relative = Path::new(relative);
        if !relative
            .components()
            .all(|component| matches!(component, std::path::Component::Normal(_)))
        {
            return None;
        }
        current_documents.join(relative)
    } else {
        current_documents.to_path_buf()
    };
    (rebased != Path::new(configured)).then_some(rebased)
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
    fn ios_documents_path_follows_new_app_container() {
        let current = Path::new("/private/var/mobile/Containers/Data/Application/NEW/Documents");
        let old = "/private/var/mobile/Containers/Data/Application/OLD/Documents/Received";
        assert_eq!(
            rebase_ios_documents_path(old, current),
            Some(current.join("Received"))
        );
        assert_eq!(
            rebase_ios_documents_path(current.to_str().unwrap(), current),
            None
        );
    }

    #[test]
    fn ios_documents_path_does_not_rewrite_other_destinations() {
        let current = Path::new("/private/var/mobile/Containers/Data/Application/NEW/Documents");
        assert_eq!(rebase_ios_documents_path("/tmp/Shared", current), None);
        assert_eq!(
            rebase_ios_documents_path(
                "/private/var/mobile/Containers/Data/Application/OLD/Library",
                current
            ),
            None
        );
        assert_eq!(
            rebase_ios_documents_path(
                "/private/var/mobile/Containers/Data/Application/OLD/DocumentsBackup",
                current
            ),
            None
        );
        assert_eq!(
            rebase_ios_documents_path(
                "/private/var/mobile/Containers/Data/Application/OLD/Documents/../Library",
                current
            ),
            None
        );
    }

    #[test]
    fn save_and_load_roundtrip() {
        let dir = temp_dir("roundtrip");
        let settings = Settings {
            destination_dir: "/tmp/my-downloads".into(),
            display_name: Some("Djalma's phone".into()),
            theme: ThemeMode::Dark,
            peer_order: vec!["linux".into(), "mac".into()],
        };

        settings.save(&dir).unwrap();
        let loaded = Settings::load(&dir);
        assert_eq!(loaded.destination_dir, "/tmp/my-downloads");
        assert_eq!(loaded.display_name.as_deref(), Some("Djalma's phone"));
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
        assert!(loaded.display_name.is_none());
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

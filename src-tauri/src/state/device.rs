use serde::{Deserialize, Serialize};
use std::path::Path;

const DEVICE_ID_FILE: &str = "device_id.txt";

/// Stable identity for this device instance.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceIdentity {
    pub device_id: String,
    pub display_name: String,
    pub hostname: String,
    pub platform: String,
}

impl DeviceIdentity {
    /// Load existing identity from `data_dir` or create a new one.
    pub fn load_or_create(data_dir: &Path) -> Result<Self, Box<dyn std::error::Error>> {
        std::fs::create_dir_all(data_dir)?;

        let id_path = data_dir.join(DEVICE_ID_FILE);
        let device_id = if id_path.exists() {
            std::fs::read_to_string(&id_path)?.trim().to_string()
        } else {
            let id = uuid::Uuid::new_v4().to_string();
            std::fs::write(&id_path, &id)?;
            id
        };

        Ok(Self {
            device_id,
            display_name: whoami::username(),
            hostname: whoami::fallible::hostname().unwrap_or_else(|_| "unknown".into()),
            platform: platform_name(),
        })
    }
}

fn platform_name() -> String {
    match std::env::consts::OS {
        "macos" => "macos",
        "windows" => "windows",
        "linux" => "linux",
        other => other,
    }
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn temp_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("duto-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn load_or_create_generates_valid_uuid() {
        let dir = temp_dir();
        let identity = DeviceIdentity::load_or_create(&dir).unwrap();

        // device_id must be a valid UUID v4
        let parsed = uuid::Uuid::parse_str(&identity.device_id).unwrap();
        assert_eq!(parsed.get_version(), Some(uuid::Version::Random));

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn load_or_create_persists_device_id() {
        let dir = temp_dir();

        let first = DeviceIdentity::load_or_create(&dir).unwrap();
        let second = DeviceIdentity::load_or_create(&dir).unwrap();

        assert_eq!(first.device_id, second.device_id);

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn load_or_create_collects_system_info() {
        let dir = temp_dir();
        let identity = DeviceIdentity::load_or_create(&dir).unwrap();

        assert!(
            !identity.display_name.is_empty(),
            "display_name must not be empty"
        );
        assert!(!identity.hostname.is_empty(), "hostname must not be empty");
        assert!(
            ["macos", "windows", "linux"].contains(&identity.platform.as_str()),
            "platform must be macos, windows, or linux, got: {}",
            identity.platform
        );

        std::fs::remove_dir_all(&dir).ok();
    }
}

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

        let (display_name, hostname) = system_identity(&device_id);

        Ok(Self {
            device_id,
            display_name,
            hostname,
            platform: platform_name(),
        })
    }
}

#[cfg(target_os = "android")]
fn system_identity(device_id: &str) -> (String, String) {
    let model = android_system_property("ro.product.model");
    android_identity(device_id, model.as_deref())
}

#[cfg(target_os = "ios")]
fn system_identity(device_id: &str) -> (String, String) {
    use objc2::MainThreadMarker;
    use objc2_ui_kit::UIDevice;

    let Some(main_thread) = MainThreadMarker::new() else {
        return ios_identity(device_id, None, None);
    };
    let device = UIDevice::currentDevice(main_thread);
    let name = device.name().to_string();
    let model = device.localizedModel().to_string();
    ios_identity(device_id, Some(&name), Some(&model))
}

#[cfg(any(target_os = "android", test))]
fn android_identity(device_id: &str, model: Option<&str>) -> (String, String) {
    let short_id: String = device_id
        .chars()
        .filter(|character| *character != '-')
        .take(8)
        .collect();
    let model = model.map(str::trim).filter(|value| !value.is_empty());
    let hostname = model
        .map(hostname_label)
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| format!("android-{short_id}"));

    (
        model.unwrap_or("Android device").to_string(),
        format!("{hostname}.local"),
    )
}

#[cfg(any(target_os = "ios", test))]
fn ios_identity(
    device_id: &str,
    device_name: Option<&str>,
    device_model: Option<&str>,
) -> (String, String) {
    let short_id = short_device_id(device_id);
    let device_name = trimmed_value(device_name);
    let device_model = trimmed_value(device_model);
    let display_name = device_name.or(device_model).unwrap_or("iOS device");
    let hostname = device_name
        .or(device_model)
        .map(hostname_label)
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| format!("ios-{short_id}"));

    (display_name.to_string(), format!("{hostname}.local"))
}

#[cfg(any(target_os = "ios", test))]
fn trimmed_value(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|value| !value.is_empty())
}

#[cfg(any(target_os = "android", target_os = "ios", test))]
fn hostname_label(value: &str) -> String {
    let mut label = String::new();
    let mut previous_was_separator = false;

    for character in value.chars() {
        if character.is_ascii_alphanumeric() {
            label.push(character.to_ascii_lowercase());
            previous_was_separator = false;
        } else if !previous_was_separator && !label.is_empty() {
            label.push('-');
            previous_was_separator = true;
        }
        if label.len() >= 63 {
            break;
        }
    }

    label.trim_end_matches('-').to_string()
}

#[cfg(any(target_os = "ios", test))]
fn short_device_id(device_id: &str) -> String {
    device_id
        .chars()
        .filter(|character| *character != '-')
        .take(8)
        .collect()
}

#[cfg(target_os = "android")]
fn android_system_property(name: &str) -> Option<String> {
    use std::ffi::CString;

    const PROPERTY_VALUE_MAX: usize = 92;
    let name = CString::new(name).ok()?;
    let mut value = [0_u8; PROPERTY_VALUE_MAX];
    // SAFETY: Android guarantees that __system_property_get writes at most
    // PROPERTY_VALUE_MAX bytes and both buffers remain valid for the call.
    let length = unsafe {
        libc::__system_property_get(name.as_ptr(), value.as_mut_ptr().cast::<libc::c_char>())
    };
    if length <= 0 {
        return None;
    }
    std::str::from_utf8(&value[..length as usize])
        .ok()
        .map(str::to_owned)
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
fn system_identity(_device_id: &str) -> (String, String) {
    (
        whoami::username(),
        whoami::fallible::hostname().unwrap_or_else(|_| "unknown".into()),
    )
}

fn platform_name() -> String {
    match std::env::consts::OS {
        "macos" => "macos",
        "windows" => "windows",
        "linux" => "linux",
        "ios" => "ios",
        "android" => "android",
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

    #[test]
    fn android_identity_uses_the_device_model_instead_of_the_installation_id() {
        let (display_name, hostname) =
            android_identity("632226ee-0d60-45b9-956c-ab3da2c0f697", Some("SM-G781B"));

        assert_eq!(display_name, "SM-G781B");
        assert_eq!(hostname, "sm-g781b.local");
    }

    #[test]
    fn android_identity_falls_back_to_a_stable_unique_hostname() {
        let (display_name, hostname) =
            android_identity("632226ee-0d60-45b9-956c-ab3da2c0f697", None);

        assert_eq!(display_name, "Android device");
        assert_eq!(hostname, "android-632226ee.local");
    }

    #[test]
    fn ios_identity_uses_the_user_assigned_device_name() {
        let (display_name, hostname) = ios_identity(
            "632226ee-0d60-45b9-956c-ab3da2c0f697",
            Some("iPhone de Leidiane"),
            Some("iPhone"),
        );

        assert_eq!(display_name, "iPhone de Leidiane");
        assert_eq!(hostname, "iphone-de-leidiane.local");
    }

    #[test]
    fn ios_identity_falls_back_to_the_device_model() {
        let (display_name, hostname) = ios_identity(
            "632226ee-0d60-45b9-956c-ab3da2c0f697",
            Some("  "),
            Some("iPhone"),
        );

        assert_eq!(display_name, "iPhone");
        assert_eq!(hostname, "iphone.local");
    }

    #[test]
    fn ios_identity_falls_back_to_a_stable_unique_hostname() {
        let (display_name, hostname) =
            ios_identity("632226ee-0d60-45b9-956c-ab3da2c0f697", None, None);

        assert_eq!(display_name, "iOS device");
        assert_eq!(hostname, "ios-632226ee.local");
    }
}

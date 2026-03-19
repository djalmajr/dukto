use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::IpAddr;

pub const SERVICE_TYPE: &str = "_dukto._tcp.local.";
pub const PROTOCOL_VERSION: &str = "0.1";

pub const TXT_PROTOCOL_VERSION: &str = "proto_ver";
pub const TXT_DEVICE_ID: &str = "device_id";
pub const TXT_DISPLAY_NAME: &str = "display_name";
pub const TXT_HOSTNAME: &str = "hostname";
pub const TXT_PLATFORM: &str = "platform";

/// A peer discovered on the local network.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerInfo {
    pub device_id: String,
    pub display_name: String,
    pub hostname: String,
    pub platform: String,
    pub addresses: Vec<IpAddr>,
    pub port: u16,
    pub protocol_version: String,
}

impl PeerInfo {
    /// Build a PeerInfo from mDNS TXT record properties.
    /// Returns None if required fields are missing.
    pub fn from_txt_records(
        properties: &HashMap<String, String>,
        addresses: Vec<IpAddr>,
        port: u16,
    ) -> Option<Self> {
        Some(Self {
            device_id: properties.get(TXT_DEVICE_ID)?.clone(),
            display_name: properties.get(TXT_DISPLAY_NAME)?.clone(),
            hostname: properties.get(TXT_HOSTNAME)?.clone(),
            platform: properties.get(TXT_PLATFORM)?.clone(),
            protocol_version: properties
                .get(TXT_PROTOCOL_VERSION)
                .cloned()
                .unwrap_or_else(|| "unknown".into()),
            addresses,
            port,
        })
    }

    /// Build a TXT record HashMap from this PeerInfo, suitable for mDNS registration.
    pub fn to_txt_properties(&self) -> Vec<(String, String)> {
        vec![
            (TXT_PROTOCOL_VERSION.into(), self.protocol_version.clone()),
            (TXT_DEVICE_ID.into(), self.device_id.clone()),
            (TXT_DISPLAY_NAME.into(), self.display_name.clone()),
            (TXT_HOSTNAME.into(), self.hostname.clone()),
            (TXT_PLATFORM.into(), self.platform.clone()),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_txt() -> HashMap<String, String> {
        let mut m = HashMap::new();
        m.insert(TXT_DEVICE_ID.into(), "abc-123".into());
        m.insert(TXT_DISPLAY_NAME.into(), "Alice".into());
        m.insert(TXT_HOSTNAME.into(), "alice-mac".into());
        m.insert(TXT_PLATFORM.into(), "macos".into());
        m.insert(TXT_PROTOCOL_VERSION.into(), "0.1".into());
        m
    }

    #[test]
    fn from_txt_records_parses_complete_data() {
        let info =
            PeerInfo::from_txt_records(&sample_txt(), vec!["192.168.1.10".parse().unwrap()], 4242)
                .unwrap();

        assert_eq!(info.device_id, "abc-123");
        assert_eq!(info.display_name, "Alice");
        assert_eq!(info.hostname, "alice-mac");
        assert_eq!(info.platform, "macos");
        assert_eq!(info.port, 4242);
        assert_eq!(info.protocol_version, "0.1");
        assert_eq!(info.addresses.len(), 1);
    }

    #[test]
    fn from_txt_records_returns_none_when_device_id_missing() {
        let mut txt = sample_txt();
        txt.remove(TXT_DEVICE_ID);

        let result = PeerInfo::from_txt_records(&txt, vec!["192.168.1.10".parse().unwrap()], 4242);

        assert!(result.is_none());
    }

    #[test]
    fn to_txt_and_back_roundtrip() {
        let original = PeerInfo {
            device_id: "id-1".into(),
            display_name: "Bob".into(),
            hostname: "bob-pc".into(),
            platform: "linux".into(),
            addresses: vec![],
            port: 5000,
            protocol_version: "0.1".into(),
        };

        let props: HashMap<String, String> = original.to_txt_properties().into_iter().collect();
        let rebuilt = PeerInfo::from_txt_records(&props, vec![], 5000).unwrap();

        assert_eq!(original.device_id, rebuilt.device_id);
        assert_eq!(original.display_name, rebuilt.display_name);
        assert_eq!(original.hostname, rebuilt.hostname);
        assert_eq!(original.platform, rebuilt.platform);
    }
}

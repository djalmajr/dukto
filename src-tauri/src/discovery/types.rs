use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::IpAddr;

pub const SERVICE_TYPE: &str = "_dukto._tcp.local.";
pub const PROTOCOL_VERSION: &str = "0.2";

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

/// Choose the most reliable IPv4 address advertised by a peer.
///
/// Bonjour may resolve the same iOS device on Wi-Fi and on the USB link. The
/// USB address is IPv4 link-local (169.254/16) and can complete a handshake but
/// disappear during a longer transfer, so a regular LAN address must win even
/// when it was discovered later.
pub fn preferred_ipv4_address(addresses: &[IpAddr]) -> Option<IpAddr> {
    let mut link_local = None;

    for address in addresses {
        let IpAddr::V4(address) = address else {
            continue;
        };
        if address.is_unspecified() || address.is_loopback() || address.is_multicast() {
            continue;
        }
        if address.is_link_local() {
            link_local.get_or_insert(IpAddr::V4(*address));
        } else {
            return Some(IpAddr::V4(*address));
        }
    }

    link_local
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

    #[test]
    fn preferred_ipv4_chooses_lan_over_ios_usb_link_local() {
        let addresses = vec![
            "169.254.195.158".parse().unwrap(),
            "192.168.0.6".parse().unwrap(),
        ];

        assert_eq!(
            preferred_ipv4_address(&addresses),
            Some("192.168.0.6".parse().unwrap())
        );
    }

    #[test]
    fn preferred_ipv4_keeps_link_local_as_a_last_resort() {
        let addresses = vec![
            "fe80::1234".parse().unwrap(),
            "169.254.195.158".parse().unwrap(),
        ];

        assert_eq!(
            preferred_ipv4_address(&addresses),
            Some("169.254.195.158".parse().unwrap())
        );
    }

    #[test]
    fn preferred_ipv4_ignores_addresses_that_cannot_identify_a_remote_peer() {
        let addresses = vec![
            "0.0.0.0".parse().unwrap(),
            "127.0.0.1".parse().unwrap(),
            "224.0.0.251".parse().unwrap(),
        ];

        assert_eq!(preferred_ipv4_address(&addresses), None);
    }
}

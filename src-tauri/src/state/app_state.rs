use dashmap::DashMap;
use iroh::SecretKey;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use super::device::DeviceIdentity;
use super::internet_session::RemoteSessionRegistry;
use super::settings::Settings;
use crate::discovery::types::PeerInfo;
use crate::internet::endpoint::InternetEndpoint;
use crate::transfer::cancellation::TransferRegistry;

/// Info about an active or completed transfer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransferInfo {
    pub transfer_id: String,
}

/// Centralized runtime state shared across Tauri commands.
pub struct AppState {
    device: Mutex<DeviceIdentity>,
    pub peers: DashMap<String, PeerInfo>,
    pub transfers: DashMap<String, TransferInfo>,
    pub settings: Mutex<Settings>,
    pub data_dir: PathBuf,
    pub transfer_registry: Arc<TransferRegistry>,
    pub remote_sessions: Arc<RemoteSessionRegistry>,
    pub internet_secret_key: SecretKey,
    pub internet_endpoint: tokio::sync::OnceCell<InternetEndpoint>,
}

impl AppState {
    pub fn new(mut device: DeviceIdentity, data_dir: PathBuf) -> Self {
        let settings = Settings::load(&data_dir);
        if let Some(display_name) = settings
            .display_name
            .as_deref()
            .map(str::trim)
            .filter(|name| !name.is_empty())
        {
            device.display_name = display_name.to_string();
        }
        Self {
            device: Mutex::new(device),
            peers: DashMap::new(),
            transfers: DashMap::new(),
            settings: Mutex::new(settings),
            data_dir,
            transfer_registry: Arc::new(TransferRegistry::default()),
            remote_sessions: Arc::new(RemoteSessionRegistry::default()),
            internet_secret_key: SecretKey::generate(),
            internet_endpoint: tokio::sync::OnceCell::new(),
        }
    }

    pub fn get_settings(&self) -> Settings {
        self.settings.lock().unwrap().clone()
    }

    pub fn get_device(&self) -> DeviceIdentity {
        self.device.lock().unwrap().clone()
    }

    pub fn update_display_name(
        &self,
        display_name: &str,
    ) -> Result<DeviceIdentity, Box<dyn std::error::Error>> {
        let display_name = display_name.trim();
        if display_name.is_empty() {
            return Err("Display name cannot be empty".into());
        }
        if display_name.chars().count() > 64 {
            return Err("Display name cannot exceed 64 characters".into());
        }
        if display_name.chars().any(char::is_control) {
            return Err("Display name cannot contain control characters".into());
        }

        self.update_settings(|settings| {
            settings.display_name = Some(display_name.to_string());
        })?;

        let mut device = self.device.lock().unwrap();
        device.display_name = display_name.to_string();
        Ok(device.clone())
    }

    pub fn update_settings<F>(&self, f: F) -> Result<(), Box<dyn std::error::Error>>
    where
        F: FnOnce(&mut Settings),
    {
        let mut settings = self.settings.lock().unwrap();
        f(&mut settings);
        settings.save(&self.data_dir)?;
        Ok(())
    }

    /// Insert or update a peer from discovery.
    pub fn upsert_peer(&self, mut peer: PeerInfo) {
        match self.peers.entry(peer.device_id.clone()) {
            dashmap::mapref::entry::Entry::Occupied(mut entry) => {
                for address in &entry.get().addresses {
                    if !peer.addresses.contains(address) {
                        peer.addresses.push(*address);
                    }
                }
                entry.insert(peer);
            }
            dashmap::mapref::entry::Entry::Vacant(entry) => {
                entry.insert(peer);
            }
        }
    }

    /// Remove a peer by fullname (mDNS service name contains device_id).
    pub fn remove_peer_by_fullname(&self, fullname: &str) {
        self.peers
            .retain(|device_id, _| !fullname.contains(device_id.as_str()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::IpAddr;

    fn temp_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("duto-appstate-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn make_device() -> DeviceIdentity {
        DeviceIdentity {
            device_id: "test-id".into(),
            display_name: "test-user".into(),
            hostname: "test-host".into(),
            platform: "macos".into(),
        }
    }

    fn make_state() -> AppState {
        AppState::new(make_device(), temp_dir())
    }

    fn make_peer(id: &str, name: &str) -> PeerInfo {
        PeerInfo {
            device_id: id.into(),
            display_name: name.into(),
            hostname: format!("{}.local", id),
            platform: "macos".into(),
            addresses: vec!["192.168.1.10".parse::<IpAddr>().unwrap()],
            port: 4242,
            protocol_version: "0.1".into(),
        }
    }

    #[test]
    fn app_state_holds_device_identity() {
        let state = make_state();
        assert_eq!(state.get_device().device_id, "test-id");
    }

    #[test]
    fn app_state_peers_starts_empty() {
        let state = make_state();
        assert!(state.peers.is_empty());
    }

    #[test]
    fn upsert_peer_inserts_new() {
        let state = make_state();
        state.upsert_peer(make_peer("peer-1", "Alice"));

        assert_eq!(state.peers.len(), 1);
        assert_eq!(state.peers.get("peer-1").unwrap().display_name, "Alice");
    }

    #[test]
    fn upsert_peer_updates_existing() {
        let state = make_state();
        state.upsert_peer(make_peer("peer-1", "Alice"));
        state.upsert_peer(make_peer("peer-1", "Alice Updated"));

        assert_eq!(state.peers.len(), 1);
        assert_eq!(
            state.peers.get("peer-1").unwrap().display_name,
            "Alice Updated"
        );
    }

    #[test]
    fn upsert_peer_preserves_addresses_from_previous_mdns_resolutions() {
        let state = make_state();
        let mut ipv4_peer = make_peer("peer-1", "Alice");
        ipv4_peer.addresses = vec!["192.168.1.10".parse().unwrap()];
        state.upsert_peer(ipv4_peer);

        let mut ipv6_peer = make_peer("peer-1", "Alice Updated");
        ipv6_peer.addresses = vec!["fe80::1234".parse().unwrap()];
        state.upsert_peer(ipv6_peer);

        let peer = state.peers.get("peer-1").unwrap();
        assert_eq!(peer.display_name, "Alice Updated");
        assert_eq!(
            peer.addresses,
            vec![
                "fe80::1234".parse::<IpAddr>().unwrap(),
                "192.168.1.10".parse::<IpAddr>().unwrap(),
            ]
        );
    }

    #[test]
    fn remove_peer_by_fullname_works() {
        let state = make_state();
        state.upsert_peer(make_peer("peer-1", "Alice"));
        state.upsert_peer(make_peer("peer-2", "Bob"));

        assert_eq!(state.peers.len(), 2);

        state.remove_peer_by_fullname("peer-1._duto._tcp.local.");
        assert_eq!(state.peers.len(), 1);
        assert!(state.peers.get("peer-1").is_none());
        assert!(state.peers.get("peer-2").is_some());
    }

    #[test]
    fn remove_peer_by_fullname_no_match_keeps_all() {
        let state = make_state();
        state.upsert_peer(make_peer("peer-1", "Alice"));

        state.remove_peer_by_fullname("unknown._duto._tcp.local.");
        assert_eq!(state.peers.len(), 1);
    }

    #[test]
    fn settings_persist_via_update() {
        let state = make_state();
        state
            .update_settings(|s| {
                s.destination_dir = "/custom/path".into();
            })
            .unwrap();

        let loaded = state.get_settings();
        assert_eq!(loaded.destination_dir, "/custom/path");
    }

    #[test]
    fn saved_display_name_overrides_the_system_identity() {
        let dir = temp_dir();
        let settings = Settings {
            display_name: Some("Djalma's phone".into()),
            ..Settings::default()
        };
        settings.save(&dir).unwrap();

        let state = AppState::new(make_device(), dir.clone());

        assert_eq!(state.get_device().display_name, "Djalma's phone");
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn updating_display_name_changes_the_runtime_identity_and_persists_it() {
        let state = make_state();

        let device = state.update_display_name("  My Galaxy  ").unwrap();

        assert_eq!(device.display_name, "My Galaxy");
        assert_eq!(state.get_device().display_name, "My Galaxy");
        assert_eq!(
            state.get_settings().display_name.as_deref(),
            Some("My Galaxy")
        );
    }
}

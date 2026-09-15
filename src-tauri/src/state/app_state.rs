use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use super::device::DeviceIdentity;
use super::settings::Settings;
use crate::discovery::types::PeerInfo;
use crate::transfer::cancellation::TransferRegistry;

/// Info about an active or completed transfer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransferInfo {
    pub transfer_id: String,
}

/// Centralized runtime state shared across Tauri commands.
pub struct AppState {
    pub device: DeviceIdentity,
    pub peers: DashMap<String, PeerInfo>,
    pub transfers: DashMap<String, TransferInfo>,
    pub settings: Mutex<Settings>,
    pub data_dir: PathBuf,
    pub transfer_registry: Arc<TransferRegistry>,
}

impl AppState {
    pub fn new(device: DeviceIdentity, data_dir: PathBuf) -> Self {
        let settings = Settings::load(&data_dir);
        Self {
            device,
            peers: DashMap::new(),
            transfers: DashMap::new(),
            settings: Mutex::new(settings),
            data_dir,
            transfer_registry: Arc::new(TransferRegistry::default()),
        }
    }

    pub fn get_settings(&self) -> Settings {
        self.settings.lock().unwrap().clone()
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
    pub fn upsert_peer(&self, peer: PeerInfo) {
        self.peers.insert(peer.device_id.clone(), peer);
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
        assert_eq!(state.device.device_id, "test-id");
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
}

use mdns_sd::{Error, Receiver, ServiceDaemon, ServiceEvent, ServiceInfo};
use std::collections::HashMap;
use std::time::{Duration, Instant};
use tokio::sync::broadcast;
use tracing;

use crate::state::device::DeviceIdentity;

use super::types::*;

/// Events emitted by the discovery system.
#[derive(Debug, Clone)]
pub enum DiscoveryEvent {
    PeerFound(PeerInfo),
    PeerRemoved(String), // fullname
}

const PEER_REMOVAL_GRACE: Duration = Duration::from_secs(5);
const DISCOVERY_POLL_INTERVAL: Duration = Duration::from_millis(200);
const BROWSE_REFRESH_DELAY: Duration = Duration::from_millis(250);
const COMMAND_RETRY_DELAY: Duration = Duration::from_millis(25);

struct PendingRemoval {
    fullname: String,
    deadline: Instant,
}

#[derive(Default)]
struct DiscoveryState {
    active_services: HashMap<String, String>,
    pending_removals: HashMap<String, PendingRemoval>,
}

/// Handles mDNS service registration and browsing.
pub struct MdnsDiscovery {
    daemon: ServiceDaemon,
    service_fullname: String,
    own_device_id: String,
    port: u16,
    event_tx: broadcast::Sender<DiscoveryEvent>,
}

/// RAII wrapper that unregisters the mDNS service on drop,
/// preventing name collisions (e.g. the `(2)` suffix) on restart.
pub struct DiscoveryHandle {
    inner: std::sync::Mutex<Option<MdnsDiscovery>>,
}

impl DiscoveryHandle {
    pub fn new(discovery: MdnsDiscovery) -> Self {
        Self {
            inner: std::sync::Mutex::new(Some(discovery)),
        }
    }

    /// Tauri exits the process directly, so call this from its Exit event.
    pub fn shutdown(&self) {
        if let Some(discovery) = self.inner.lock().unwrap().take() {
            if let Err(e) = discovery.shutdown() {
                tracing::warn!("Failed to shut down mDNS discovery: {}", e);
            }
        }
    }

    pub fn update_identity(
        &self,
        device: &DeviceIdentity,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut guard = self.inner.lock().unwrap();
        let discovery = guard.as_mut().ok_or("Discovery is not running")?;
        discovery.update_identity(device)
    }
}

impl Drop for DiscoveryHandle {
    fn drop(&mut self) {
        self.shutdown();
    }
}

impl MdnsDiscovery {
    /// Create a new discovery instance, register our service, and start browsing.
    pub fn new(
        device: &DeviceIdentity,
        port: u16,
    ) -> Result<(Self, broadcast::Receiver<DiscoveryEvent>), Box<dyn std::error::Error>> {
        let daemon = ServiceDaemon::new()?;
        let (event_tx, event_rx) = broadcast::channel(64);

        let service_info = service_info(device, port)?;

        let service_fullname = service_info.get_fullname().to_string();

        daemon.register(service_info)?;

        tracing::info!(
            service_type = SERVICE_TYPE,
            instance = %device.device_id,
            port = port,
            "mDNS service registered"
        );

        let discovery = Self {
            daemon,
            service_fullname,
            own_device_id: device.device_id.clone(),
            port,
            event_tx,
        };

        Ok((discovery, event_rx))
    }

    /// Start browsing for peers in a background thread.
    /// Keep one receiver alive and refresh it when the last known announcement for a
    /// peer disappears. Mobile platforms can restart their advertiser and immediately
    /// return under a conflict-safe alias such as "(2)".
    pub fn start_browsing(&self) -> Result<(), Box<dyn std::error::Error>> {
        let mut receiver = self.daemon.browse(SERVICE_TYPE)?;
        let daemon = self.daemon.clone();
        let own_device_id = self.own_device_id.clone();
        let event_tx = self.event_tx.clone();

        std::thread::spawn(move || {
            let mut state = DiscoveryState::default();
            let mut refresh_deadline = None;
            loop {
                match receiver.recv_timeout(DISCOVERY_POLL_INTERVAL) {
                    Ok(event) => {
                        let refresh =
                            forward_service_event(event, &own_device_id, &mut state, &event_tx);
                        if refresh && refresh_deadline.is_none() {
                            // Drain the current batch first. An interface change can remove
                            // several peers at once and replacing the receiver immediately
                            // would discard the remaining removal events.
                            refresh_deadline = Some(Instant::now() + BROWSE_REFRESH_DELAY);
                        }
                    }
                    Err(_) if receiver.is_disconnected() => break,
                    Err(_) => {}
                }

                let now = Instant::now();
                if refresh_deadline.is_some_and(|deadline| deadline <= now) {
                    if state.pending_removals.is_empty() {
                        refresh_deadline = None;
                    } else {
                        match refresh_browser(&daemon) {
                            Ok(new_receiver) => {
                                receiver = new_receiver;
                                refresh_deadline = None;
                            }
                            Err(error) => {
                                tracing::warn!("Failed to refresh mDNS browse: {}", error);
                                refresh_deadline = Some(now + Duration::from_secs(1));
                            }
                        }
                    }
                }

                emit_expired_removals(&mut state, &event_tx, now);
            }
        });

        Ok(())
    }

    fn update_identity(
        &mut self,
        device: &DeviceIdentity,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if device.device_id != self.own_device_id {
            return Err("Cannot replace the discovery device id".into());
        }

        let receiver = self.daemon.unregister(&self.service_fullname)?;
        let _ = receiver.recv_timeout(std::time::Duration::from_secs(2));
        let service_info = service_info(device, self.port)?;
        self.service_fullname = service_info.get_fullname().to_string();
        self.daemon.register(service_info)?;
        tracing::info!(display_name = %device.display_name, "mDNS identity updated");
        Ok(())
    }

    /// Unregister our service and shut down the daemon.
    pub fn shutdown(self) -> Result<(), Box<dyn std::error::Error>> {
        tracing::info!("Shutting down mDNS discovery");
        let receiver = self.daemon.unregister(&self.service_fullname)?;
        // Wait for unregister to complete (with timeout)
        let _ = receiver.recv_timeout(std::time::Duration::from_secs(2));
        self.daemon.shutdown()?;
        Ok(())
    }
}

fn refresh_browser(daemon: &ServiceDaemon) -> mdns_sd::Result<Receiver<ServiceEvent>> {
    // Commands sent through a ServiceDaemon are processed in order. Stop the old
    // listener first so the replacement receiver owns the service-type subscription
    // and receives both cached records and a fresh PTR query.
    retry_again(|| daemon.stop_browse(SERVICE_TYPE))?;
    retry_again(|| daemon.browse(SERVICE_TYPE))
}

fn retry_again<T>(mut operation: impl FnMut() -> mdns_sd::Result<T>) -> mdns_sd::Result<T> {
    for attempt in 0..4 {
        match operation() {
            Err(Error::Again) if attempt < 3 => std::thread::sleep(COMMAND_RETRY_DELAY),
            result => return result,
        }
    }
    unreachable!("The retry loop always returns on its final attempt")
}

fn service_info(
    device: &DeviceIdentity,
    port: u16,
) -> Result<ServiceInfo, Box<dyn std::error::Error>> {
    let properties: Vec<(&str, &str)> = vec![
        (TXT_PROTOCOL_VERSION, PROTOCOL_VERSION),
        (TXT_DEVICE_ID, &device.device_id),
        (TXT_DISPLAY_NAME, &device.display_name),
        (TXT_HOSTNAME, &device.hostname),
        (TXT_PLATFORM, &device.platform),
    ];

    // Keep the DNS-SD server name unique even when two devices share a model or display name.
    let short_id = &device.device_id[..8.min(device.device_id.len())];
    let mdns_hostname = format!("dukto-{short_id}.local.");

    Ok(ServiceInfo::new(
        SERVICE_TYPE,
        &device.device_id,
        &mdns_hostname,
        "",
        port,
        properties.as_slice(),
    )?
    .enable_addr_auto())
}

fn forward_service_event(
    event: ServiceEvent,
    own_device_id: &str,
    state: &mut DiscoveryState,
    event_tx: &broadcast::Sender<DiscoveryEvent>,
) -> bool {
    match event {
        ServiceEvent::ServiceResolved(info) => {
            let props = txt_properties_to_map(info.get_properties());
            let addresses: Vec<std::net::IpAddr> = info.get_addresses().iter().copied().collect();
            let port = info.get_port();

            if let Some(peer) = PeerInfo::from_txt_records(&props, addresses, port) {
                if peer.device_id == own_device_id {
                    tracing::debug!("Skipping own device in discovery");
                    return false;
                }
                state
                    .active_services
                    .insert(info.get_fullname().to_owned(), peer.device_id.clone());
                state.pending_removals.remove(&peer.device_id);
                tracing::info!(
                    device_id = %peer.device_id,
                    display_name = %peer.display_name,
                    fullname = %info.get_fullname(),
                    "Peer found"
                );
                let _ = event_tx.send(DiscoveryEvent::PeerFound(peer));
            }
            false
        }
        ServiceEvent::ServiceRemoved(_service_type, fullname) => {
            let Some(device_id) = state.active_services.remove(&fullname) else {
                return false;
            };
            // A restarted device can coexist with its old mDNS name (e.g. "(2)").
            // Retiring one announcement must not hide another live announcement.
            if state.active_services.values().any(|id| id == &device_id) {
                tracing::debug!(%fullname, %device_id, "Another service still announces this peer");
                return false;
            }
            tracing::debug!(
                %fullname,
                %device_id,
                grace_seconds = PEER_REMOVAL_GRACE.as_secs(),
                "Peer announcement removed; waiting for a replacement alias"
            );
            state.pending_removals.insert(
                device_id,
                PendingRemoval {
                    fullname,
                    deadline: Instant::now() + PEER_REMOVAL_GRACE,
                },
            );
            true
        }
        ServiceEvent::SearchStarted(s) => {
            tracing::debug!(service_type = %s, "mDNS browse started");
            false
        }
        _ => false,
    }
}

fn emit_expired_removals(
    state: &mut DiscoveryState,
    event_tx: &broadcast::Sender<DiscoveryEvent>,
    now: Instant,
) {
    let expired: Vec<String> = state
        .pending_removals
        .iter()
        .filter(|(_, removal)| removal.deadline <= now)
        .map(|(device_id, _)| device_id.clone())
        .collect();

    for device_id in expired {
        let Some(removal) = state.pending_removals.remove(&device_id) else {
            continue;
        };
        if state.active_services.values().any(|id| id == &device_id) {
            continue;
        }
        tracing::info!(fullname = %removal.fullname, %device_id, "Peer removed after discovery grace period");
        let _ = event_tx.send(DiscoveryEvent::PeerRemoved(removal.fullname));
    }
}

fn txt_properties_to_map(props: &mdns_sd::TxtProperties) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for prop in props.iter() {
        let val = prop.val_str();
        if !val.is_empty() {
            map.insert(prop.key().to_string(), val.to_string());
        }
    }
    map
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_device(id: &str) -> DeviceIdentity {
        DeviceIdentity {
            device_id: id.into(),
            display_name: format!("user-{}", id),
            hostname: format!("host-{}", id),
            platform: "macos".into(),
        }
    }

    #[test]
    fn removing_old_service_alias_keeps_current_peer_visible() {
        let (tx, mut rx) = broadcast::channel(16);
        let mut state = DiscoveryState::default();
        let device = test_device("alias-peer");
        let properties = [
            (TXT_DEVICE_ID, device.device_id.as_str()),
            (TXT_DISPLAY_NAME, device.display_name.as_str()),
            (TXT_HOSTNAME, device.hostname.as_str()),
            (TXT_PLATFORM, device.platform.as_str()),
        ];
        let service = |name| {
            ServiceInfo::new(
                SERVICE_TYPE,
                name,
                "alias-host.local.",
                "127.0.0.1",
                4242,
                &properties[..],
            )
            .unwrap()
        };
        let old = service("alias-peer");
        let current = service("alias-peer (2)");
        let old_name = old.get_fullname().to_owned();
        let current_name = current.get_fullname().to_owned();
        forward_service_event(
            ServiceEvent::ServiceResolved(old),
            "observer",
            &mut state,
            &tx,
        );
        forward_service_event(
            ServiceEvent::ServiceResolved(current),
            "observer",
            &mut state,
            &tx,
        );
        assert!(matches!(rx.try_recv(), Ok(DiscoveryEvent::PeerFound(_))));
        assert!(matches!(rx.try_recv(), Ok(DiscoveryEvent::PeerFound(_))));
        forward_service_event(
            ServiceEvent::ServiceRemoved(SERVICE_TYPE.into(), old_name),
            "observer",
            &mut state,
            &tx,
        );
        assert!(
            rx.try_recv().is_err(),
            "Removing an old alias must not hide the active device"
        );
        assert!(forward_service_event(
            ServiceEvent::ServiceRemoved(SERVICE_TYPE.into(), current_name.clone()),
            "observer",
            &mut state,
            &tx,
        ));
        assert!(
            rx.try_recv().is_err(),
            "Removal must honor the grace period"
        );
        emit_expired_removals(
            &mut state,
            &tx,
            Instant::now() + PEER_REMOVAL_GRACE + Duration::from_millis(1),
        );
        assert!(
            matches!(rx.try_recv(), Ok(DiscoveryEvent::PeerRemoved(name)) if name == current_name)
        );
    }

    #[test]
    fn replacement_alias_cancels_pending_removal() {
        let (tx, mut rx) = broadcast::channel(16);
        let mut state = DiscoveryState::default();
        let device = test_device("restarting-peer");
        let properties = [
            (TXT_DEVICE_ID, device.device_id.as_str()),
            (TXT_DISPLAY_NAME, device.display_name.as_str()),
            (TXT_HOSTNAME, device.hostname.as_str()),
            (TXT_PLATFORM, device.platform.as_str()),
        ];
        let service = |name| {
            ServiceInfo::new(
                SERVICE_TYPE,
                name,
                "restart-host.local.",
                "127.0.0.1",
                4242,
                &properties[..],
            )
            .unwrap()
        };
        let old = service("restarting-peer");
        let old_name = old.get_fullname().to_owned();

        assert!(!forward_service_event(
            ServiceEvent::ServiceResolved(old),
            "observer",
            &mut state,
            &tx,
        ));
        assert!(matches!(rx.try_recv(), Ok(DiscoveryEvent::PeerFound(_))));
        assert!(forward_service_event(
            ServiceEvent::ServiceRemoved(SERVICE_TYPE.into(), old_name),
            "observer",
            &mut state,
            &tx,
        ));
        assert!(rx.try_recv().is_err());

        assert!(!forward_service_event(
            ServiceEvent::ServiceResolved(service("restarting-peer (2)")),
            "observer",
            &mut state,
            &tx,
        ));
        assert!(matches!(rx.try_recv(), Ok(DiscoveryEvent::PeerFound(_))));
        emit_expired_removals(
            &mut state,
            &tx,
            Instant::now() + PEER_REMOVAL_GRACE + Duration::from_millis(1),
        );
        assert!(
            rx.try_recv().is_err(),
            "A replacement alias must keep the peer visible"
        );
    }

    #[test]
    fn browse_refresh_rediscovers_restarted_peer_before_removal() {
        let observer = test_device(&uuid::Uuid::new_v4().to_string());
        let (discovery, mut events) = MdnsDiscovery::new(&observer, 9010).unwrap();
        discovery.start_browsing().unwrap();

        let peer = test_device(&uuid::Uuid::new_v4().to_string());
        let advertiser = ServiceDaemon::new().unwrap();
        let old = service_info(&peer, 9011).unwrap();
        let old_fullname = old.get_fullname().to_owned();
        advertiser.register(old).unwrap();

        let found_deadline = Instant::now() + Duration::from_secs(10);
        let mut initially_found = false;
        while Instant::now() < found_deadline {
            if let Ok(DiscoveryEvent::PeerFound(info)) = events.try_recv() {
                if info.device_id == peer.device_id {
                    initially_found = true;
                    break;
                }
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        assert!(initially_found, "Peer must be visible before it restarts");

        let unregister = advertiser.unregister(&old_fullname).unwrap();
        let _ = unregister.recv_timeout(Duration::from_secs(2));
        let alias_properties = [
            (TXT_PROTOCOL_VERSION, PROTOCOL_VERSION),
            (TXT_DEVICE_ID, peer.device_id.as_str()),
            (TXT_DISPLAY_NAME, peer.display_name.as_str()),
            (TXT_HOSTNAME, peer.hostname.as_str()),
            (TXT_PLATFORM, peer.platform.as_str()),
        ];
        let short_id = &peer.device_id[..8];
        advertiser
            .register(
                ServiceInfo::new(
                    SERVICE_TYPE,
                    &format!("{} (2)", peer.device_id),
                    &format!("dukto-{short_id}.local."),
                    "",
                    9011,
                    &alias_properties[..],
                )
                .unwrap()
                .enable_addr_auto(),
            )
            .unwrap();

        let observation_deadline = Instant::now() + PEER_REMOVAL_GRACE + Duration::from_secs(2);
        let mut rediscovered = false;
        let mut removed = false;
        while Instant::now() < observation_deadline {
            match events.try_recv() {
                Ok(DiscoveryEvent::PeerFound(info)) if info.device_id == peer.device_id => {
                    rediscovered = true;
                }
                Ok(DiscoveryEvent::PeerRemoved(fullname)) if fullname.contains(&peer.device_id) => {
                    removed = true;
                    break;
                }
                _ => std::thread::sleep(Duration::from_millis(50)),
            }
        }

        advertiser.shutdown().ok();
        discovery.shutdown().ok();
        assert!(rediscovered, "The replacement alias must be rediscovered");
        assert!(
            !removed,
            "A peer that returned under a replacement alias must stay visible"
        );
    }

    #[test]
    fn raw_mdns_register_and_browse() {
        // Verify mdns-sd works at the raw API level
        let daemon = ServiceDaemon::new().expect("Failed to create daemon");

        // Use a unique hostname to avoid Bonjour conflicts
        let test_host = format!(
            "dukto-test-raw-{}.local.",
            &uuid::Uuid::new_v4().to_string()[..8]
        );
        let service = ServiceInfo::new(
            SERVICE_TYPE,
            "test-raw",
            &test_host,
            "",
            9010,
            &[("device_id", "raw-test")][..],
        )
        .expect("Failed to create service info")
        .enable_addr_auto();

        daemon.register(service).expect("Failed to register");

        let browse_rx = daemon.browse(SERVICE_TYPE).expect("Failed to browse");

        let mut found = false;
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);

        while std::time::Instant::now() < deadline {
            match browse_rx.recv_timeout(std::time::Duration::from_millis(200)) {
                Ok(ServiceEvent::ServiceResolved(info)) => {
                    eprintln!("[test] Resolved: {}", info.get_fullname());
                    if info.get_fullname().contains("test-raw") {
                        found = true;
                        break;
                    }
                }
                Ok(event) => {
                    eprintln!("[test] Event: {:?}", event);
                }
                Err(_) => {}
            }
        }

        assert!(
            found,
            "Should discover our own registered service via raw mDNS"
        );
        daemon.shutdown().ok();
    }

    #[test]
    fn register_and_browse_finds_peer() {
        // Device A registers
        let device_a = test_device("aaa-111");
        let (discovery_a, _rx_a) =
            MdnsDiscovery::new(&device_a, 9001).expect("Failed to create discovery A");

        // Device B registers and browses
        let device_b = test_device("bbb-222");
        let (discovery_b, _rx_b) =
            MdnsDiscovery::new(&device_b, 9002).expect("Failed to create discovery B");

        // Use raw browse receiver from B's daemon to find A
        let browse_rx = discovery_b
            .daemon
            .browse(SERVICE_TYPE)
            .expect("Failed to browse");

        let mut found_a = false;
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(8);

        while std::time::Instant::now() < deadline {
            match browse_rx.recv_timeout(std::time::Duration::from_millis(200)) {
                Ok(ServiceEvent::ServiceResolved(info)) => {
                    let props = txt_properties_to_map(info.get_properties());
                    if props.get(TXT_DEVICE_ID).map(|s| s.as_str()) == Some("aaa-111") {
                        found_a = true;
                        assert_eq!(
                            props.get(TXT_DISPLAY_NAME).map(|s| s.as_str()),
                            Some("user-aaa-111")
                        );
                        assert_eq!(info.get_port(), 9001);
                        break;
                    }
                }
                Ok(_) => {}
                Err(_) => {}
            }
        }

        assert!(found_a, "Device B should have discovered Device A via mDNS");

        discovery_b.shutdown().ok();
        discovery_a.shutdown().ok();
    }

    #[test]
    fn explicit_shutdown_announces_peer_removal() {
        let observer = test_device(&uuid::Uuid::new_v4().to_string());
        let (discovery, mut events) = MdnsDiscovery::new(&observer, 9006).unwrap();
        discovery.start_browsing().unwrap();
        let peer = test_device(&uuid::Uuid::new_v4().to_string());
        let (advertiser, _events) = MdnsDiscovery::new(&peer, 9007).unwrap();
        let handle = DiscoveryHandle::new(advertiser);

        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        let mut found = false;
        while std::time::Instant::now() < deadline {
            if let Ok(DiscoveryEvent::PeerFound(info)) = events.try_recv() {
                if info.device_id == peer.device_id {
                    found = true;
                    break;
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
        assert!(found, "Peer must be visible before testing departure");
        handle.shutdown();
        handle.shutdown(); // Exit cleanup and Drop must be safe together.

        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        let mut removed = false;
        while std::time::Instant::now() < deadline {
            if let Ok(DiscoveryEvent::PeerRemoved(fullname)) = events.try_recv() {
                if fullname.starts_with(&peer.device_id) {
                    removed = true;
                    break;
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
        discovery.shutdown().unwrap();
        assert!(
            removed,
            "Departure must arrive without waiting for the mDNS TTL"
        );
    }

    #[test]
    fn discovers_peer_arriving_after_thirty_seconds() {
        // Regression: the periodic browse replaced the event consumer after 30s.
        let observer = test_device(&uuid::Uuid::new_v4().to_string());
        let (discovery, mut events) = MdnsDiscovery::new(&observer, 9004).unwrap();
        discovery.start_browsing().unwrap();
        std::thread::sleep(std::time::Duration::from_secs(35));

        let late_peer = test_device(&uuid::Uuid::new_v4().to_string());
        let (advertiser, _events) = MdnsDiscovery::new(&late_peer, 9005).unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        let mut found = false;
        while std::time::Instant::now() < deadline {
            if let Ok(DiscoveryEvent::PeerFound(peer)) = events.try_recv() {
                if peer.device_id == late_peer.device_id {
                    found = true;
                    break;
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
        advertiser.shutdown().unwrap();
        discovery.shutdown().unwrap();
        assert!(found, "The active discovery stream must report a late peer");
    }

    #[test]
    fn does_not_discover_self_via_broadcast() {
        let device = test_device("ccc-333");
        let (discovery, mut rx) =
            MdnsDiscovery::new(&device, 9003).expect("Failed to create discovery");
        discovery
            .start_browsing()
            .expect("Failed to start browsing");

        // Give time for browse loop to process events
        std::thread::sleep(std::time::Duration::from_secs(3));

        let mut found_self = false;
        while let Ok(event) = rx.try_recv() {
            if let DiscoveryEvent::PeerFound(peer) = event {
                if peer.device_id == "ccc-333" {
                    found_self = true;
                }
            }
        }

        assert!(!found_self, "Should not discover own device");

        discovery.shutdown().ok();
    }
}

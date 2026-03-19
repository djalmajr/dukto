use mdns_sd::{ServiceDaemon, ServiceEvent, ServiceInfo};
use std::collections::HashMap;
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

/// Handles mDNS service registration and browsing.
pub struct MdnsDiscovery {
    daemon: ServiceDaemon,
    service_fullname: String,
    own_device_id: String,
    event_tx: broadcast::Sender<DiscoveryEvent>,
}

impl MdnsDiscovery {
    /// Create a new discovery instance, register our service, and start browsing.
    pub fn new(
        device: &DeviceIdentity,
        port: u16,
    ) -> Result<(Self, broadcast::Receiver<DiscoveryEvent>), Box<dyn std::error::Error>> {
        let daemon = ServiceDaemon::new()?;
        let (event_tx, event_rx) = broadcast::channel(64);

        let instance_name = &device.device_id;

        let properties: Vec<(&str, &str)> = vec![
            (TXT_PROTOCOL_VERSION, PROTOCOL_VERSION),
            (TXT_DEVICE_ID, &device.device_id),
            (TXT_DISPLAY_NAME, &device.display_name),
            (TXT_HOSTNAME, &device.hostname),
            (TXT_PLATFORM, &device.platform),
        ];

        // Use a unique hostname derived from device_id to avoid conflicting
        // with the system's own Bonjour hostname registration.
        let short_id = &device.device_id[..8.min(device.device_id.len())];
        let mdns_hostname = format!("dukto-{}.local.", short_id);

        let service_info = ServiceInfo::new(
            SERVICE_TYPE,
            instance_name,
            &mdns_hostname,
            "",
            port,
            properties.as_slice(),
        )?
        .enable_addr_auto();

        let service_fullname = service_info.get_fullname().to_string();

        daemon.register(service_info)?;

        tracing::info!(
            service_type = SERVICE_TYPE,
            instance = instance_name,
            port = port,
            "mDNS service registered"
        );

        let discovery = Self {
            daemon,
            service_fullname,
            own_device_id: device.device_id.clone(),
            event_tx,
        };

        Ok((discovery, event_rx))
    }

    /// Start browsing for peers in a background thread.
    /// Returns a JoinHandle that runs until the daemon shuts down.
    pub fn start_browsing(&self) -> Result<(), Box<dyn std::error::Error>> {
        let receiver = self.daemon.browse(SERVICE_TYPE)?;
        let own_device_id = self.own_device_id.clone();
        let event_tx = self.event_tx.clone();

        std::thread::spawn(move || {
            while let Ok(event) = receiver.recv() {
                match event {
                    ServiceEvent::ServiceResolved(info) => {
                        let props = txt_properties_to_map(info.get_properties());
                        let addresses: Vec<std::net::IpAddr> =
                            info.get_addresses().iter().copied().collect();
                        let port = info.get_port();

                        if let Some(peer) = PeerInfo::from_txt_records(&props, addresses, port) {
                            if peer.device_id == own_device_id {
                                tracing::debug!("Skipping own device in discovery");
                                continue;
                            }
                            tracing::info!(
                                device_id = %peer.device_id,
                                display_name = %peer.display_name,
                                "Peer found"
                            );
                            let _ = event_tx.send(DiscoveryEvent::PeerFound(peer));
                        }
                    }
                    ServiceEvent::ServiceRemoved(_service_type, fullname) => {
                        tracing::info!(fullname = %fullname, "Peer removed");
                        let _ = event_tx.send(DiscoveryEvent::PeerRemoved(fullname));
                    }
                    ServiceEvent::SearchStarted(s) => {
                        tracing::debug!(service_type = %s, "mDNS browse started");
                    }
                    _ => {}
                }
            }
        });

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

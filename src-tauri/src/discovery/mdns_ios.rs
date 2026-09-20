use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use tokio::sync::broadcast;
use zeroconf::prelude::{TEventLoop, TMdnsBrowser, TMdnsService, TTxtRecord};
use zeroconf::{BrowserEvent, MdnsBrowser, MdnsService, ServiceType, TxtRecord};

use crate::state::device::DeviceIdentity;

use super::types::*;

#[derive(Debug, Clone)]
pub enum DiscoveryEvent {
    PeerFound(PeerInfo),
    PeerRemoved(String),
}

const PEER_REMOVAL_GRACE: Duration = Duration::from_secs(5);
const EVENT_LOOP_POLL_INTERVAL: Duration = Duration::from_millis(200);

struct PendingRemoval {
    fullname: String,
    deadline: Instant,
}

#[derive(Default)]
struct DiscoveryState {
    active_services: HashMap<String, String>,
    pending_removals: HashMap<String, PendingRemoval>,
}

struct NativeWorker {
    stop: Arc<AtomicBool>,
    join: Option<JoinHandle<()>>,
}

impl NativeWorker {
    fn new(stop: Arc<AtomicBool>, join: JoinHandle<()>) -> Self {
        Self {
            stop,
            join: Some(join),
        }
    }

    fn shutdown(mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}

impl Drop for NativeWorker {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}

pub struct MdnsDiscovery {
    own_device_id: String,
    port: u16,
    registration: Mutex<Option<NativeWorker>>,
    browser: Mutex<Option<NativeWorker>>,
    event_tx: broadcast::Sender<DiscoveryEvent>,
}

pub struct DiscoveryHandle {
    inner: Mutex<Option<MdnsDiscovery>>,
}

impl DiscoveryHandle {
    pub fn new(discovery: MdnsDiscovery) -> Self {
        Self {
            inner: Mutex::new(Some(discovery)),
        }
    }

    pub fn shutdown(&self) {
        if let Some(discovery) = self.inner.lock().unwrap().take() {
            discovery.shutdown();
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
    pub fn new(
        device: &DeviceIdentity,
        port: u16,
    ) -> Result<(Self, broadcast::Receiver<DiscoveryEvent>), Box<dyn std::error::Error>> {
        let (event_tx, event_rx) = broadcast::channel(64);
        let registration = start_registration(device, port)?;

        tracing::info!(
            service_type = SERVICE_TYPE,
            instance = %device.device_id,
            port,
            "Native Bonjour service registered"
        );

        Ok((
            Self {
                own_device_id: device.device_id.clone(),
                port,
                registration: Mutex::new(Some(registration)),
                browser: Mutex::new(None),
                event_tx,
            },
            event_rx,
        ))
    }

    pub fn start_browsing(&self) -> Result<(), Box<dyn std::error::Error>> {
        let mut browser_guard = self.browser.lock().unwrap();
        if browser_guard.is_some() {
            return Ok(());
        }

        let service_type = service_type()?;
        let mut browser = MdnsBrowser::new(service_type);
        let own_device_id = self.own_device_id.clone();
        let event_tx = self.event_tx.clone();
        let state = Arc::new(Mutex::new(DiscoveryState::default()));
        let callback_state = state.clone();
        let callback_tx = event_tx.clone();

        browser.set_service_callback(Box::new(move |result, _context| match result {
            Ok(event) => handle_browser_event(event, &own_device_id, &callback_state, &callback_tx),
            Err(error) => tracing::warn!(%error, "Native Bonjour browse callback failed"),
        }));

        let event_loop = browser.browse_services()?;
        let stop = Arc::new(AtomicBool::new(false));
        let thread_stop = stop.clone();
        let join = std::thread::Builder::new()
            .name("dukto-ios-bonjour-browser".into())
            .spawn(move || {
                while !thread_stop.load(Ordering::Acquire) {
                    if let Err(error) = event_loop.poll(EVENT_LOOP_POLL_INTERVAL) {
                        tracing::warn!(%error, "Native Bonjour browse poll failed");
                    }
                    emit_expired_removals(&state, &event_tx, Instant::now());
                }
                drop(event_loop);
                drop(browser);
            })?;

        *browser_guard = Some(NativeWorker::new(stop, join));
        tracing::debug!(service_type = SERVICE_TYPE, "Native Bonjour browse started");
        Ok(())
    }

    fn update_identity(
        &mut self,
        device: &DeviceIdentity,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if device.device_id != self.own_device_id {
            return Err("Cannot replace the discovery device id".into());
        }

        let new_registration = start_registration(device, self.port)?;
        let old_registration = self.registration.lock().unwrap().replace(new_registration);
        if let Some(worker) = old_registration {
            worker.shutdown();
        }
        tracing::info!(display_name = %device.display_name, "Native Bonjour identity updated");
        Ok(())
    }

    pub fn shutdown(self) {
        if let Some(worker) = self.browser.into_inner().unwrap() {
            worker.shutdown();
        }
        if let Some(worker) = self.registration.into_inner().unwrap() {
            worker.shutdown();
        }
        tracing::info!("Native Bonjour discovery stopped");
    }
}

fn service_type() -> Result<ServiceType, Box<dyn std::error::Error>> {
    Ok(ServiceType::new("dukto", "tcp")?)
}

fn start_registration(
    device: &DeviceIdentity,
    port: u16,
) -> Result<NativeWorker, Box<dyn std::error::Error>> {
    let mut txt = TxtRecord::new();
    for (key, value) in (PeerInfo {
        device_id: device.device_id.clone(),
        display_name: device.display_name.clone(),
        hostname: device.hostname.clone(),
        platform: device.platform.clone(),
        addresses: Vec::new(),
        port,
        protocol_version: PROTOCOL_VERSION.into(),
    })
    .to_txt_properties()
    {
        txt.insert(&key, &value)?;
    }

    let mut service = MdnsService::new(service_type()?, port);
    service.set_name(&device.device_id);
    service.set_domain("local.");
    service.set_txt_record(txt);
    service.set_registered_callback(Box::new(|result, _context| match result {
        Ok(registration) => tracing::debug!(
            name = %registration.name(),
            "Native Bonjour registration confirmed"
        ),
        Err(error) => tracing::warn!(%error, "Native Bonjour registration failed"),
    }));

    let event_loop = service.register()?;
    let stop = Arc::new(AtomicBool::new(false));
    let thread_stop = stop.clone();
    let join = std::thread::Builder::new()
        .name("dukto-ios-bonjour-register".into())
        .spawn(move || {
            while !thread_stop.load(Ordering::Acquire) {
                if let Err(error) = event_loop.poll(EVENT_LOOP_POLL_INTERVAL) {
                    tracing::warn!(%error, "Native Bonjour registration poll failed");
                }
            }
            drop(event_loop);
            drop(service);
        })?;

    Ok(NativeWorker::new(stop, join))
}

fn handle_browser_event(
    event: BrowserEvent,
    own_device_id: &str,
    state: &Arc<Mutex<DiscoveryState>>,
    event_tx: &broadcast::Sender<DiscoveryEvent>,
) {
    match event {
        BrowserEvent::Add(service) => {
            let properties = service
                .txt()
                .as_ref()
                .map(|txt| txt.to_map())
                .unwrap_or_default();
            let addresses = service
                .address()
                .parse::<IpAddr>()
                .map(|address| vec![address])
                .unwrap_or_default();

            let Some(peer) = PeerInfo::from_txt_records(&properties, addresses, *service.port())
            else {
                tracing::warn!(name = %service.name(), "Ignoring incomplete Bonjour service");
                return;
            };
            if peer.device_id == own_device_id {
                tracing::debug!("Skipping own device in native Bonjour discovery");
                return;
            }

            let fullname = service_fullname(service.name(), service.domain());
            let mut state = state.lock().unwrap();
            state
                .active_services
                .insert(fullname.clone(), peer.device_id.clone());
            state.pending_removals.remove(&peer.device_id);
            drop(state);

            tracing::info!(
                device_id = %peer.device_id,
                display_name = %peer.display_name,
                %fullname,
                "Peer found through native Bonjour"
            );
            let _ = event_tx.send(DiscoveryEvent::PeerFound(peer));
        }
        BrowserEvent::Remove(service) => {
            let fullname = service_fullname(service.name(), service.domain());
            let mut state = state.lock().unwrap();
            let Some(device_id) = state.active_services.remove(&fullname) else {
                return;
            };
            if state.active_services.values().any(|id| id == &device_id) {
                return;
            }
            state.pending_removals.insert(
                device_id,
                PendingRemoval {
                    fullname,
                    deadline: Instant::now() + PEER_REMOVAL_GRACE,
                },
            );
        }
    }
}

fn service_fullname(name: &str, domain: &str) -> String {
    format!("{name}._dukto._tcp.{}.", domain.trim_matches('.'))
}

fn emit_expired_removals(
    state: &Arc<Mutex<DiscoveryState>>,
    event_tx: &broadcast::Sender<DiscoveryEvent>,
    now: Instant,
) {
    let mut state = state.lock().unwrap();
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
        tracing::info!(fullname = %removal.fullname, %device_id, "Peer removed after native Bonjour grace period");
        let _ = event_tx.send(DiscoveryEvent::PeerRemoved(removal.fullname));
    }
}

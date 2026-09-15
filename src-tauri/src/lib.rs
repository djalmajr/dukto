#[cfg(feature = "desktop")]
pub mod commands;
pub mod crypto;
pub mod discovery;
pub mod platform;
pub mod protocol;
pub mod state;
pub mod transfer;

#[cfg(feature = "desktop")]
mod desktop {
    use tauri::{Emitter, Manager};

    use crate::commands;
    use crate::discovery::mdns::{DiscoveryEvent, DiscoveryHandle, MdnsDiscovery};
    use crate::state::app_state::AppState;
    use crate::state::device::DeviceIdentity;
    use crate::transfer::server::TransferServer;

    const QUIC_PORT: u16 = 4242;

    #[cfg_attr(mobile, tauri::mobile_entry_point)]
    pub fn run() {
        tracing_subscriber::fmt()
            .with_env_filter(
                tracing_subscriber::EnvFilter::try_from_default_env()
                    .unwrap_or_else(|_| "dukto_lib=debug,mdns_sd=info".into()),
            )
            .init();

        let _ = rustls::crypto::ring::default_provider().install_default();

        tauri::Builder::default()
            .plugin(tauri_plugin_dialog::init())
            .plugin(tauri_plugin_notification::init())
            .plugin(tauri_plugin_os::init())
            .plugin(tauri_plugin_shell::init())
            .plugin(tauri_plugin_updater::Builder::new().build())
            .plugin(tauri_plugin_process::init())
            .invoke_handler(tauri::generate_handler![
                commands::get_device_info,
                commands::get_peers,
                commands::get_settings,
                commands::set_destination_dir,
                commands::resolve_file_metadata,
                commands::send_to_peer,
                commands::cancel_transfer,
                commands::respond_transfer,
                commands::updater::download_app_update,
                commands::updater::install_app_update,
            ])
            .setup(|app| {
                #[cfg(target_os = "windows")]
                if let Some(window) = app.get_webview_window("main") {
                    window.set_decorations(false)?;
                }
                tracing::info!("Dukto v{} starting", env!("CARGO_PKG_VERSION"));

                let data_dir = app
                    .path()
                    .app_data_dir()
                    .expect("failed to get app data dir");
                let device = DeviceIdentity::load_or_create(&data_dir)
                    .expect("failed to load or create device identity");

                tracing::info!(
                    device_id = %device.device_id,
                    display_name = %device.display_name,
                    hostname = %device.hostname,
                    platform = %device.platform,
                    "Device identity loaded"
                );

                // Start mDNS discovery
                let (mdns_discovery, mut event_rx) =
                    MdnsDiscovery::new(&device, QUIC_PORT).expect("failed to start mDNS discovery");

                mdns_discovery
                    .start_browsing()
                    .expect("failed to start mDNS browsing");

                let app_state = AppState::new(device, data_dir);
                app.manage(app_state);

                // Start QUIC listener for incoming transfers
                let transfer_registry = app.state::<AppState>().transfer_registry.clone();
                let transfer_server =
                    TransferServer::start(app.handle().clone(), QUIC_PORT, transfer_registry);
                app.manage(transfer_server);

                // Forward discovery events to frontend AND update AppState
                let handle = app.handle().clone();
                tauri::async_runtime::spawn(async move {
                    loop {
                        match event_rx.recv().await {
                            Ok(DiscoveryEvent::PeerFound(peer)) => {
                                tracing::info!(device_id = %peer.device_id, "Peer found");
                                handle.state::<AppState>().upsert_peer(peer.clone());
                                let _ = handle.emit("peer:found", &peer);
                            }
                            Ok(DiscoveryEvent::PeerRemoved(fullname)) => {
                                tracing::info!(fullname = %fullname, "Peer removed");
                                handle
                                    .state::<AppState>()
                                    .remove_peer_by_fullname(&fullname);
                                let _ = handle.emit("peer:removed", &fullname);
                            }
                            Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                                tracing::warn!("Discovery events lagged by {}", n);
                            }
                            Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                                tracing::info!("Discovery event channel closed");
                                break;
                            }
                        }
                    }
                });

                // Store discovery handle — DiscoveryHandle unregisters the
                // mDNS service on drop, preventing name collisions on restart.
                app.manage(DiscoveryHandle::new(mdns_discovery));

                Ok(())
            })
            .build(tauri::generate_context!())
            .expect("error while building Dukto")
            .run(|app, event| {
                if matches!(event, tauri::RunEvent::Exit) {
                    app.state::<DiscoveryHandle>().shutdown();
                }
            });
    }
}

#[cfg(feature = "desktop")]
pub use desktop::run;

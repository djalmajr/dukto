#[cfg(feature = "app-common")]
pub mod commands;
pub mod crypto;
pub mod discovery;
pub mod internet;
pub mod platform;
pub mod protocol;
pub mod state;
pub mod transfer;

#[cfg(feature = "app-common")]
mod app_runner {
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    use tauri::{Emitter, Manager};

    use crate::commands;
    use crate::discovery::mdns::{DiscoveryEvent, DiscoveryHandle, MdnsDiscovery};
    use crate::state::app_state::AppState;
    use crate::state::device::DeviceIdentity;
    use crate::transfer::server::TransferServer;

    const QUIC_PORT: u16 = 4242;
    const REMOTE_SESSION_SWEEP_INTERVAL: Duration = Duration::from_secs(30);

    #[cfg_attr(
        any(target_os = "android", target_os = "ios"),
        tauri::mobile_entry_point
    )]
    pub fn run() {
        tracing_subscriber::fmt()
            .with_env_filter(
                tracing_subscriber::EnvFilter::try_from_default_env()
                    .unwrap_or_else(|_| "dukto_lib=debug,mdns_sd=info".into()),
            )
            .init();

        let _ = rustls::crypto::ring::default_provider().install_default();

        #[allow(unused_mut)]
        let mut builder = tauri::Builder::default()
            .plugin(tauri_plugin_deep_link::init())
            .plugin(tauri_plugin_dialog::init())
            .plugin(tauri_plugin_notification::init())
            .plugin(tauri_plugin_os::init())
            .plugin(tauri_plugin_shell::init())
            .plugin(tauri_plugin_process::init());

        #[cfg(target_os = "android")]
        {
            builder = builder.plugin(crate::platform::android_destination::init());
        }

        #[cfg(all(
            feature = "desktop",
            not(any(target_os = "ios", target_os = "android"))
        ))]
        {
            builder = builder.plugin(tauri_plugin_updater::Builder::new().build());
        }

        #[cfg(feature = "desktop")]
        let builder = builder.invoke_handler(tauri::generate_handler![
            commands::get_device_info,
            commands::get_peers,
            commands::get_settings,
            commands::set_destination_dir,
            commands::open_destination_picker,
            commands::open_send_file_picker,
            commands::release_selected_files,
            commands::open_privacy_policy,
            commands::set_display_name,
            commands::set_peer_order,
            commands::resolve_file_metadata,
            commands::send_to_peer,
            commands::cancel_transfer,
            commands::respond_transfer,
            commands::internet::create_internet_invite,
            commands::internet::get_internet_invitation_link,
            commands::internet::import_internet_invite,
            commands::internet::send_to_internet_session,
            commands::internet::cancel_internet_invite,
            commands::internet::confirm_internet_pairing_code,
            commands::updater::download_app_update,
            commands::updater::install_app_update,
        ]);

        #[cfg(not(feature = "desktop"))]
        let builder = builder.invoke_handler(tauri::generate_handler![
            commands::get_device_info,
            commands::get_peers,
            commands::get_settings,
            commands::set_destination_dir,
            commands::open_destination_picker,
            commands::open_send_file_picker,
            commands::release_selected_files,
            commands::open_privacy_policy,
            commands::set_display_name,
            commands::set_peer_order,
            commands::resolve_file_metadata,
            commands::send_to_peer,
            commands::cancel_transfer,
            commands::respond_transfer,
            commands::internet::create_internet_invite,
            commands::internet::get_internet_invitation_link,
            commands::internet::import_internet_invite,
            commands::internet::send_to_internet_session,
            commands::internet::cancel_internet_invite,
            commands::internet::confirm_internet_pairing_code,
        ]);

        builder
            .setup(|app| {
                #[cfg(target_os = "windows")]
                if let Some(window) = app.get_webview_window("main") {
                    window.set_decorations(false)?;
                }
                #[cfg(target_os = "ios")]
                crate::platform::ios_power::set_idle_timer_disabled(app.handle(), true)?;
                tracing::info!("Dukto v{} starting", env!("CARGO_PKG_VERSION"));

                let data_dir = app
                    .path()
                    .app_data_dir()
                    .expect("failed to get app data dir");
                let device = DeviceIdentity::load_or_create(&data_dir)
                    .expect("failed to load or create device identity");

                let app_state = AppState::new(device, data_dir);
                let remote_sessions = app_state.remote_sessions.clone();
                let device = app_state.get_device();
                tracing::info!(
                    device_id = %device.device_id,
                    display_name = %device.display_name,
                    hostname = %device.hostname,
                    platform = %device.platform,
                    "Device identity loaded"
                );
                app.manage(app_state);

                tauri::async_runtime::spawn(async move {
                    let mut interval = tokio::time::interval(REMOTE_SESSION_SWEEP_INTERVAL);
                    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
                    loop {
                        interval.tick().await;
                        let Ok(elapsed) = SystemTime::now().duration_since(UNIX_EPOCH) else {
                            continue;
                        };
                        remote_sessions.expire(elapsed.as_secs());
                    }
                });

                // Start QUIC listener for incoming transfers with fallback port
                let transfer_registry = app.state::<AppState>().transfer_registry.clone();
                let (transfer_server, bound_port) =
                    TransferServer::start(app.handle().clone(), QUIC_PORT, transfer_registry)
                        .expect("failed to start QUIC transfer server");
                app.manage(transfer_server);

                // Start mDNS discovery with actual bound port
                let (mdns_discovery, mut event_rx) = MdnsDiscovery::new(&device, bound_port)
                    .expect("failed to start mDNS discovery");

                mdns_discovery
                    .start_browsing()
                    .expect("failed to start mDNS browsing");

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
                    #[cfg(target_os = "ios")]
                    let _ = crate::platform::ios_power::set_idle_timer_disabled(app, false);
                }
            });
    }
}

#[cfg(feature = "app-common")]
pub use app_runner::run;

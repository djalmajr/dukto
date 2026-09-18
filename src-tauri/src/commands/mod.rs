#[cfg(feature = "desktop")]
pub mod updater;

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tauri::{Emitter, State};

use crate::crypto::noise::handshake_initiator;
use crate::discovery::mdns::DiscoveryHandle;
use crate::state::app_state::AppState;
use crate::state::device::DeviceIdentity;
use crate::state::settings::Settings;
use crate::transfer::quic::create_endpoint;
use crate::transfer::sender::{send_transfer_with_cancellation, CancellableSendCallbacks};
use crate::transfer::server::TransferServer;

#[tauri::command]
pub fn get_device_info(state: State<'_, AppState>) -> DeviceIdentity {
    state.get_device()
}

#[tauri::command]
pub fn get_peers(state: State<'_, AppState>) -> Vec<crate::discovery::types::PeerInfo> {
    state
        .peers
        .iter()
        .map(|peer| peer.value().clone())
        .collect()
}

#[tauri::command]
pub fn get_settings(state: State<'_, AppState>) -> Settings {
    state.get_settings()
}

#[tauri::command]
pub fn set_destination_dir(state: State<'_, AppState>, path: String) -> Result<(), String> {
    tracing::debug!(path = %path, "Saving destination directory");
    state
        .update_settings(|s| {
            s.destination_dir = path;
        })
        .map_err(|e| e.to_string())?;
    tracing::debug!("Destination directory saved");
    Ok(())
}

#[derive(Clone, Serialize)]
pub struct DestinationSelectionEvent {
    path: Option<String>,
    error: Option<String>,
}

#[cfg(target_os = "android")]
#[derive(Clone, Serialize)]
pub struct SendFileSelectionEvent {
    files: Option<Vec<crate::platform::android_destination::AndroidSelectedFile>>,
    error: Option<String>,
}

#[tauri::command]
pub fn open_destination_picker(app_handle: tauri::AppHandle) -> Result<(), String> {
    #[cfg(target_os = "android")]
    {
        use tauri::Manager;

        let picker = app_handle
            .state::<crate::platform::android_destination::AndroidDestination<tauri::Wry>>()
            .inner()
            .clone();
        tauri::async_runtime::spawn(async move {
            tracing::debug!("Opening Android destination picker");
            let event = match picker.pick_directory().await {
                Ok(path) => {
                    tracing::debug!(?path, "Android destination picker returned");
                    DestinationSelectionEvent { path, error: None }
                }
                Err(error) => {
                    tracing::warn!(%error, "Android destination picker failed");
                    DestinationSelectionEvent {
                        path: None,
                        error: Some(error),
                    }
                }
            };
            if let Err(error) = app_handle.emit("destination:selected", event) {
                tracing::warn!(%error, "Could not emit Android destination selection");
            }
        });
        Ok(())
    }

    #[cfg(not(target_os = "android"))]
    {
        let _ = app_handle;
        Err("Native destination picker is only available on Android".into())
    }
}

#[tauri::command]
pub fn open_send_file_picker(app_handle: tauri::AppHandle, multiple: bool) -> Result<(), String> {
    #[cfg(target_os = "android")]
    {
        use tauri::Manager;

        let picker = app_handle
            .state::<crate::platform::android_destination::AndroidDestination<tauri::Wry>>()
            .inner()
            .clone();
        tauri::async_runtime::spawn(async move {
            tracing::debug!(multiple, "Opening Android send file picker");
            let event = match picker.pick_files(multiple).await {
                Ok(files) => SendFileSelectionEvent { files, error: None },
                Err(error) => {
                    tracing::warn!(%error, "Android send file picker failed");
                    SendFileSelectionEvent {
                        files: None,
                        error: Some(error),
                    }
                }
            };
            if let Err(error) = app_handle.emit("files:selected", event) {
                tracing::warn!(%error, "Could not emit Android file selection");
            }
        });
        Ok(())
    }

    #[cfg(not(target_os = "android"))]
    {
        let _ = (app_handle, multiple);
        Err("Native send file picker is only available on Android".into())
    }
}

#[tauri::command]
pub async fn release_selected_files(
    app_handle: tauri::AppHandle,
    paths: Vec<String>,
) -> Result<(), String> {
    #[cfg(target_os = "android")]
    {
        release_android_selected_files(&app_handle, &paths).await
    }

    #[cfg(not(target_os = "android"))]
    {
        let _ = (app_handle, paths);
        Ok(())
    }
}

#[tauri::command]
pub async fn open_privacy_policy(app_handle: tauri::AppHandle) -> Result<(), String> {
    #[cfg(target_os = "android")]
    {
        use tauri::Manager;

        app_handle
            .state::<crate::platform::android_destination::AndroidDestination<tauri::Wry>>()
            .inner()
            .open_privacy_policy()
            .await
    }

    #[cfg(not(target_os = "android"))]
    {
        let _ = app_handle;
        Err("Native privacy policy opener is only available on Android".into())
    }
}

#[cfg(target_os = "android")]
async fn release_android_selected_files(
    app_handle: &tauri::AppHandle,
    paths: &[String],
) -> Result<(), String> {
    use tauri::Manager;

    app_handle
        .state::<crate::platform::android_destination::AndroidDestination<tauri::Wry>>()
        .inner()
        .release_files(paths)
        .await
}

#[tauri::command]
pub fn set_display_name(
    state: State<'_, AppState>,
    discovery: State<'_, DiscoveryHandle>,
    display_name: String,
) -> Result<DeviceIdentity, String> {
    let device = state
        .update_display_name(&display_name)
        .map_err(|error| error.to_string())?;
    discovery
        .update_identity(&device)
        .map_err(|error| error.to_string())?;
    Ok(device)
}

#[tauri::command]
pub fn set_peer_order(state: State<'_, AppState>, order: Vec<String>) -> Result<(), String> {
    let mut seen = std::collections::HashSet::new();
    let order = order
        .into_iter()
        .filter(|id| !id.is_empty() && seen.insert(id.clone()))
        .collect();
    state
        .update_settings(|s| s.peer_order = order)
        .map_err(|e| e.to_string())
}

/// Send files/folders to a peer by device_id.
/// The frontend passes peer_address and peer_port as fallback in case the peer
/// has already been removed from the backend's discovery map (mDNS TTL expired).
/// Returns the transfer_id so the frontend can track progress.
#[tauri::command]
pub async fn send_to_peer(
    app_handle: tauri::AppHandle,
    state: State<'_, AppState>,
    device_id: String,
    paths: Vec<String>,
    peer_address: Option<String>,
    peer_port: Option<u16>,
) -> Result<String, String> {
    // Try backend state first, fall back to frontend-provided address
    let socket_addr = if let Some(peer) = state.peers.get(&device_id) {
        if peer.protocol_version != crate::discovery::types::PROTOCOL_VERSION {
            return Err("Peer uses an incompatible protocol. Update both Dukto apps.".into());
        }
        let addr = peer
            .addresses
            .iter()
            .find(|ip| ip.is_ipv4())
            .ok_or("Peer has no IPv4 address")?;
        SocketAddr::new(*addr, peer.port)
    } else if let (Some(addr_str), Some(port)) = (&peer_address, peer_port) {
        let addr: std::net::IpAddr = addr_str
            .parse()
            .map_err(|e| format!("Invalid address: {}", e))?;
        SocketAddr::new(addr, port)
    } else {
        return Err(format!("Peer {} not found", device_id));
    };

    let sender = state.get_device();
    let transfer_id = uuid::Uuid::new_v4().to_string();
    let input_paths: Vec<PathBuf> = paths.iter().map(PathBuf::from).collect();
    #[cfg(target_os = "android")]
    let selected_paths = paths.clone();
    let control = state
        .transfer_registry
        .register(transfer_id.clone())
        .await?;
    let registry = state.transfer_registry.clone();

    #[derive(Serialize)]
    struct SendStarted {
        transfer_id: String,
        peer_device_id: String,
    }
    let _ = app_handle.emit(
        "transfer:send-started",
        &SendStarted {
            transfer_id: transfer_id.clone(),
            peer_device_id: device_id.clone(),
        },
    );

    let tid = transfer_id.clone();
    let handle = app_handle.clone();

    tracing::info!(
        transfer_id = %transfer_id,
        peer = %device_id,
        addr = %socket_addr,
        items = paths.len(),
        "Initiating transfer to peer"
    );

    // Each outgoing transfer owns its endpoint and cancellation control.
    tauri::async_runtime::spawn(async move {
        let result = async {
            let endpoint = create_endpoint("0.0.0.0:0".parse().unwrap())
                .map_err(|e| format!("Endpoint creation failed: {}", e))?;
            let setup = tokio::select! {
                biased;
                _ = control.cancelled() => Err("Transfer cancelled".to_string()),
                result = async {
                    let conn = endpoint
                        .connect(socket_addr, "localhost")
                        .map_err(|e| format!("Connect call failed: {}", e))?
                        .await
                        .map_err(|e| format!("Connection failed: {}", e))?;
                    if !control.attach_connection(conn.clone()).await {
                        return Err("Transfer cancelled".to_string());
                    }

                    let (mut send, mut recv) = conn
                        .open_bi()
                        .await
                        .map_err(|e| format!("Open bi failed: {}", e))?;

                    let noise = handshake_initiator(&mut send, &mut recv)
                        .await
                        .map_err(|e| format!("Noise handshake failed: {}", e))?;

                    tracing::info!(transfer_id = %tid, "Noise handshake done, sending files");
                    Ok::<_, String>((send, recv, noise))
                } => result,
            };
            let result = match setup {
                Ok((mut send, mut recv, mut noise)) => {
                    let handle_progress = handle.clone();
                    let handle_accepted = handle.clone();
                    let tid_accepted = tid.clone();
                    send_transfer_with_cancellation(
                        &mut send,
                        &mut recv,
                        &mut noise,
                        &input_paths,
                        &tid,
                        &sender,
                        CancellableSendCallbacks {
                            cancelled: control.cancelled(),
                            on_progress:
                                move |progress: &crate::protocol::types::TransferProgress| {
                                    let _ = handle_progress.emit("transfer:progress", progress);
                                },
                            on_accepted: move || {
                                #[derive(Serialize)]
                                struct SendAccepted {
                                    transfer_id: String,
                                }
                                let _ = handle_accepted.emit(
                                    "transfer:send-accepted",
                                    &SendAccepted {
                                        transfer_id: tid_accepted,
                                    },
                                );
                            },
                        },
                    )
                    .await
                    .map_err(|e| format!("Transfer failed: {}", e))
                }
                Err(error) => Err(error),
            };
            if control.is_cancelled() {
                endpoint.close(
                    crate::protocol::types::TRANSFER_CANCEL_CLOSE_CODE.into(),
                    crate::protocol::types::TRANSFER_CANCEL_CLOSE_REASON,
                );
            } else {
                endpoint.close(0u32.into(), b"transfer finished");
            }
            let _ =
                tokio::time::timeout(std::time::Duration::from_secs(5), endpoint.wait_idle()).await;
            result
        }
        .await;
        registry.remove(&tid, &control).await;
        control.close_connection().await;

        #[cfg(target_os = "android")]
        if let Err(error) = release_android_selected_files(&handle, &selected_paths).await {
            tracing::warn!(%error, transfer_id = %tid, "Could not release selected Android files");
        }

        if control.is_cancelled() {
            tracing::info!(transfer_id = %tid, "Transfer cancelled");
            return;
        }

        match result {
            Ok(bytes) => {
                tracing::info!(transfer_id = %tid, bytes = bytes, "Transfer sent successfully");
                #[derive(Serialize)]
                struct SendComplete {
                    transfer_id: String,
                    bytes_sent: u64,
                }
                let _ = handle.emit(
                    "transfer:send-complete",
                    &SendComplete {
                        transfer_id: tid,
                        bytes_sent: bytes,
                    },
                );
            }
            Err(e) => {
                tracing::error!(transfer_id = %tid, error = %e, "Transfer send failed");
                #[derive(Serialize)]
                struct SendError {
                    transfer_id: String,
                    error: String,
                }
                let _ = handle.emit(
                    "transfer:send-error",
                    &SendError {
                        transfer_id: tid,
                        error: e,
                    },
                );
            }
        }
    });

    Ok(transfer_id)
}

/// Cancel exactly one active incoming or outgoing transfer.
#[tauri::command]
pub async fn cancel_transfer(
    app_handle: tauri::AppHandle,
    state: State<'_, AppState>,
    transfer_id: String,
) -> Result<(), String> {
    if !state.transfer_registry.cancel(&transfer_id).await {
        return Err(format!("Transfer {transfer_id} is not active"));
    }
    let _ = app_handle.emit("transfer:cancelled", &transfer_id);
    Ok(())
}

/// Respond to an incoming transfer request (accept or reject).
#[tauri::command]
pub async fn respond_transfer(
    server: State<'_, Arc<TransferServer>>,
    transfer_id: String,
    accepted: bool,
) -> Result<(), String> {
    server.respond(&transfer_id, accepted).await;
    Ok(())
}

/// Metadata about a file or directory, returned to the frontend.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileMetadataInfo {
    pub name: String,
    pub path: String,
    pub size: u64,
    pub is_dir: bool,
}

/// Resolve metadata for a list of file/directory paths.
/// Used by the frontend after drag-and-drop or file picker selection.
#[tauri::command]
pub async fn resolve_file_metadata(paths: Vec<String>) -> Result<Vec<FileMetadataInfo>, String> {
    let mut results = Vec::with_capacity(paths.len());

    for path_str in &paths {
        let path = std::path::Path::new(path_str);
        let metadata = tokio::fs::metadata(path)
            .await
            .map_err(|e| format!("Cannot read {}: {}", path_str, e))?;

        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();

        let size = if metadata.is_dir() {
            dir_size(path).await.unwrap_or(0)
        } else {
            metadata.len()
        };

        results.push(FileMetadataInfo {
            name,
            path: path_str.clone(),
            size,
            is_dir: metadata.is_dir(),
        });
    }

    Ok(results)
}

/// Compute total size of a directory recursively.
async fn dir_size(path: &std::path::Path) -> Result<u64, std::io::Error> {
    let mut total: u64 = 0;
    let mut stack = vec![path.to_path_buf()];

    while let Some(dir) = stack.pop() {
        let mut entries = tokio::fs::read_dir(&dir).await?;
        while let Some(entry) = entries.next_entry().await? {
            let meta = entry.metadata().await?;
            if meta.is_file() {
                total += meta.len();
            } else if meta.is_dir() {
                stack.push(entry.path());
            }
        }
    }

    Ok(total)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(suffix: &str) -> std::path::PathBuf {
        let dir =
            std::env::temp_dir().join(format!("duto-cmd-test-{}-{}", suffix, uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[tokio::test]
    async fn resolve_metadata_for_file() {
        let dir = temp_dir("meta-file");
        let file = dir.join("test.txt");
        tokio::fs::write(&file, "hello world").await.unwrap();

        let result = resolve_file_metadata(vec![file.to_string_lossy().to_string()])
            .await
            .unwrap();

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].name, "test.txt");
        assert_eq!(result[0].size, 11);
        assert!(!result[0].is_dir);

        tokio::fs::remove_dir_all(&dir).await.ok();
    }

    #[tokio::test]
    async fn resolve_metadata_for_directory() {
        let dir = temp_dir("meta-dir");
        let sub = dir.join("folder");
        tokio::fs::create_dir_all(&sub).await.unwrap();
        tokio::fs::write(sub.join("a.txt"), "aaa").await.unwrap();
        tokio::fs::write(sub.join("b.txt"), "bbbbbb").await.unwrap();

        let result = resolve_file_metadata(vec![sub.to_string_lossy().to_string()])
            .await
            .unwrap();

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].name, "folder");
        assert_eq!(result[0].size, 9); // 3 + 6
        assert!(result[0].is_dir);

        tokio::fs::remove_dir_all(&dir).await.ok();
    }

    #[tokio::test]
    async fn resolve_metadata_mixed() {
        let dir = temp_dir("meta-mixed");
        let file = dir.join("single.txt");
        let folder = dir.join("myfolder");
        tokio::fs::write(&file, "data").await.unwrap();
        tokio::fs::create_dir_all(&folder).await.unwrap();
        tokio::fs::write(folder.join("inner.txt"), "inner")
            .await
            .unwrap();

        let result = resolve_file_metadata(vec![
            file.to_string_lossy().to_string(),
            folder.to_string_lossy().to_string(),
        ])
        .await
        .unwrap();

        assert_eq!(result.len(), 2);
        assert_eq!(result[0].name, "single.txt");
        assert!(!result[0].is_dir);
        assert_eq!(result[1].name, "myfolder");
        assert!(result[1].is_dir);

        tokio::fs::remove_dir_all(&dir).await.ok();
    }

    #[tokio::test]
    async fn resolve_metadata_nonexistent_fails() {
        let result = resolve_file_metadata(vec!["/tmp/duto-nonexistent-xyz".into()]).await;
        assert!(result.is_err());
    }
}

pub mod updater;

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tauri::{Emitter, State};

use crate::crypto::noise::handshake_initiator;
use crate::state::app_state::AppState;
use crate::state::device::DeviceIdentity;
use crate::state::settings::Settings;
use crate::transfer::quic::create_endpoint;
use crate::transfer::sender::send_transfer;
use crate::transfer::server::TransferServer;

#[tauri::command]
pub fn get_device_info(state: State<'_, AppState>) -> DeviceIdentity {
    state.device.clone()
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
    state
        .update_settings(|s| {
            s.destination_dir = path;
        })
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

    let sender = state.device.clone();
    let transfer_id = uuid::Uuid::new_v4().to_string();
    let input_paths: Vec<PathBuf> = paths.iter().map(PathBuf::from).collect();
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
            let result = tokio::select! {
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

                    let mut noise = handshake_initiator(&mut send, &mut recv)
                        .await
                        .map_err(|e| format!("Noise handshake failed: {}", e))?;

                    tracing::info!(transfer_id = %tid, "Noise handshake done, sending files");

                    let handle_progress = handle.clone();
                    let bytes = send_transfer(
                        &mut send,
                        &mut recv,
                        &mut noise,
                        &input_paths,
                        &tid,
                        &sender,
                        move |progress| {
                            let _ = handle_progress.emit("transfer:progress", progress);
                        },
                    )
                    .await
                    .map_err(|e| format!("Transfer failed: {}", e))?;

                    Ok::<u64, String>(bytes)
                } => result,
            };
            endpoint.close(0u32.into(), b"transfer finished");
            result
        }
        .await;
        registry.remove(&tid, &control).await;
        control.close_connection().await;

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

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::{mpsc, Mutex};

use crate::crypto::noise::handshake_responder;
use crate::protocol::framing::decode_packet_header;
use crate::protocol::types::*;
use crate::state::app_state::AppState;
use crate::transfer::quic::create_endpoint;

/// Event emitted to the frontend when an incoming transfer request arrives.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IncomingTransferRequest {
    pub transfer_id: String,
    pub sender_device_id: String,
    pub item_count: u32,
    pub total_size: u64,
}

/// Accept/reject decision channel per transfer.
type AcceptDecision = bool;

/// Active pending incoming transfer.
struct PendingIncoming {
    tx: mpsc::Sender<AcceptDecision>,
}

/// Manages the QUIC listener and pending incoming transfers.
pub struct TransferServer {
    pending: Arc<Mutex<std::collections::HashMap<String, PendingIncoming>>>,
}

impl TransferServer {
    /// Start the QUIC listener on the given port and process incoming connections.
    /// Must be called from a context where tauri::async_runtime is available.
    pub fn start(app_handle: AppHandle, port: u16) -> Arc<Self> {
        let server = Arc::new(Self {
            pending: Arc::new(Mutex::new(std::collections::HashMap::new())),
        });

        let server_ref = server.clone();
        tauri::async_runtime::spawn(async move {
            let bind_addr: SocketAddr = format!("0.0.0.0:{}", port).parse().unwrap();
            let endpoint = match create_endpoint(bind_addr) {
                Ok(ep) => ep,
                Err(e) => {
                    tracing::error!("Failed to create QUIC endpoint: {}", e);
                    return;
                }
            };
            tracing::info!(port = port, "QUIC listener started");

            loop {
                match endpoint.accept().await {
                    Some(incoming) => {
                        let handle = app_handle.clone();
                        let pending = server_ref.pending.clone();
                        tauri::async_runtime::spawn(async move {
                            if let Err(e) = handle_incoming(handle, pending, incoming).await {
                                tracing::error!("Incoming transfer failed: {}", e);
                            }
                        });
                    }
                    None => {
                        tracing::info!("QUIC endpoint closed");
                        break;
                    }
                }
            }
        });

        server
    }

    /// Respond to a pending incoming transfer.
    pub async fn respond(&self, transfer_id: &str, accepted: bool) {
        let mut pending = self.pending.lock().await;
        if let Some(p) = pending.remove(transfer_id) {
            let _ = p.tx.send(accepted).await;
        }
    }
}

/// Handle a single incoming QUIC connection.
async fn handle_incoming(
    app_handle: AppHandle,
    pending: Arc<Mutex<std::collections::HashMap<String, PendingIncoming>>>,
    incoming: quinn::Incoming,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let conn = incoming.await?;
    tracing::info!(remote = %conn.remote_address(), "Incoming QUIC connection");

    let (mut send, mut recv) = conn.accept_bi().await?;

    // Noise handshake
    let mut noise = handshake_responder(&mut send, &mut recv).await?;
    tracing::info!("Noise handshake completed (responder)");

    // Read TransferHeader
    let data = recv_encrypted(&mut recv, &mut noise).await?;
    let (ptype, payload) = decode_packet_header(&data)?;
    if ptype != PacketType::TransferHeader {
        return Err(format!("Expected TransferHeader, got {:?}", ptype).into());
    }
    let header: TransferHeader = serde_json::from_slice(payload)?;

    tracing::info!(
        transfer_id = %header.transfer_id,
        sender = %header.sender_device_id,
        items = header.item_count,
        size = header.total_size,
        "Incoming transfer request"
    );

    // Send native notification
    use tauri_plugin_notification::NotificationExt;
    let items_label = if header.item_count == 1 { "item" } else { "items" };
    let _ = app_handle
        .notification()
        .builder()
        .title("Incoming transfer")
        .body(format!("{} {} from another device", header.item_count, items_label))
        .show();

    // Emit to frontend and wait for accept/reject
    let request = IncomingTransferRequest {
        transfer_id: header.transfer_id.clone(),
        sender_device_id: header.sender_device_id.clone(),
        item_count: header.item_count,
        total_size: header.total_size,
    };
    let _ = app_handle.emit("transfer:incoming", &request);

    // Create a channel for the accept/reject decision
    let (tx, mut rx) = mpsc::channel::<AcceptDecision>(1);
    {
        let mut map = pending.lock().await;
        map.insert(header.transfer_id.clone(), PendingIncoming { tx });
    }

    // Wait for decision (timeout 60s)
    let accepted = tokio::time::timeout(std::time::Duration::from_secs(60), rx.recv())
        .await
        .unwrap_or(Some(false))
        .unwrap_or(false);

    // Get destination dir from settings
    let dest_dir = {
        let state = app_handle.state::<AppState>();
        PathBuf::from(&state.get_settings().destination_dir)
    };

    // Send AcceptReject response
    let response = AcceptRejectResponse {
        accepted,
        destination_dir: Some(dest_dir.to_string_lossy().to_string()),
    };
    send_encrypted_json(&mut send, &mut noise, PacketType::AcceptReject, &response).await?;

    if !accepted {
        tracing::info!(transfer_id = %header.transfer_id, "Transfer rejected");
        let _ = app_handle.emit("transfer:rejected", &header.transfer_id);
        conn.close(0u32.into(), b"rejected");
        return Ok(());
    }

    tracing::info!(transfer_id = %header.transfer_id, "Transfer accepted, receiving items");

    // Receive items
    let mut items_received: u32 = 0;
    let mut bytes_received: u64 = 0;

    loop {
        let data = recv_encrypted(&mut recv, &mut noise).await?;
        let (ptype, payload) = decode_packet_header(&data)?;

        match ptype {
            PacketType::ItemMetadata => {
                let item: ItemMetadata = serde_json::from_slice(payload)?;
                let rel_path = item.relative_path.as_deref().unwrap_or(&item.name);

                use crate::transfer::fs::{resolve_conflict, sanitize_name, validate_relative_path};

                validate_relative_path(rel_path)
                    .map_err(|e| -> Box<dyn std::error::Error + Send + Sync> { e.into() })?;

                let sanitized: String = rel_path
                    .split('/')
                    .map(|part| sanitize_name(part))
                    .collect::<Vec<_>>()
                    .join("/");

                let dest_path = dest_dir.join(&sanitized);

                match item.kind {
                    ItemKind::Directory => {
                        let final_path = resolve_conflict(&dest_path);
                        tokio::fs::create_dir_all(&final_path).await?;
                        items_received += 1;
                    }
                    ItemKind::File => {
                        if let Some(parent) = dest_path.parent() {
                            tokio::fs::create_dir_all(parent).await?;
                        }
                        let final_path = resolve_conflict(&dest_path);
                        let mut file = tokio::fs::File::create(&final_path).await?;
                        let mut item_bytes: u64 = 0;

                        while item_bytes < item.size_bytes {
                            let data = recv_encrypted(&mut recv, &mut noise).await?;
                            let (ptype, payload) = decode_packet_header(&data)?;
                            if ptype != PacketType::Binary {
                                return Err(format!("Expected Binary, got {:?}", ptype).into());
                            }
                            tokio::io::AsyncWriteExt::write_all(&mut file, payload).await?;
                            item_bytes += payload.len() as u64;
                            bytes_received += payload.len() as u64;

                            // Emit progress
                            let progress = TransferProgress {
                                transfer_id: header.transfer_id.clone(),
                                bytes_sent: bytes_received,
                                bytes_total: header.total_size,
                                speed_bps: 0, // simplified for now
                                percent: if header.total_size > 0 {
                                    (bytes_received as f64 / header.total_size as f64) * 100.0
                                } else {
                                    100.0
                                },
                            };
                            let _ = app_handle.emit("transfer:progress", &progress);
                        }

                        tokio::io::AsyncWriteExt::flush(&mut file).await?;
                        items_received += 1;
                    }
                }
            }
            PacketType::Success => {
                break;
            }
            PacketType::TransferError => {
                let msg = String::from_utf8_lossy(payload);
                return Err(format!("Transfer error from sender: {}", msg).into());
            }
            other => {
                return Err(format!("Unexpected packet type: {:?}", other).into());
            }
        }
    }

    tracing::info!(
        transfer_id = %header.transfer_id,
        items = items_received,
        bytes = bytes_received,
        "Transfer completed"
    );

    #[derive(Serialize)]
    struct TransferComplete {
        transfer_id: String,
        items_received: u32,
        bytes_received: u64,
    }
    let _ = app_handle.emit(
        "transfer:complete",
        &TransferComplete {
            transfer_id: header.transfer_id,
            items_received,
            bytes_received,
        },
    );

    conn.close(0u32.into(), b"done");
    Ok(())
}

/// Send an encrypted JSON packet.
async fn send_encrypted_json<T: serde::Serialize>(
    send: &mut quinn::SendStream,
    noise: &mut snow::TransportState,
    ptype: PacketType,
    value: &T,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let json = serde_json::to_vec(value)?;
    let mut plaintext = Vec::with_capacity(1 + json.len());
    plaintext.push(ptype as u8);
    plaintext.extend_from_slice(&json);
    let mut encrypted = vec![0u8; plaintext.len() + 64];
    let len = noise.write_message(&plaintext, &mut encrypted)?;
    crate::crypto::noise::send_framed(send, &encrypted[..len]).await?;
    Ok(())
}

/// Receive and decrypt a packet.
async fn recv_encrypted(
    recv: &mut quinn::RecvStream,
    noise: &mut snow::TransportState,
) -> Result<Vec<u8>, Box<dyn std::error::Error + Send + Sync>> {
    let encrypted = crate::crypto::noise::recv_framed(recv).await?;
    let mut decrypted = vec![0u8; encrypted.len() + 128];
    let len = noise.read_message(&encrypted, &mut decrypted)?;
    decrypted.truncate(len);
    Ok(decrypted)
}

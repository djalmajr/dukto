use snow::TransportState;
use std::path::Path;
use tokio::io::AsyncWriteExt;

use crate::crypto::noise::{recv_framed, send_framed};
use crate::protocol::framing::decode_packet_header;
use crate::protocol::types::*;
use crate::transfer::fs::{
    create_conflict_free_file, resolve_conflict, sanitize_name, validate_relative_path,
};

/// Result of a completed transfer.
#[derive(Debug, serde::Serialize)]
pub struct ReceiveResult {
    pub transfer_id: String,
    pub items_received: u32,
    pub bytes_received: u64,
}

/// Receive a transfer (single or multi-item) over an established Noise-encrypted QUIC session.
pub async fn receive_transfer(
    send: &mut quinn::SendStream,
    recv: &mut quinn::RecvStream,
    noise: &mut TransportState,
    destination_dir: &Path,
    auto_accept: bool,
) -> Result<ReceiveResult, Box<dyn std::error::Error + Send + Sync>> {
    receive_transfer_with_accept(
        send,
        recv,
        noise,
        destination_dir,
        |_| async move { auto_accept },
        |_| {},
    )
    .await
}

/// Shared CLI/desktop receiver: approval and progress belong to the caller.
pub async fn receive_transfer_with_accept<F, Fut, P>(
    send: &mut quinn::SendStream,
    recv: &mut quinn::RecvStream,
    noise: &mut TransportState,
    destination_dir: &Path,
    approve: F,
    mut on_progress: P,
) -> Result<ReceiveResult, Box<dyn std::error::Error + Send + Sync>>
where
    F: FnOnce(TransferHeader) -> Fut,
    Fut: std::future::Future<Output = bool>,
    P: FnMut(&TransferProgress),
{
    // 1. Receive TransferHeader
    let data = recv_encrypted(recv, noise).await?;
    let (ptype, payload) = decode_packet_header(&data)?;
    if ptype != PacketType::TransferHeader {
        return Err(format!("Expected TransferHeader, got {:?}", ptype).into());
    }
    let header: TransferHeader = serde_json::from_slice(payload)?;
    if header
        .sender
        .as_ref()
        .is_some_and(|sender| sender.device_id != header.sender_device_id)
    {
        return Err("Sender identity does not match sender_device_id".into());
    }
    let accepted = approve(header.clone()).await;

    // 2. Send AcceptReject
    let response = AcceptRejectResponse {
        accepted,
        destination_dir: Some(destination_dir.to_string_lossy().to_string()),
    };
    send_encrypted_json(send, noise, PacketType::AcceptReject, &response).await?;

    if !accepted {
        send.finish()?;
        let _ = tokio::time::timeout(std::time::Duration::from_secs(5), send.stopped()).await;
        return Err("Transfer rejected".into());
    }

    tokio::fs::create_dir_all(destination_dir).await?;

    // 3. Receive items
    let mut items_received: u32 = 0;
    let mut bytes_received: u64 = 0;
    // Measure transfer time only after approval, across all files in this session.
    let started = std::time::Instant::now();

    loop {
        let data = recv_encrypted(recv, noise).await?;
        let (ptype, payload) = decode_packet_header(&data)?;

        match ptype {
            PacketType::ItemMetadata => {
                if items_received >= header.item_count {
                    return Err("Received more items than advertised".into());
                }
                let item: ItemMetadata = serde_json::from_slice(payload)?;

                // Validate and sanitize the path
                let rel_path = item.relative_path.as_deref().unwrap_or(&item.name);
                validate_relative_path(rel_path)
                    .map_err(|e| -> Box<dyn std::error::Error + Send + Sync> { e.into() })?;

                // Sanitize each component of the path
                let sanitized: String = rel_path
                    .split('/')
                    .map(|part| sanitize_name(part))
                    .collect::<Vec<_>>()
                    .join("/");

                let dest_path = destination_dir.join(&sanitized);

                match item.kind {
                    ItemKind::Directory => {
                        let final_path = resolve_conflict(&dest_path);
                        tokio::fs::create_dir_all(&final_path).await?;
                        items_received += 1;
                    }
                    ItemKind::File => {
                        // Ensure parent dir exists
                        if let Some(parent) = dest_path.parent() {
                            tokio::fs::create_dir_all(parent).await?;
                        }

                        let (_final_path, mut file) = create_conflict_free_file(&dest_path).await?;

                        receive_item_chunks(
                            recv,
                            noise,
                            &mut file,
                            item.size_bytes,
                            &mut bytes_received,
                            &header,
                            &mut |bytes_received| {
                                let elapsed = started.elapsed().as_secs_f64();
                                on_progress(&TransferProgress {
                                    transfer_id: header.transfer_id.clone(),
                                    bytes_sent: bytes_received,
                                    bytes_total: header.total_size,
                                    speed_bps: if elapsed > 0.0 {
                                        (bytes_received as f64 / elapsed) as u64
                                    } else {
                                        0
                                    },
                                    percent: if header.total_size == 0 {
                                        100.0
                                    } else {
                                        bytes_received as f64 / header.total_size as f64 * 100.0
                                    },
                                });
                            },
                        )
                        .await?;

                        file.flush().await?;
                        items_received += 1;
                    }
                }
            }
            PacketType::Success => {
                break;
            }
            PacketType::TransferError => {
                let msg = String::from_utf8_lossy(payload);
                return Err(format!("Transfer error: {}", msg).into());
            }
            other => {
                return Err(format!("Unexpected packet type: {:?}", other).into());
            }
        }
    }

    if items_received != header.item_count || bytes_received != header.total_size {
        return Err("Received item count or byte count does not match transfer header".into());
    }
    let receipt = TransferReceipt {
        transfer_id: header.transfer_id.clone(),
        items_received,
        bytes_received,
    };
    send_encrypted_json(send, noise, PacketType::Success, &receipt).await?;
    send.finish()?;
    // Keep the response alive until delivered; the sender can close after reading it.
    let _ = tokio::time::timeout(std::time::Duration::from_secs(5), send.stopped()).await;
    Ok(ReceiveResult {
        transfer_id: header.transfer_id,
        items_received,
        bytes_received,
    })
}

/// Receive binary chunks for a single file item, reading exactly `expected_bytes`.
async fn receive_item_chunks<P: FnMut(u64)>(
    recv: &mut quinn::RecvStream,
    noise: &mut TransportState,
    file: &mut tokio::fs::File,
    expected_bytes: u64,
    total_bytes: &mut u64,
    header: &TransferHeader,
    on_progress: &mut P,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut item_bytes: u64 = 0;

    while item_bytes < expected_bytes {
        let data = recv_encrypted(recv, noise).await?;
        let (ptype, payload) = decode_packet_header(&data)?;

        match ptype {
            PacketType::Binary => {
                if payload.is_empty()
                    || payload.len() as u64 > expected_bytes - item_bytes
                    || payload.len() as u64 > header.total_size.saturating_sub(*total_bytes)
                {
                    return Err("Binary chunk exceeds advertised size or is empty".into());
                }
                file.write_all(payload).await?;
                item_bytes += payload.len() as u64;
                *total_bytes += payload.len() as u64;
                on_progress(*total_bytes);
            }
            other => {
                return Err(format!("Expected Binary chunk, got {:?}", other).into());
            }
        }
    }

    Ok(())
}

/// Backward-compatible wrapper for single-file receive.
pub async fn receive_file(
    send: &mut quinn::SendStream,
    recv: &mut quinn::RecvStream,
    noise: &mut TransportState,
    destination_dir: &Path,
    auto_accept: bool,
) -> Result<(String, u64), Box<dyn std::error::Error + Send + Sync>> {
    let result = receive_transfer(send, recv, noise, destination_dir, auto_accept).await?;
    // For backward compat, return a generic name
    Ok(("transfer".into(), result.bytes_received))
}

/// Send an encrypted JSON packet.
async fn send_encrypted_json<T: serde::Serialize>(
    send: &mut quinn::SendStream,
    noise: &mut TransportState,
    ptype: PacketType,
    value: &T,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let json = serde_json::to_vec(value)?;
    let mut plaintext = Vec::with_capacity(1 + json.len());
    plaintext.push(ptype as u8);
    plaintext.extend_from_slice(&json);
    let mut encrypted = vec![0u8; plaintext.len() + 64];
    let len = noise.write_message(&plaintext, &mut encrypted)?;
    send_framed(send, &encrypted[..len]).await?;
    Ok(())
}

/// Receive and decrypt a packet.
async fn recv_encrypted(
    recv: &mut quinn::RecvStream,
    noise: &mut TransportState,
) -> Result<Vec<u8>, Box<dyn std::error::Error + Send + Sync>> {
    let encrypted = recv_framed(recv).await?;
    let mut decrypted = vec![0u8; encrypted.len() + 128];
    let len = noise.read_message(&encrypted, &mut decrypted)?;
    decrypted.truncate(len);
    Ok(decrypted)
}

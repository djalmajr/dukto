use snow::TransportState;
use std::path::Path;
use tokio::io::AsyncWriteExt;

use crate::crypto::noise::{recv_framed, send_framed};
use crate::protocol::framing::decode_packet_header;
use crate::protocol::types::*;
use crate::transfer::fs::{resolve_conflict, sanitize_name, validate_relative_path};

/// Result of a completed transfer.
#[derive(Debug)]
pub struct ReceiveResult {
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
    // 1. Receive TransferHeader
    let data = recv_encrypted(recv, noise).await?;
    let (ptype, payload) = decode_packet_header(&data)?;
    if ptype != PacketType::TransferHeader {
        return Err(format!("Expected TransferHeader, got {:?}", ptype).into());
    }
    let _header: TransferHeader = serde_json::from_slice(payload)?;

    // 2. Send AcceptReject
    let response = AcceptRejectResponse {
        accepted: auto_accept,
        destination_dir: Some(destination_dir.to_string_lossy().to_string()),
    };
    send_encrypted_json(send, noise, PacketType::AcceptReject, &response).await?;

    if !auto_accept {
        return Err("Transfer rejected".into());
    }

    tokio::fs::create_dir_all(destination_dir).await?;

    // 3. Receive items
    let mut items_received: u32 = 0;
    let mut bytes_received: u64 = 0;

    loop {
        let data = recv_encrypted(recv, noise).await?;
        let (ptype, payload) = decode_packet_header(&data)?;

        match ptype {
            PacketType::ItemMetadata => {
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

                        let final_path = resolve_conflict(&dest_path);
                        let mut file = tokio::fs::File::create(&final_path).await?;

                        receive_item_chunks(
                            recv, noise, &mut file, item.size_bytes, &mut bytes_received,
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

    Ok(ReceiveResult {
        items_received,
        bytes_received,
    })
}

/// Receive binary chunks for a single file item, reading exactly `expected_bytes`.
async fn receive_item_chunks(
    recv: &mut quinn::RecvStream,
    noise: &mut TransportState,
    file: &mut tokio::fs::File,
    expected_bytes: u64,
    total_bytes: &mut u64,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut item_bytes: u64 = 0;

    while item_bytes < expected_bytes {
        let data = recv_encrypted(recv, noise).await?;
        let (ptype, payload) = decode_packet_header(&data)?;

        match ptype {
            PacketType::Binary => {
                file.write_all(payload).await?;
                item_bytes += payload.len() as u64;
                *total_bytes += payload.len() as u64;
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

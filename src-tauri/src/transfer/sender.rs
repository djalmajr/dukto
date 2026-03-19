use snow::TransportState;
use std::path::{Path, PathBuf};
use tokio::io::AsyncReadExt;

use crate::crypto::noise::{recv_framed, send_framed};
use crate::protocol::framing::decode_packet_header;
use crate::protocol::types::*;
use crate::transfer::fs::{walk_directory, WalkItem};

// Noise max message is 65535. Keep chunks under the limit with overhead.
const CHUNK_SIZE: usize = 32 * 1024; // 32 KB

/// Send one or more files/folders over an established Noise-encrypted QUIC session.
/// `inputs` can be files or directories; directories are walked recursively.
/// `on_progress` is called after each chunk with a `TransferProgress` snapshot.
/// Returns total bytes sent.
pub async fn send_transfer<F>(
    send: &mut quinn::SendStream,
    recv: &mut quinn::RecvStream,
    noise: &mut TransportState,
    inputs: &[PathBuf],
    transfer_id: &str,
    sender_device_id: &str,
    mut on_progress: F,
) -> Result<u64, Box<dyn std::error::Error + Send + Sync>>
where
    F: FnMut(&TransferProgress),
{
    // 1. Build flat item list from all inputs
    let items = collect_items(inputs)?;
    let item_count = items.len() as u32;
    let total_size: u64 = items.iter().map(|i| i.size).sum();

    // 2. Send TransferHeader
    let header = TransferHeader {
        transfer_id: transfer_id.to_string(),
        sender_device_id: sender_device_id.to_string(),
        item_count,
        total_size,
    };
    send_encrypted_json(send, noise, PacketType::TransferHeader, &header).await?;

    // 3. Wait for AcceptReject
    let response_data = recv_encrypted(recv, noise).await?;
    let (ptype, payload) = decode_packet_header(&response_data)?;
    if ptype != PacketType::AcceptReject {
        return Err(format!("Expected AcceptReject, got {:?}", ptype).into());
    }
    let response: AcceptRejectResponse = serde_json::from_slice(payload)?;
    if !response.accepted {
        return Err("Transfer rejected by receiver".into());
    }

    // 4. Send items sequentially
    let mut total_sent: u64 = 0;
    let start = std::time::Instant::now();

    for item in &items {
        let item_meta = ItemMetadata {
            item_id: uuid::Uuid::new_v4().to_string(),
            kind: item.kind.clone(),
            name: item
                .absolute_path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown")
                .to_string(),
            relative_path: Some(item.relative_path.clone()),
            size_bytes: item.size,
            modified_at: std::fs::metadata(&item.absolute_path)
                .ok()
                .and_then(|m| m.modified().ok())
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs()),
        };
        send_encrypted_json(send, noise, PacketType::ItemMetadata, &item_meta).await?;

        // Only send binary data for files
        if item.kind == ItemKind::File {
            let mut file = tokio::fs::File::open(&item.absolute_path).await?;
            let mut buf = vec![0u8; CHUNK_SIZE];

            loop {
                let n = file.read(&mut buf).await?;
                if n == 0 {
                    break;
                }
                send_encrypted_raw(send, noise, PacketType::Binary, &buf[..n]).await?;
                total_sent += n as u64;

                let elapsed = start.elapsed().as_secs_f64();
                let speed_bps = if elapsed > 0.0 {
                    (total_sent as f64 / elapsed) as u64
                } else {
                    0
                };
                let percent = if total_size > 0 {
                    (total_sent as f64 / total_size as f64) * 100.0
                } else {
                    100.0
                };

                on_progress(&TransferProgress {
                    transfer_id: transfer_id.to_string(),
                    bytes_sent: total_sent,
                    bytes_total: total_size,
                    speed_bps,
                    percent,
                });
            }
        }
    }

    // 5. Send Success
    send_encrypted_raw(send, noise, PacketType::Success, &[]).await?;

    Ok(total_sent)
}

/// Convenience wrapper for sending a single file (backward compatible).
pub async fn send_file<F>(
    send: &mut quinn::SendStream,
    recv: &mut quinn::RecvStream,
    noise: &mut TransportState,
    file_path: &Path,
    transfer_id: &str,
    sender_device_id: &str,
    on_progress: F,
) -> Result<u64, Box<dyn std::error::Error + Send + Sync>>
where
    F: FnMut(&TransferProgress),
{
    send_transfer(
        send,
        recv,
        noise,
        &[file_path.to_path_buf()],
        transfer_id,
        sender_device_id,
        on_progress,
    )
    .await
}

/// Collect all items from inputs (files and directories).
fn collect_items(inputs: &[PathBuf]) -> Result<Vec<WalkItem>, Box<dyn std::error::Error + Send + Sync>> {
    fn wrap(e: Box<dyn std::error::Error>) -> Box<dyn std::error::Error + Send + Sync> {
        e.to_string().into()
    }
    let mut all_items = Vec::new();

    for input in inputs {
        let metadata = std::fs::metadata(input)
            .map_err(|e| format!("Cannot read {}: {}", input.display(), e))?;

        if metadata.is_file() {
            let name = input
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown")
                .to_string();
            all_items.push(WalkItem {
                absolute_path: input.clone(),
                relative_path: name,
                kind: ItemKind::File,
                size: metadata.len(),
            });
        } else if metadata.is_dir() {
            let walked = walk_directory(input).map_err(wrap)?;
            let dir_name = input
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("folder");

            // Prefix relative paths with the directory name
            for mut item in walked {
                if item.relative_path == "." {
                    item.relative_path = dir_name.to_string();
                } else {
                    item.relative_path = format!("{}/{}", dir_name, item.relative_path);
                }
                all_items.push(item);
            }
        }
    }

    Ok(all_items)
}

/// Send an encrypted JSON packet.
async fn send_encrypted_json<T: serde::Serialize>(
    send: &mut quinn::SendStream,
    noise: &mut TransportState,
    ptype: PacketType,
    value: &T,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let json = serde_json::to_vec(value)?;
    send_encrypted_raw(send, noise, ptype, &json).await
}

/// Send an encrypted raw packet.
async fn send_encrypted_raw(
    send: &mut quinn::SendStream,
    noise: &mut TransportState,
    ptype: PacketType,
    payload: &[u8],
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut plaintext = Vec::with_capacity(1 + payload.len());
    plaintext.push(ptype as u8);
    plaintext.extend_from_slice(payload);

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

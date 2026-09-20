use snow::TransportState;
use std::{
    future::Future,
    path::{Path, PathBuf},
};
use tokio::io::AsyncReadExt;

use crate::crypto::noise::{
    is_remote_transfer_cancellation, recv_framed, send_framed, RemoteTransferCancelled,
    TransferCancelled,
};
use crate::protocol::framing::decode_packet_header;
use crate::protocol::types::*;
use crate::state::device::DeviceIdentity;
use crate::transfer::channel::{TransferReceiveStream, TransferSendStream};
use crate::transfer::fs::{walk_directory, WalkItem};

// Noise max message is 65535. Keep chunks under the limit with overhead.
const CHUNK_SIZE: usize = 32 * 1024; // 32 KB

/// Local cancellation, progress, and acceptance callbacks for one sender operation.
pub struct CancellableSendCallbacks<C, F, A = fn()> {
    pub cancelled: C,
    pub on_progress: F,
    pub on_accepted: A,
}

/// Send one or more files/folders over an established Noise-encrypted QUIC session.
/// `inputs` can be files or directories; directories are walked recursively.
/// `on_progress` is called after each chunk with a `TransferProgress` snapshot.
/// Returns total bytes sent.
pub async fn send_transfer<S, R, F>(
    send: &mut S,
    recv: &mut R,
    noise: &mut TransportState,
    inputs: &[PathBuf],
    transfer_id: &str,
    sender: &DeviceIdentity,
    on_progress: F,
) -> Result<u64, Box<dyn std::error::Error + Send + Sync>>
where
    S: TransferSendStream,
    R: TransferReceiveStream,
    F: FnMut(&TransferProgress),
{
    send_transfer_full(
        send,
        recv,
        noise,
        inputs,
        transfer_id,
        sender,
        on_progress,
        || {},
    )
    .await
}

/// Send one or more files/folders over an established Noise-encrypted QUIC session,
/// invoking `on_accepted` when the receiver confirms acceptance before item transmission begins.
#[allow(clippy::too_many_arguments)]
pub async fn send_transfer_full<S, R, F, A>(
    send: &mut S,
    recv: &mut R,
    noise: &mut TransportState,
    inputs: &[PathBuf],
    transfer_id: &str,
    sender: &DeviceIdentity,
    mut on_progress: F,
    on_accepted: A,
) -> Result<u64, Box<dyn std::error::Error + Send + Sync>>
where
    S: TransferSendStream,
    R: TransferReceiveStream,
    F: FnMut(&TransferProgress),
    A: FnOnce(),
{
    // 1. Build flat item list from all inputs
    let items = collect_items(inputs)?;
    if items.is_empty() {
        return Err("No transferable items selected".into());
    }
    let item_count = items.len() as u32;
    let total_size: u64 = items.iter().map(|i| i.size).sum();

    // 2. Send TransferHeader
    let header = TransferHeader {
        transfer_id: transfer_id.to_string(),
        sender_device_id: sender.device_id.clone(),
        sender: Some(sender.clone()),
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

    on_accepted();

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
    send.finish_transfer()?;

    // Do not report success or close QUIC while payloads are still in flight.
    let data = tokio::time::timeout(
        std::time::Duration::from_secs(30),
        recv_encrypted(recv, noise),
    )
    .await
    .map_err(|_| "Timed out waiting for receiver acknowledgement (requires protocol 0.2)")??;
    let (ptype, payload) = decode_packet_header(&data)?;
    if ptype != PacketType::Success {
        return Err(format!("Receiver did not confirm completion: {:?}", ptype).into());
    }
    let receipt: TransferReceipt = serde_json::from_slice(payload)?;
    if receipt.transfer_id != transfer_id
        || receipt.items_received != item_count
        || receipt.bytes_received != total_sent
    {
        return Err("Receiver acknowledgement does not match the transfer".into());
    }

    Ok(total_sent)
}

/// Send a transfer while handling both local cancellation and the peer's reliable
/// QUIC stream cancellation signal. The reverse stream carries the acknowledgement
/// when this side initiated the cancellation.
pub async fn send_transfer_with_cancellation<S, R, F, C, A>(
    send: &mut S,
    recv: &mut R,
    noise: &mut TransportState,
    inputs: &[PathBuf],
    transfer_id: &str,
    sender: &DeviceIdentity,
    callbacks: CancellableSendCallbacks<C, F, A>,
) -> Result<u64, Box<dyn std::error::Error + Send + Sync>>
where
    S: TransferSendStream,
    R: TransferReceiveStream,
    F: FnMut(&TransferProgress),
    C: Future<Output = ()>,
    A: FnOnce(),
{
    let CancellableSendCallbacks {
        cancelled,
        on_progress,
        on_accepted,
    } = callbacks;

    enum Outcome<T> {
        Complete(T),
        LocalCancellation,
        PeerStopped(u64),
    }

    let outcome = {
        let peer_stopped = send.stopped_transfer();
        tokio::pin!(peer_stopped);
        let transfer = send_transfer_full(
            send,
            recv,
            noise,
            inputs,
            transfer_id,
            sender,
            on_progress,
            on_accepted,
        );
        tokio::pin!(transfer);
        tokio::pin!(cancelled);
        let mut watch_peer_stop = true;

        loop {
            tokio::select! {
                biased;
                result = &mut transfer => break Outcome::Complete(result),
                stopped = &mut peer_stopped, if watch_peer_stop => {
                    match stopped {
                        Ok(Some(code)) => break Outcome::PeerStopped(code),
                        Ok(None) | Err(_) => watch_peer_stop = false,
                    }
                },
                _ = &mut cancelled => break Outcome::LocalCancellation,
            }
        }
    };

    match outcome {
        Outcome::Complete(Ok(bytes)) => Ok(bytes),
        Outcome::Complete(Err(error)) if is_remote_transfer_cancellation(error.as_ref()) => {
            // The peer reset the inbound stream or stopped this outbound stream.
            // Send both available stream signals so either case is acknowledged.
            send.reset_transfer(crate::protocol::types::TRANSFER_CANCEL_CLOSE_CODE);
            recv.stop_transfer(crate::protocol::types::TRANSFER_CANCEL_CLOSE_CODE);
            Err(Box::new(RemoteTransferCancelled))
        }
        Outcome::Complete(Err(error)) => Err(error),
        Outcome::PeerStopped(code)
            if code == u64::from(crate::protocol::types::TRANSFER_CANCEL_CLOSE_CODE) =>
        {
            recv.stop_transfer(crate::protocol::types::TRANSFER_CANCEL_CLOSE_CODE);
            Err(Box::new(RemoteTransferCancelled))
        }
        Outcome::PeerStopped(code) => {
            Err(format!("Peer stopped the transfer stream ({code})").into())
        }
        Outcome::LocalCancellation => {
            send.reset_transfer(crate::protocol::types::TRANSFER_CANCEL_CLOSE_CODE);
            let _ = tokio::time::timeout(
                std::time::Duration::from_secs(3),
                recv.received_reset_transfer(),
            )
            .await;
            Err(Box::new(TransferCancelled))
        }
    }
}

/// Convenience wrapper for sending a single file (backward compatible).
pub async fn send_file<S, R, F>(
    send: &mut S,
    recv: &mut R,
    noise: &mut TransportState,
    file_path: &Path,
    transfer_id: &str,
    sender: &DeviceIdentity,
    on_progress: F,
) -> Result<u64, Box<dyn std::error::Error + Send + Sync>>
where
    S: TransferSendStream,
    R: TransferReceiveStream,
    F: FnMut(&TransferProgress),
{
    send_transfer(
        send,
        recv,
        noise,
        &[file_path.to_path_buf()],
        transfer_id,
        sender,
        on_progress,
    )
    .await
}

/// Collect all items from inputs (files and directories).
fn collect_items(
    inputs: &[PathBuf],
) -> Result<Vec<WalkItem>, Box<dyn std::error::Error + Send + Sync>> {
    fn wrap(e: Box<dyn std::error::Error>) -> Box<dyn std::error::Error + Send + Sync> {
        e.to_string().into()
    }
    let mut all_items = Vec::new();

    for input in inputs {
        let metadata = std::fs::metadata(input)
            .map_err(|error| format!("Selected transfer input could not be read: {error}"))?;

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
async fn send_encrypted_json<S, T>(
    send: &mut S,
    noise: &mut TransportState,
    ptype: PacketType,
    value: &T,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>>
where
    S: TransferSendStream,
    T: serde::Serialize,
{
    let json = serde_json::to_vec(value)?;
    send_encrypted_raw(send, noise, ptype, &json).await
}

/// Send an encrypted raw packet.
async fn send_encrypted_raw<S>(
    send: &mut S,
    noise: &mut TransportState,
    ptype: PacketType,
    payload: &[u8],
) -> Result<(), Box<dyn std::error::Error + Send + Sync>>
where
    S: TransferSendStream,
{
    let mut plaintext = Vec::with_capacity(1 + payload.len());
    plaintext.push(ptype as u8);
    plaintext.extend_from_slice(payload);

    let mut encrypted = vec![0u8; plaintext.len() + 64];
    let len = noise.write_message(&plaintext, &mut encrypted)?;
    send_framed(send, &encrypted[..len]).await?;
    Ok(())
}

/// Receive and decrypt a packet.
async fn recv_encrypted<R>(
    recv: &mut R,
    noise: &mut TransportState,
) -> Result<Vec<u8>, Box<dyn std::error::Error + Send + Sync>>
where
    R: TransferReceiveStream,
{
    let encrypted = recv_framed(recv).await?;
    let mut decrypted = vec![0u8; encrypted.len() + 128];
    let len = noise.read_message(&encrypted, &mut decrypted)?;
    decrypted.truncate(len);
    Ok(decrypted)
}

#[cfg(test)]
mod tests {
    use super::collect_items;

    #[test]
    fn input_errors_do_not_echo_selected_paths() {
        let sentinel = "private-filename-sentinel";
        let path = std::env::temp_dir().join(sentinel);
        let error = collect_items(&[path]).unwrap_err().to_string();

        assert!(!error.contains(sentinel));
        assert!(error.starts_with("Selected transfer input could not be read:"));
    }
}

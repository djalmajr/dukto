use snow::TransportState;
use std::{future::Future, path::Path};
use tokio::io::AsyncWriteExt;
use tokio::sync::watch;

use crate::crypto::noise::{
    is_remote_transfer_cancellation, recv_framed, reset_transfer_send_stream, send_framed,
    stop_transfer_receive_stream, RemoteTransferCancelled, TransferCancelled,
};
use crate::protocol::framing::decode_packet_header;
use crate::protocol::types::*;
use crate::transfer::fs::resolve_conflict;
use crate::transfer::partial_file::create_partial_file;
use crate::transfer::safe_destination::resolve_safe_destination_path;

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
    on_progress: P,
) -> Result<ReceiveResult, Box<dyn std::error::Error + Send + Sync>>
where
    F: FnOnce(TransferHeader) -> Fut,
    Fut: std::future::Future<Output = bool>,
    P: FnMut(&TransferProgress),
{
    receive_transfer_inner(
        send,
        recv,
        noise,
        destination_dir,
        move |header, _| approve(header),
        on_progress,
    )
    .await
}

async fn receive_transfer_inner<F, Fut, P>(
    send: &mut quinn::SendStream,
    recv: &mut quinn::RecvStream,
    noise: &mut TransportState,
    destination_dir: &Path,
    approve: F,
    mut on_progress: P,
) -> Result<ReceiveResult, Box<dyn std::error::Error + Send + Sync>>
where
    F: FnOnce(TransferHeader, watch::Receiver<bool>) -> Fut,
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
    let (peer_cancelled, peer_cancel_receiver) = watch::channel(false);
    let approval = approve(header.clone(), peer_cancel_receiver);
    tokio::pin!(approval);
    let mut watch_peer_reset = true;
    let accepted = loop {
        tokio::select! {
            biased;
            accepted = &mut approval => break accepted,
            reset = recv.received_reset(), if watch_peer_reset => {
                match reset {
                    Ok(Some(code)) if code == TRANSFER_CANCEL_CLOSE_CODE.into() => {
                        peer_cancelled.send_replace(true);
                        // Give approval adapters time to remove their pending request before
                        // the caller sends the reliable reverse-stream acknowledgement.
                        let _ = tokio::time::timeout(
                            std::time::Duration::from_secs(2),
                            &mut approval,
                        ).await;
                        return Err(Box::new(RemoteTransferCancelled));
                    }
                    Ok(Some(code)) => return Err(format!("Transfer stream reset with code {code}").into()),
                    Ok(None) | Err(_) => watch_peer_reset = false,
                }
            }
        }
    };
    if *peer_cancelled.borrow() {
        return Err(Box::new(RemoteTransferCancelled));
    }

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

                // Validate, sanitize and resolve each parent beneath the selected root.
                let rel_path = item.relative_path.as_deref().unwrap_or(&item.name);
                let dest_path = resolve_safe_destination_path(destination_dir, rel_path).await?;

                match item.kind {
                    ItemKind::Directory => {
                        let final_path = resolve_conflict(&dest_path);
                        tokio::fs::create_dir_all(&final_path).await?;
                        items_received += 1;
                    }
                    ItemKind::File => {
                        let mut partial_file = create_partial_file(&dest_path).await?;

                        receive_item_chunks(
                            recv,
                            noise,
                            partial_file.file_mut(),
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

                        partial_file.commit_to(&dest_path).await?;
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

/// Receive a transfer while handling local cancellation and reliable peer reset/stop signals.
pub async fn receive_transfer_with_cancellation<F, Fut, P, C>(
    send: &mut quinn::SendStream,
    recv: &mut quinn::RecvStream,
    noise: &mut TransportState,
    destination_dir: &Path,
    approve: F,
    on_progress: P,
    cancelled: C,
) -> Result<ReceiveResult, Box<dyn std::error::Error + Send + Sync>>
where
    F: FnOnce(TransferHeader, watch::Receiver<bool>) -> Fut,
    Fut: std::future::Future<Output = bool>,
    P: FnMut(&TransferProgress),
    C: Future<Output = ()>,
{
    let result = {
        let receive =
            receive_transfer_inner(send, recv, noise, destination_dir, approve, on_progress);
        tokio::pin!(receive);
        tokio::pin!(cancelled);
        tokio::select! {
            biased;
            result = &mut receive => Some(result),
            _ = &mut cancelled => None,
        }
    };

    match result {
        Some(Ok(receipt)) => Ok(receipt),
        Some(Err(error)) if is_remote_transfer_cancellation(error.as_ref()) => {
            // A reset from the sender is acknowledged on our send half. A stop
            // from the sender is answered by stopping the corresponding receive half.
            reset_transfer_send_stream(send);
            stop_transfer_receive_stream(recv);
            Err(Box::new(RemoteTransferCancelled))
        }
        Some(Err(error)) => Err(error),
        None => {
            stop_transfer_receive_stream(recv);
            let _ = tokio::time::timeout(std::time::Duration::from_secs(3), send.stopped()).await;
            Err(Box::new(TransferCancelled))
        }
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto::noise::{
        close_for_transfer_cancellation, handshake_initiator, handshake_responder,
    };
    use crate::transfer::quic::create_endpoint;
    use std::net::SocketAddr;
    use tokio::sync::oneshot;

    fn temp_dir() -> std::path::PathBuf {
        let directory =
            std::env::temp_dir().join(format!("dukto-receive-cancel-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&directory).unwrap();
        directory
    }

    async fn interrupt_after_first_chunk(cancel: bool) -> (String, std::path::PathBuf) {
        let destination = temp_dir();
        let existing = destination.join("payload.bin");
        tokio::fs::write(&existing, b"preserve this existing file")
            .await
            .unwrap();

        let server = create_endpoint("127.0.0.1:0".parse::<SocketAddr>().unwrap()).unwrap();
        let address = server.local_addr().unwrap();
        let receive_destination = destination.clone();
        let (progress_tx, progress_rx) = oneshot::channel();
        let receiver = tokio::spawn(async move {
            let connection = server.accept().await.unwrap().await.unwrap();
            let (mut send, mut recv) = connection.accept_bi().await.unwrap();
            let mut noise = handshake_responder(&mut send, &mut recv).await.unwrap();
            let mut progress_tx = Some(progress_tx);
            let result = receive_transfer_with_accept(
                &mut send,
                &mut recv,
                &mut noise,
                &receive_destination,
                |_| async { true },
                |progress| {
                    if progress.bytes_sent > 0 {
                        if let Some(progress_tx) = progress_tx.take() {
                            let _ = progress_tx.send(());
                        }
                    }
                },
            )
            .await;
            result
        });

        let client = create_endpoint("127.0.0.1:0".parse::<SocketAddr>().unwrap()).unwrap();
        let connection = client.connect(address, "localhost").unwrap().await.unwrap();
        let (mut send, mut recv) = connection.open_bi().await.unwrap();
        let mut noise = handshake_initiator(&mut send, &mut recv).await.unwrap();

        let header = TransferHeader {
            transfer_id: "interrupted-transfer".into(),
            sender_device_id: "test-sender".into(),
            sender: None,
            item_count: 1,
            total_size: 1024 * 1024,
        };
        send_encrypted_json(&mut send, &mut noise, PacketType::TransferHeader, &header)
            .await
            .unwrap();
        let response = recv_encrypted(&mut recv, &mut noise).await.unwrap();
        assert_eq!(
            decode_packet_header(&response).unwrap().0,
            PacketType::AcceptReject
        );

        let metadata = ItemMetadata {
            item_id: "partial-file".into(),
            kind: ItemKind::File,
            name: "payload.bin".into(),
            relative_path: Some("payload.bin".into()),
            size_bytes: header.total_size,
            modified_at: None,
        };
        send_encrypted_json(&mut send, &mut noise, PacketType::ItemMetadata, &metadata)
            .await
            .unwrap();

        let payload = vec![b'X'; 32 * 1024];
        let mut plaintext = Vec::with_capacity(payload.len() + 1);
        plaintext.push(PacketType::Binary as u8);
        plaintext.extend_from_slice(&payload);
        let mut encrypted = vec![0; plaintext.len() + 64];
        let encrypted_len = noise.write_message(&plaintext, &mut encrypted).unwrap();
        send_framed(&mut send, &encrypted[..encrypted_len])
            .await
            .unwrap();

        tokio::time::timeout(std::time::Duration::from_secs(5), progress_rx)
            .await
            .unwrap()
            .unwrap();
        if cancel {
            close_for_transfer_cancellation(&connection);
        } else {
            connection.close(0xD0171u32.into(), b"test connection drop");
        }

        let error = tokio::time::timeout(std::time::Duration::from_secs(5), receiver)
            .await
            .unwrap()
            .unwrap()
            .unwrap_err()
            .to_string();
        client.close(0u32.into(), b"test complete");
        (error, destination)
    }

    #[tokio::test]
    async fn sender_cancellation_while_waiting_for_approval_is_typed_and_acknowledged() {
        let destination = temp_dir();
        let receive_destination = destination.clone();
        let server = create_endpoint("127.0.0.1:0".parse::<SocketAddr>().unwrap()).unwrap();
        let address = server.local_addr().unwrap();
        let (approval_started_tx, approval_started_rx) = oneshot::channel();
        let (receiver_result_tx, receiver_result_rx) = oneshot::channel();
        let (receiver_release_tx, receiver_release_rx) = oneshot::channel();
        let receiver = tokio::spawn(async move {
            let connection = server.accept().await.unwrap().await.unwrap();
            let (mut send, mut recv) = connection.accept_bi().await.unwrap();
            let mut noise = handshake_responder(&mut send, &mut recv).await.unwrap();
            let result = receive_transfer_with_cancellation(
                &mut send,
                &mut recv,
                &mut noise,
                &receive_destination,
                move |_, mut peer_cancelled| async move {
                    let _ = approval_started_tx.send(());
                    while !*peer_cancelled.borrow() {
                        if peer_cancelled.changed().await.is_err() {
                            break;
                        }
                    }
                    false
                },
                |_| {},
                std::future::pending(),
            )
            .await
            .map_err(|error| error.to_string());
            let _ = receiver_result_tx.send(result);
            let _ = receiver_release_rx.await;
        });

        let client = create_endpoint("127.0.0.1:0".parse::<SocketAddr>().unwrap()).unwrap();
        let connection = client.connect(address, "localhost").unwrap().await.unwrap();
        let (mut send, mut recv) = connection.open_bi().await.unwrap();
        let mut noise = handshake_initiator(&mut send, &mut recv).await.unwrap();
        let header = TransferHeader {
            transfer_id: "cancel-during-approval".into(),
            sender_device_id: "test-sender".into(),
            sender: None,
            item_count: 1,
            total_size: 1,
        };
        send_encrypted_json(&mut send, &mut noise, PacketType::TransferHeader, &header)
            .await
            .unwrap();
        tokio::time::timeout(std::time::Duration::from_secs(2), approval_started_rx)
            .await
            .unwrap()
            .unwrap();

        let cancellation_code = TRANSFER_CANCEL_CLOSE_CODE.into();
        send.reset(cancellation_code).unwrap();
        let mut byte = [0u8; 1];
        let acknowledgement =
            tokio::time::timeout(std::time::Duration::from_secs(3), recv.read(&mut byte))
                .await
                .expect("receiver should acknowledge the reset on the reverse stream");
        assert!(
            matches!(
                acknowledgement,
                Err(quinn::ReadError::Reset(code)) if code == cancellation_code
            ),
            "expected cancellation reset acknowledgement, got {acknowledgement:?}"
        );

        let error = tokio::time::timeout(std::time::Duration::from_secs(3), receiver_result_rx)
            .await
            .unwrap()
            .unwrap()
            .unwrap_err();
        assert_eq!(error, "Transfer cancelled by the remote peer");
        assert_eq!(std::fs::read_dir(&destination).unwrap().count(), 0);
        let _ = receiver_release_tx.send(());
        receiver.await.unwrap();
        client.close(0u32.into(), b"test complete");
        std::fs::remove_dir_all(destination).unwrap();
    }

    #[tokio::test]
    async fn remote_cancellation_discards_only_the_incomplete_file() {
        // Mutation captured: publishing before the final chunk leaves a completed-looking truncated file.
        let (error, destination) = interrupt_after_first_chunk(true).await;

        assert!(
            error.contains("Transfer cancelled by the remote peer"),
            "{error}"
        );
        assert!(!error.contains("connection lost"), "{error}");
        assert_eq!(
            tokio::fs::read(destination.join("payload.bin"))
                .await
                .unwrap(),
            b"preserve this existing file"
        );
        let entries: Vec<_> = std::fs::read_dir(&destination)
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect();
        assert_eq!(entries, vec![std::ffi::OsString::from("payload.bin")]);
        tokio::fs::remove_dir_all(destination).await.unwrap();
    }

    #[tokio::test]
    async fn ordinary_connection_loss_is_not_misreported_as_cancellation() {
        // Mutation captured: treating every remote QUIC close as cancellation hides network failures.
        let (error, destination) = interrupt_after_first_chunk(false).await;

        assert!(error.to_lowercase().contains("connection lost"), "{error}");
        assert!(!error.contains("Transfer cancelled"), "{error}");
        assert_eq!(
            tokio::fs::read(destination.join("payload.bin"))
                .await
                .unwrap(),
            b"preserve this existing file"
        );
        assert_eq!(std::fs::read_dir(&destination).unwrap().count(), 1);
        tokio::fs::remove_dir_all(destination).await.unwrap();
    }
}

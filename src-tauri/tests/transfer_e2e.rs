use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use dukto_lib::crypto::noise::{handshake_initiator, handshake_responder};
use dukto_lib::protocol::types::TransferProgress;
use dukto_lib::state::device::DeviceIdentity;
use dukto_lib::transfer::channel::{
    AuthenticatedChannel, ChannelCloseReason, QuinnAuthenticatedChannel, RouteKind,
};
use dukto_lib::transfer::quic::create_endpoint;
use dukto_lib::transfer::receiver::{receive_transfer, receive_transfer_with_accept};
use dukto_lib::transfer::sender::send_transfer;

#[tokio::test]
async fn sender_requires_receiver_receipt() {
    use dukto_lib::crypto::noise::{recv_framed, send_framed};
    use dukto_lib::protocol::{framing::decode_packet_header, types::PacketType};
    let source = temp_dir("no-receipt");
    let file = source.join("payload.txt");
    tokio::fs::write(&file, b"must be acknowledged")
        .await
        .unwrap();
    let server = create_endpoint("127.0.0.1:0".parse().unwrap()).unwrap();
    let address = server.local_addr().unwrap();
    let receiver = tokio::spawn(async move {
        let conn = server.accept().await.unwrap().await.unwrap();
        let (mut send, mut recv) = conn.accept_bi().await.unwrap();
        let mut noise = handshake_responder(&mut send, &mut recv).await.unwrap();
        let header = recv_framed(&mut recv).await.unwrap();
        let mut plaintext = vec![0; 65535];
        noise.read_message(&header, &mut plaintext).unwrap();
        let mut response = vec![PacketType::AcceptReject as u8];
        response.extend_from_slice(br#"{"accepted":true,"destination_dir":null}"#);
        let mut encrypted = vec![0; 65535];
        let len = noise.write_message(&response, &mut encrypted).unwrap();
        send_framed(&mut send, &encrypted[..len]).await.unwrap();
        loop {
            let encrypted = recv_framed(&mut recv).await.unwrap();
            let len = noise.read_message(&encrypted, &mut plaintext).unwrap();
            if decode_packet_header(&plaintext[..len]).unwrap().0 == PacketType::Success {
                break;
            }
        }
        // Emulate the old receiver: it consumes all data but never acknowledges completion.
        conn.close(0u32.into(), b"done");
        server.wait_idle().await;
    });
    let client = create_endpoint("127.0.0.1:0".parse().unwrap()).unwrap();
    let conn = client.connect(address, "localhost").unwrap().await.unwrap();
    let (mut send, mut recv) = conn.open_bi().await.unwrap();
    let mut noise = handshake_initiator(&mut send, &mut recv).await.unwrap();
    let result = tokio::time::timeout(
        std::time::Duration::from_secs(5),
        send_transfer(
            &mut send,
            &mut recv,
            &mut noise,
            &[file],
            "no-receipt",
            &test_identity("sender"),
            |_| {},
        ),
    )
    .await
    .unwrap();
    assert!(
        result.is_err(),
        "queued data without a receiver receipt must not count as success"
    );
    receiver.await.unwrap();
    tokio::fs::remove_dir_all(source).await.unwrap();
}

fn temp_dir(suffix: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("dukto-e2e-{}-{}", suffix, uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn test_identity(device_id: &str) -> DeviceIdentity {
    DeviceIdentity {
        device_id: device_id.into(),
        display_name: "Test Sender".into(),
        hostname: "test-host.local".into(),
        platform: "linux".into(),
    }
}

#[tokio::test]
async fn transfer_header_delivers_sender_identity_over_quic() {
    let source = temp_dir("sender-identity-source");
    let destination = temp_dir("sender-identity-destination");
    let file = source.join("payload.txt");
    tokio::fs::write(&file, b"identity over the transfer header")
        .await
        .unwrap();

    let server = create_endpoint("127.0.0.1:0".parse().unwrap()).unwrap();
    let address = server.local_addr().unwrap();
    let (header_tx, header_rx) = tokio::sync::oneshot::channel();
    let destination_for_server = destination.clone();
    let receiver = tokio::spawn(async move {
        let connection = server.accept().await.unwrap().await.unwrap();
        let (mut send, mut recv) = connection.accept_bi().await.unwrap();
        let mut noise = handshake_responder(&mut send, &mut recv).await.unwrap();
        let result = receive_transfer_with_accept(
            &mut send,
            &mut recv,
            &mut noise,
            &destination_for_server,
            move |header| async move {
                header_tx.send(header).unwrap();
                true
            },
            |_| {},
        )
        .await;
        connection.close(0u32.into(), b"done");
        result
    });

    let client = create_endpoint("127.0.0.1:0".parse().unwrap()).unwrap();
    let connection = client.connect(address, "localhost").unwrap().await.unwrap();
    let (mut send, mut recv) = connection.open_bi().await.unwrap();
    let mut noise = handshake_initiator(&mut send, &mut recv).await.unwrap();
    let sender = test_identity("direct-address-sender");
    send_transfer(
        &mut send,
        &mut recv,
        &mut noise,
        &[file],
        "sender-identity",
        &sender,
        |_| {},
    )
    .await
    .unwrap();

    let header = header_rx.await.unwrap();
    let received_sender = header.sender.expect("sender identity in transfer header");
    assert_eq!(header.sender_device_id, sender.device_id);
    assert_eq!(received_sender.display_name, sender.display_name);
    assert_eq!(received_sender.hostname, sender.hostname);
    assert_eq!(received_sender.platform, sender.platform);
    receiver.await.unwrap().unwrap();

    connection.close(0u32.into(), b"done");
    client.close(0u32.into(), b"shutdown");
    tokio::fs::remove_dir_all(source).await.unwrap();
    tokio::fs::remove_dir_all(destination).await.unwrap();
}

/// Helper: run a transfer between two endpoints.
async fn run_transfer(
    inputs: &[PathBuf],
    dest_dir: &Path,
    transfer_id: &str,
    auto_accept: bool,
) -> Result<
    (u64, dukto_lib::transfer::receiver::ReceiveResult),
    Box<dyn std::error::Error + Send + Sync>,
> {
    let server_addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
    let server_ep = create_endpoint(server_addr).expect("server ep");
    let server_addr = server_ep.local_addr().expect("server addr");
    let client_ep = create_endpoint("127.0.0.1:0".parse().unwrap()).expect("client ep");

    let dest = dest_dir.to_path_buf();
    let server_handle = tokio::spawn(async move {
        let incoming = server_ep.accept().await.unwrap();
        let conn = incoming.await.unwrap();
        let channel = QuinnAuthenticatedChannel::new_direct(conn, "device-sender").unwrap();
        assert_eq!(channel.peer_id(), "device-sender");
        assert_eq!(channel.route_kind(), RouteKind::Direct);
        let (mut send, mut recv) = channel.accept_bi().await.unwrap();
        let mut noise = handshake_responder(&mut send, &mut recv).await.unwrap();
        let result = receive_transfer(&mut send, &mut recv, &mut noise, &dest, auto_accept).await;
        channel.close(ChannelCloseReason::Completed);
        server_ep.close(0u32.into(), b"shutdown");
        result
    });

    let conn = client_ep
        .connect(server_addr, "localhost")
        .unwrap()
        .await
        .unwrap();
    let channel = QuinnAuthenticatedChannel::new_direct(conn, "test-receiver").unwrap();
    assert_eq!(channel.peer_id(), "test-receiver");
    assert_eq!(channel.route_kind(), RouteKind::Direct);
    let (mut send, mut recv) = channel.open_bi().await.unwrap();
    let mut noise = handshake_initiator(&mut send, &mut recv).await.unwrap();

    let bytes_sent = send_transfer(
        &mut send,
        &mut recv,
        &mut noise,
        inputs,
        transfer_id,
        &test_identity("device-sender"),
        |_| {},
    )
    .await;

    let recv_result = server_handle.await.unwrap();

    channel.close(ChannelCloseReason::Completed);
    client_ep.close(0u32.into(), b"shutdown");

    match (bytes_sent, recv_result) {
        (Ok(sent), Ok(received)) => Ok((sent, received)),
        (Err(e), _) => Err(e),
        (_, Err(e)) => Err(e),
    }
}

#[tokio::test]
async fn single_file_transfer_end_to_end() {
    let src_dir = temp_dir("src");
    let test_file = src_dir.join("hello.txt");
    let test_content = "Hello from Dukto! This is a test file for transfer.";
    tokio::fs::write(&test_file, test_content).await.unwrap();

    let dest_dir = temp_dir("dest");

    let (bytes_sent, result) = run_transfer(&[test_file], &dest_dir, "t-001", true)
        .await
        .unwrap();

    assert_eq!(bytes_sent, test_content.len() as u64);
    assert_eq!(result.items_received, 1);
    assert_eq!(result.bytes_received, test_content.len() as u64);

    let received = tokio::fs::read_to_string(dest_dir.join("hello.txt"))
        .await
        .unwrap();
    assert_eq!(received, test_content);

    tokio::fs::remove_dir_all(&src_dir).await.ok();
    tokio::fs::remove_dir_all(&dest_dir).await.ok();
}

#[tokio::test]
async fn transfer_rejected_by_receiver() {
    let src_dir = temp_dir("reject-src");
    let test_file = src_dir.join("secret.txt");
    tokio::fs::write(&test_file, "secret data").await.unwrap();

    let dest_dir = temp_dir("reject-dest");

    let result = run_transfer(&[test_file], &dest_dir, "t-reject", false).await;
    assert!(result.is_err());
    assert!(!dest_dir.join("secret.txt").exists());

    tokio::fs::remove_dir_all(&src_dir).await.ok();
    tokio::fs::remove_dir_all(&dest_dir).await.ok();
}

#[tokio::test]
async fn large_file_transfer_multiple_chunks() {
    let src_dir = temp_dir("large-src");
    let test_file = src_dir.join("bigfile.bin");
    let file_size: usize = 256 * 1024;
    let test_data: Vec<u8> = (0..file_size).map(|i| (i % 251) as u8).collect();
    tokio::fs::write(&test_file, &test_data).await.unwrap();

    let dest_dir = temp_dir("large-dest");

    let (bytes_sent, result) = run_transfer(&[test_file], &dest_dir, "t-large", true)
        .await
        .unwrap();

    assert_eq!(bytes_sent, file_size as u64);
    assert_eq!(result.bytes_received, file_size as u64);

    let received = tokio::fs::read(dest_dir.join("bigfile.bin")).await.unwrap();
    assert_eq!(received, test_data);

    tokio::fs::remove_dir_all(&src_dir).await.ok();
    tokio::fs::remove_dir_all(&dest_dir).await.ok();
}

#[tokio::test]
async fn progress_callback_reports_correct_values() {
    let src_dir = temp_dir("progress-src");
    let test_file = src_dir.join("progress.bin");
    let file_size: usize = 64 * 1024 * 1024;
    let test_data: Vec<u8> = (0..file_size).map(|i| (i % 199) as u8).collect();
    tokio::fs::write(&test_file, &test_data).await.unwrap();
    let empty_file = src_dir.join("empty.txt");
    tokio::fs::write(&empty_file, b"").await.unwrap();
    let folder = src_dir.join("folder");
    tokio::fs::create_dir_all(folder.join("nested/empty"))
        .await
        .unwrap();
    tokio::fs::write(folder.join("first.bin"), vec![17; 32768])
        .await
        .unwrap();
    tokio::fs::write(folder.join("nested/second.bin"), vec![23; 65536])
        .await
        .unwrap();
    let empty_folder = src_dir.join("empty-folder");
    tokio::fs::create_dir_all(&empty_folder).await.unwrap();
    tokio::fs::create_dir_all(src_dir.join("left"))
        .await
        .unwrap();
    tokio::fs::create_dir_all(src_dir.join("right"))
        .await
        .unwrap();
    let same_name_a = src_dir.join("left/shared.txt");
    let same_name_b = src_dir.join("right/shared.txt");
    tokio::fs::write(&same_name_a, b"first").await.unwrap();
    tokio::fs::write(&same_name_b, b"another").await.unwrap();
    let total_size = file_size as u64 + 32768 + 65536 + 5 + 7;

    let dest_dir = temp_dir("progress-dest");

    let server_addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
    let server_ep = create_endpoint(server_addr).unwrap();
    let server_addr = server_ep.local_addr().unwrap();
    let client_ep = create_endpoint("127.0.0.1:0".parse().unwrap()).unwrap();

    let dest = dest_dir.clone();
    let server_handle = tokio::spawn(async move {
        let incoming = server_ep.accept().await.unwrap();
        let conn = incoming.await.unwrap();
        let (mut send, mut recv) = conn.accept_bi().await.unwrap();
        let mut noise = handshake_responder(&mut send, &mut recv).await.unwrap();
        let mut received_progress = Vec::new();
        dukto_lib::transfer::receiver::receive_transfer_with_accept(
            &mut send,
            &mut recv,
            &mut noise,
            &dest,
            |_| async { true },
            |progress| received_progress.push(progress.clone()),
        )
        .await
        .unwrap();
        conn.close(0u32.into(), b"done");
        server_ep.close(0u32.into(), b"shutdown");
        received_progress
    });

    let conn = client_ep
        .connect(server_addr, "localhost")
        .unwrap()
        .await
        .unwrap();
    let (mut send, mut recv) = conn.open_bi().await.unwrap();
    let mut noise = handshake_initiator(&mut send, &mut recv).await.unwrap();

    let snapshots: Arc<Mutex<Vec<TransferProgress>>> = Arc::new(Mutex::new(Vec::new()));
    let snaps = snapshots.clone();

    send_transfer(
        &mut send,
        &mut recv,
        &mut noise,
        &[
            test_file,
            folder,
            empty_file,
            empty_folder,
            same_name_a,
            same_name_b,
        ],
        "t-progress",
        &test_identity("device-sender"),
        move |p| {
            snaps.lock().unwrap().push(p.clone());
        },
    )
    .await
    .unwrap();

    let received_progress = server_handle.await.unwrap();
    conn.close(0u32.into(), b"done");
    client_ep.close(0u32.into(), b"shutdown");

    {
        let snaps = snapshots.lock().unwrap();
        let expected = [
            ("progress.bin", file_size as u64, 1),
            ("folder", 98304, 3),
            ("empty.txt", 0, 1),
            ("empty-folder", 0, 1),
            ("shared.txt", 5, 1),
            ("shared.txt", 7, 1),
        ];
        for progress in [&*snaps, &received_progress] {
            let manifest = progress.first().unwrap().items.as_ref().unwrap();
            assert_eq!(manifest.len(), expected.len());
            for (item, (name, size, count)) in manifest.iter().zip(expected) {
                assert_eq!(
                    (item.name.as_str(), item.size_bytes, item.item_count),
                    (name, size, count)
                );
                let events: Vec<_> = progress
                    .iter()
                    .filter_map(|event| event.item_progress.as_ref())
                    .filter(|event| event.item_id == item.item_id)
                    .collect();
                assert_eq!(events.first().unwrap().bytes_sent, 0);
                assert!(!events.first().unwrap().completed);
                assert_eq!(events.last().unwrap().bytes_sent, size);
                assert!(events.last().unwrap().completed);
                assert_eq!(events.iter().filter(|event| event.completed).count(), 1);
                assert!(events
                    .iter()
                    .all(|event| event.bytes_total == size && event.bytes_sent <= size));
                assert!(events
                    .windows(2)
                    .all(|pair| pair[0].bytes_sent <= pair[1].bytes_sent));
            }
            assert_ne!(manifest[4].item_id, manifest[5].item_id);
            assert!(progress.iter().all(|event| event.bytes_total == total_size));
            assert_eq!(progress.last().unwrap().bytes_sent, total_size);
            assert_eq!(progress.last().unwrap().percent, 100.0);
        }
        let sent_ids: Vec<_> = snaps[0]
            .items
            .as_ref()
            .unwrap()
            .iter()
            .map(|item| &item.item_id)
            .collect();
        let received_ids: Vec<_> = received_progress[0]
            .items
            .as_ref()
            .unwrap()
            .iter()
            .map(|item| &item.item_id)
            .collect();
        assert_eq!(sent_ids, received_ids);
    }

    assert_eq!(
        tokio::fs::read(dest_dir.join("progress.bin"))
            .await
            .unwrap(),
        test_data
    );
    assert_eq!(
        tokio::fs::read(dest_dir.join("folder/first.bin"))
            .await
            .unwrap(),
        vec![17; 32768]
    );
    assert_eq!(
        tokio::fs::read(dest_dir.join("folder/nested/second.bin"))
            .await
            .unwrap(),
        vec![23; 65536]
    );
    assert!(dest_dir.join("folder/nested/empty").is_dir());
    assert!(dest_dir.join("empty-folder").is_dir());
    assert_eq!(
        tokio::fs::read(dest_dir.join("shared.txt")).await.unwrap(),
        b"first"
    );
    assert_eq!(
        tokio::fs::read(dest_dir.join("shared (1).txt"))
            .await
            .unwrap(),
        b"another"
    );

    tokio::fs::remove_dir_all(&src_dir).await.ok();
    tokio::fs::remove_dir_all(&dest_dir).await.ok();
}

// --- Story 6: Multi-file and folder tests ---

#[tokio::test]
async fn multi_file_transfer() {
    let src_dir = temp_dir("multi-src");
    tokio::fs::write(src_dir.join("a.txt"), "file-a")
        .await
        .unwrap();
    tokio::fs::write(src_dir.join("b.txt"), "file-b-longer")
        .await
        .unwrap();

    let dest_dir = temp_dir("multi-dest");

    let inputs = vec![src_dir.join("a.txt"), src_dir.join("b.txt")];
    let (bytes_sent, result) = run_transfer(&inputs, &dest_dir, "t-multi", true)
        .await
        .unwrap();

    assert_eq!(result.items_received, 2);
    assert_eq!(bytes_sent, 6 + 13); // "file-a" + "file-b-longer"
    assert_eq!(result.bytes_received, 6 + 13);

    assert_eq!(
        tokio::fs::read_to_string(dest_dir.join("a.txt"))
            .await
            .unwrap(),
        "file-a"
    );
    assert_eq!(
        tokio::fs::read_to_string(dest_dir.join("b.txt"))
            .await
            .unwrap(),
        "file-b-longer"
    );

    tokio::fs::remove_dir_all(&src_dir).await.ok();
    tokio::fs::remove_dir_all(&dest_dir).await.ok();
}

#[tokio::test]
async fn folder_transfer_preserves_structure() {
    let src_dir = temp_dir("folder-src");
    let folder = src_dir.join("project");
    tokio::fs::create_dir_all(folder.join("sub")).await.unwrap();
    tokio::fs::create_dir_all(folder.join("empty_dir"))
        .await
        .unwrap();
    tokio::fs::write(folder.join("root.txt"), "root content")
        .await
        .unwrap();
    tokio::fs::write(folder.join("sub/nested.txt"), "nested content")
        .await
        .unwrap();

    let dest_dir = temp_dir("folder-dest");

    let (bytes_sent, result) = run_transfer(&[folder], &dest_dir, "t-folder", true)
        .await
        .unwrap();

    // 2 files + 1 empty dir = 3 items
    assert_eq!(result.items_received, 3);
    assert_eq!(bytes_sent, 12 + 14); // "root content" + "nested content"

    assert_eq!(
        tokio::fs::read_to_string(dest_dir.join("project/root.txt"))
            .await
            .unwrap(),
        "root content"
    );
    assert_eq!(
        tokio::fs::read_to_string(dest_dir.join("project/sub/nested.txt"))
            .await
            .unwrap(),
        "nested content"
    );
    assert!(dest_dir.join("project/empty_dir").is_dir());

    tokio::fs::remove_dir_all(&src_dir).await.ok();
    tokio::fs::remove_dir_all(&dest_dir).await.ok();
}

#[tokio::test]
async fn conflict_resolution_renames_duplicate() {
    let src_dir = temp_dir("conflict-src");
    tokio::fs::write(src_dir.join("file.txt"), "new content")
        .await
        .unwrap();

    let dest_dir = temp_dir("conflict-dest");
    // Pre-create a conflicting file
    tokio::fs::write(dest_dir.join("file.txt"), "existing content")
        .await
        .unwrap();

    let (_, result) = run_transfer(&[src_dir.join("file.txt")], &dest_dir, "t-conflict", true)
        .await
        .unwrap();

    assert_eq!(result.items_received, 1);

    // Original should be untouched
    assert_eq!(
        tokio::fs::read_to_string(dest_dir.join("file.txt"))
            .await
            .unwrap(),
        "existing content"
    );
    // New file should be renamed
    assert_eq!(
        tokio::fs::read_to_string(dest_dir.join("file (1).txt"))
            .await
            .unwrap(),
        "new content"
    );

    tokio::fs::remove_dir_all(&src_dir).await.ok();
    tokio::fs::remove_dir_all(&dest_dir).await.ok();
}

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use dukto_lib::crypto::noise::{handshake_initiator, handshake_responder};
use dukto_lib::protocol::types::TransferProgress;
use dukto_lib::transfer::quic::create_endpoint;
use dukto_lib::transfer::receiver::receive_transfer;
use dukto_lib::transfer::sender::{send_file, send_transfer};

fn temp_dir(suffix: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("dukto-e2e-{}-{}", suffix, uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Helper: run a transfer between two endpoints.
async fn run_transfer(
    inputs: &[PathBuf],
    dest_dir: &PathBuf,
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

    let dest = dest_dir.clone();
    let server_handle = tokio::spawn(async move {
        let incoming = server_ep.accept().await.unwrap();
        let conn = incoming.await.unwrap();
        let (mut send, mut recv) = conn.accept_bi().await.unwrap();
        let mut noise = handshake_responder(&mut send, &mut recv).await.unwrap();
        let result = receive_transfer(&mut send, &mut recv, &mut noise, &dest, auto_accept).await;
        conn.close(0u32.into(), b"done");
        server_ep.close(0u32.into(), b"shutdown");
        result
    });

    let conn = client_ep.connect(server_addr, "localhost").unwrap().await.unwrap();
    let (mut send, mut recv) = conn.open_bi().await.unwrap();
    let mut noise = handshake_initiator(&mut send, &mut recv).await.unwrap();

    let bytes_sent = send_transfer(
        &mut send,
        &mut recv,
        &mut noise,
        inputs,
        transfer_id,
        "device-sender",
        |_| {},
    )
    .await;

    let recv_result = server_handle.await.unwrap();

    conn.close(0u32.into(), b"done");
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

    let (bytes_sent, result) =
        run_transfer(&[test_file], &dest_dir, "t-001", true).await.unwrap();

    assert_eq!(bytes_sent, test_content.len() as u64);
    assert_eq!(result.items_received, 1);
    assert_eq!(result.bytes_received, test_content.len() as u64);

    let received = tokio::fs::read_to_string(dest_dir.join("hello.txt")).await.unwrap();
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

    let (bytes_sent, result) =
        run_transfer(&[test_file], &dest_dir, "t-large", true).await.unwrap();

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
    let file_size: usize = 128 * 1024;
    let test_data: Vec<u8> = (0..file_size).map(|i| (i % 199) as u8).collect();
    tokio::fs::write(&test_file, &test_data).await.unwrap();

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
        receive_transfer(&mut send, &mut recv, &mut noise, &dest, true)
            .await
            .unwrap();
        conn.close(0u32.into(), b"done");
        server_ep.close(0u32.into(), b"shutdown");
    });

    let conn = client_ep.connect(server_addr, "localhost").unwrap().await.unwrap();
    let (mut send, mut recv) = conn.open_bi().await.unwrap();
    let mut noise = handshake_initiator(&mut send, &mut recv).await.unwrap();

    let snapshots: Arc<Mutex<Vec<TransferProgress>>> = Arc::new(Mutex::new(Vec::new()));
    let snaps = snapshots.clone();

    send_file(
        &mut send, &mut recv, &mut noise, &test_file,
        "t-progress", "device-sender",
        move |p| { snaps.lock().unwrap().push(p.clone()); },
    )
    .await
    .unwrap();

    server_handle.await.unwrap();
    conn.close(0u32.into(), b"done");
    client_ep.close(0u32.into(), b"shutdown");

    let snaps = snapshots.lock().unwrap();
    assert_eq!(snaps.len(), 4, "Should have 4 progress callbacks for 128KB file");
    for (i, snap) in snaps.iter().enumerate() {
        assert_eq!(snap.bytes_total, file_size as u64);
        assert_eq!(snap.bytes_sent, ((i + 1) as u64) * 32 * 1024);
    }
    assert!((snaps.last().unwrap().percent - 100.0).abs() < 0.01);

    tokio::fs::remove_dir_all(&src_dir).await.ok();
    tokio::fs::remove_dir_all(&dest_dir).await.ok();
}

// --- Story 6: Multi-file and folder tests ---

#[tokio::test]
async fn multi_file_transfer() {
    let src_dir = temp_dir("multi-src");
    tokio::fs::write(src_dir.join("a.txt"), "file-a").await.unwrap();
    tokio::fs::write(src_dir.join("b.txt"), "file-b-longer").await.unwrap();

    let dest_dir = temp_dir("multi-dest");

    let inputs = vec![src_dir.join("a.txt"), src_dir.join("b.txt")];
    let (bytes_sent, result) =
        run_transfer(&inputs, &dest_dir, "t-multi", true).await.unwrap();

    assert_eq!(result.items_received, 2);
    assert_eq!(bytes_sent, 6 + 13); // "file-a" + "file-b-longer"
    assert_eq!(result.bytes_received, 6 + 13);

    assert_eq!(
        tokio::fs::read_to_string(dest_dir.join("a.txt")).await.unwrap(),
        "file-a"
    );
    assert_eq!(
        tokio::fs::read_to_string(dest_dir.join("b.txt")).await.unwrap(),
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
    tokio::fs::create_dir_all(folder.join("empty_dir")).await.unwrap();
    tokio::fs::write(folder.join("root.txt"), "root content").await.unwrap();
    tokio::fs::write(folder.join("sub/nested.txt"), "nested content").await.unwrap();

    let dest_dir = temp_dir("folder-dest");

    let (bytes_sent, result) =
        run_transfer(&[folder], &dest_dir, "t-folder", true).await.unwrap();

    // 2 files + 1 empty dir = 3 items
    assert_eq!(result.items_received, 3);
    assert_eq!(bytes_sent, 12 + 14); // "root content" + "nested content"

    assert_eq!(
        tokio::fs::read_to_string(dest_dir.join("project/root.txt")).await.unwrap(),
        "root content"
    );
    assert_eq!(
        tokio::fs::read_to_string(dest_dir.join("project/sub/nested.txt")).await.unwrap(),
        "nested content"
    );
    assert!(dest_dir.join("project/empty_dir").is_dir());

    tokio::fs::remove_dir_all(&src_dir).await.ok();
    tokio::fs::remove_dir_all(&dest_dir).await.ok();
}

#[tokio::test]
async fn conflict_resolution_renames_duplicate() {
    let src_dir = temp_dir("conflict-src");
    tokio::fs::write(src_dir.join("file.txt"), "new content").await.unwrap();

    let dest_dir = temp_dir("conflict-dest");
    // Pre-create a conflicting file
    tokio::fs::write(dest_dir.join("file.txt"), "existing content").await.unwrap();

    let (_, result) =
        run_transfer(&[src_dir.join("file.txt")], &dest_dir, "t-conflict", true)
            .await
            .unwrap();

    assert_eq!(result.items_received, 1);

    // Original should be untouched
    assert_eq!(
        tokio::fs::read_to_string(dest_dir.join("file.txt")).await.unwrap(),
        "existing content"
    );
    // New file should be renamed
    assert_eq!(
        tokio::fs::read_to_string(dest_dir.join("file (1).txt")).await.unwrap(),
        "new content"
    );

    tokio::fs::remove_dir_all(&src_dir).await.ok();
    tokio::fs::remove_dir_all(&dest_dir).await.ok();
}

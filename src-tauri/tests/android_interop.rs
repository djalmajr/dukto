use std::path::PathBuf;

use dukto_lib::crypto::noise::{handshake_initiator, handshake_responder};
use dukto_lib::state::device::DeviceIdentity;
use dukto_lib::transfer::quic::create_endpoint;
use dukto_lib::transfer::receiver::receive_transfer;
use dukto_lib::transfer::sender::send_transfer;

fn temp_dir(suffix: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("dukto-android-test-{}-{}", suffix, uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn macos_identity() -> DeviceIdentity {
    DeviceIdentity {
        device_id: "macos-device-id-12345".into(),
        display_name: "MacBook Pro".into(),
        hostname: "macbook.local".into(),
        platform: "macos".into(),
    }
}

fn android_identity() -> DeviceIdentity {
    DeviceIdentity {
        device_id: "android-device-id-54321".into(),
        display_name: "Pixel 9 Pro".into(),
        hostname: "pixel.local".into(),
        platform: "android".into(),
    }
}

/// Test Direction 1: macOS -> Android transfer over QUIC + Noise.
#[tokio::test]
async fn transfer_macos_to_android_bidirectional() {
    let mac_source = temp_dir("mac-source");
    let android_downloads = temp_dir("android-downloads");

    let test_file = mac_source.join("document.pdf");
    let payload = b"%PDF-1.4 Fake PDF Content sent from macOS to Android";
    tokio::fs::write(&test_file, payload).await.unwrap();

    // Android node listening on ephemeral port (simulating dynamic bind)
    let android_endpoint = create_endpoint("127.0.0.1:0".parse().unwrap()).unwrap();
    let android_addr = android_endpoint.local_addr().unwrap();

    let android_dest_clone = android_downloads.clone();
    let android_receiver_task = tokio::spawn(async move {
        let incoming = android_endpoint.accept().await.unwrap();
        let conn = incoming.await.unwrap();
        let (mut send, mut recv) = conn.accept_bi().await.unwrap();
        let mut noise = handshake_responder(&mut send, &mut recv).await.unwrap();

        let result = receive_transfer(&mut send, &mut recv, &mut noise, &android_dest_clone, true).await;
        conn.close(0u32.into(), b"done");
        android_endpoint.close(0u32.into(), b"shutdown");
        result
    });

    // macOS node connecting to Android
    let mac_endpoint = create_endpoint("127.0.0.1:0".parse().unwrap()).unwrap();
    let mac_conn = mac_endpoint.connect(android_addr, "localhost").unwrap().await.unwrap();
    let (mut mac_send, mut mac_recv) = mac_conn.open_bi().await.unwrap();
    let mut mac_noise = handshake_initiator(&mut mac_send, &mut mac_recv).await.unwrap();

    let mac_sender = macos_identity();
    let bytes_sent = send_transfer(
        &mut mac_send,
        &mut mac_recv,
        &mut mac_noise,
        &[test_file.clone()],
        "xfer-mac-to-android",
        &mac_sender,
        |_| {},
    )
    .await
    .unwrap();

    assert_eq!(bytes_sent, payload.len() as u64);

    let recv_result = android_receiver_task.await.unwrap().unwrap();
    assert_eq!(recv_result.items_received, 1);
    assert_eq!(recv_result.bytes_received, payload.len() as u64);

    // Verify file exists in Android download dir and content matches
    let received_file = android_downloads.join("document.pdf");
    assert!(received_file.exists());
    let received_content = tokio::fs::read(&received_file).await.unwrap();
    assert_eq!(received_content, payload);

    mac_conn.close(0u32.into(), b"done");
    mac_endpoint.close(0u32.into(), b"shutdown");
    tokio::fs::remove_dir_all(mac_source).await.ok();
    tokio::fs::remove_dir_all(android_downloads).await.ok();
}

/// Test Direction 2: Android -> macOS transfer over QUIC + Noise.
#[tokio::test]
async fn transfer_android_to_macos_bidirectional() {
    let android_source = temp_dir("android-source");
    let mac_downloads = temp_dir("mac-downloads");

    let test_file = android_source.join("capture.mp4");
    let payload = b"ftypisom Fake MP4 Video bytes sent from Android to macOS";
    tokio::fs::write(&test_file, payload).await.unwrap();

    // macOS node listening
    let mac_endpoint = create_endpoint("127.0.0.1:0".parse().unwrap()).unwrap();
    let mac_addr = mac_endpoint.local_addr().unwrap();

    let mac_dest_clone = mac_downloads.clone();
    let mac_receiver_task = tokio::spawn(async move {
        let incoming = mac_endpoint.accept().await.unwrap();
        let conn = incoming.await.unwrap();
        let (mut send, mut recv) = conn.accept_bi().await.unwrap();
        let mut noise = handshake_responder(&mut send, &mut recv).await.unwrap();

        let result = receive_transfer(&mut send, &mut recv, &mut noise, &mac_dest_clone, true).await;
        conn.close(0u32.into(), b"done");
        mac_endpoint.close(0u32.into(), b"shutdown");
        result
    });

    // Android node connecting to macOS
    let android_endpoint = create_endpoint("127.0.0.1:0".parse().unwrap()).unwrap();
    let android_conn = android_endpoint.connect(mac_addr, "localhost").unwrap().await.unwrap();
    let (mut android_send, mut android_recv) = android_conn.open_bi().await.unwrap();
    let mut android_noise = handshake_initiator(&mut android_send, &mut android_recv).await.unwrap();

    let android_sender = android_identity();
    let bytes_sent = send_transfer(
        &mut android_send,
        &mut android_recv,
        &mut android_noise,
        &[test_file.clone()],
        "xfer-android-to-mac",
        &android_sender,
        |_| {},
    )
    .await
    .unwrap();

    assert_eq!(bytes_sent, payload.len() as u64);

    let recv_result = mac_receiver_task.await.unwrap().unwrap();
    assert_eq!(recv_result.items_received, 1);
    assert_eq!(recv_result.bytes_received, payload.len() as u64);

    let received_file = mac_downloads.join("capture.mp4");
    assert!(received_file.exists());
    let received_content = tokio::fs::read(&received_file).await.unwrap();
    assert_eq!(received_content, payload);

    android_conn.close(0u32.into(), b"done");
    android_endpoint.close(0u32.into(), b"shutdown");
    tokio::fs::remove_dir_all(android_source).await.ok();
    tokio::fs::remove_dir_all(mac_downloads).await.ok();
}

/// Test Direction 3: Multi-file directory tree transfer between Android and macOS.
#[tokio::test]
async fn multi_file_folder_transfer_between_android_and_macos() {
    let android_source = temp_dir("android-folder-source");
    let mac_destination = temp_dir("mac-folder-dest");

    // Create a folder hierarchy:
    // folder/
    // ├── file1.txt
    // ├── subfolder/
    // │   └── file2.bin
    // └── empty_dir/
    let transfer_dir = android_source.join("Projects");
    let sub_dir = transfer_dir.join("subdir");
    let empty_dir = transfer_dir.join("empty_folder");
    tokio::fs::create_dir_all(&sub_dir).await.unwrap();
    tokio::fs::create_dir_all(&empty_dir).await.unwrap();

    let file1 = transfer_dir.join("notes.txt");
    let file2 = sub_dir.join("data.bin");
    tokio::fs::write(&file1, b"Android notes line 1\nline 2").await.unwrap();
    tokio::fs::write(&file2, &[0xDE, 0xAD, 0xBE, 0xEF, 0x01, 0x02, 0x03]).await.unwrap();

    // macOS receiver
    let mac_endpoint = create_endpoint("127.0.0.1:0".parse().unwrap()).unwrap();
    let mac_addr = mac_endpoint.local_addr().unwrap();

    let mac_dest_clone = mac_destination.clone();
    let mac_receiver_task = tokio::spawn(async move {
        let incoming = mac_endpoint.accept().await.unwrap();
        let conn = incoming.await.unwrap();
        let (mut send, mut recv) = conn.accept_bi().await.unwrap();
        let mut noise = handshake_responder(&mut send, &mut recv).await.unwrap();

        let result = receive_transfer(&mut send, &mut recv, &mut noise, &mac_dest_clone, true).await;
        conn.close(0u32.into(), b"done");
        mac_endpoint.close(0u32.into(), b"shutdown");
        result
    });

    // Android sender
    let android_endpoint = create_endpoint("127.0.0.1:0".parse().unwrap()).unwrap();
    let android_conn = android_endpoint.connect(mac_addr, "localhost").unwrap().await.unwrap();
    let (mut android_send, mut android_recv) = android_conn.open_bi().await.unwrap();
    let mut android_noise = handshake_initiator(&mut android_send, &mut android_recv).await.unwrap();

    let android_sender = android_identity();
    let total_bytes = send_transfer(
        &mut android_send,
        &mut android_recv,
        &mut android_noise,
        &[transfer_dir.clone()],
        "xfer-folder-android-to-mac",
        &android_sender,
        |_| {},
    )
    .await
    .unwrap();

    let recv_result = mac_receiver_task.await.unwrap().unwrap();
    assert_eq!(recv_result.bytes_received, total_bytes);

    // Verify directory structure on macOS
    let received_projects = mac_destination.join("Projects");
    assert!(received_projects.is_dir());
    assert!(received_projects.join("notes.txt").is_file());
    assert!(received_projects.join("subdir").join("data.bin").is_file());
    assert!(received_projects.join("empty_folder").is_dir());

    assert_eq!(
        tokio::fs::read(received_projects.join("notes.txt")).await.unwrap(),
        b"Android notes line 1\nline 2"
    );
    assert_eq!(
        tokio::fs::read(received_projects.join("subdir").join("data.bin")).await.unwrap(),
        &[0xDE, 0xAD, 0xBE, 0xEF, 0x01, 0x02, 0x03]
    );

    android_conn.close(0u32.into(), b"done");
    android_endpoint.close(0u32.into(), b"shutdown");
    tokio::fs::remove_dir_all(android_source).await.ok();
    tokio::fs::remove_dir_all(mac_destination).await.ok();
}

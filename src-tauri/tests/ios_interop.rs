use std::path::PathBuf;

use dukto_lib::crypto::noise::{handshake_initiator, handshake_responder};
use dukto_lib::state::device::DeviceIdentity;
use dukto_lib::transfer::quic::create_endpoint;
use dukto_lib::transfer::receiver::receive_transfer;
use dukto_lib::transfer::sender::send_transfer;

fn temp_dir(suffix: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "dukto-ios-test-{}-{}",
        suffix,
        uuid::Uuid::new_v4()
    ));
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

fn ios_identity() -> DeviceIdentity {
    DeviceIdentity {
        device_id: "ios-device-id-67890".into(),
        display_name: "iPhone 16 Pro".into(),
        hostname: "iphone.local".into(),
        platform: "ios".into(),
    }
}

/// Test Direction 1: macOS -> iOS transfer over QUIC + Noise.
#[tokio::test]
async fn transfer_macos_to_ios_bidirectional() {
    let mac_source = temp_dir("mac-source");
    let ios_documents = temp_dir("ios-documents");

    let test_file = mac_source.join("photo.jpg");
    let payload = b"GIF89a Fake Image Payload sent from macOS to iOS";
    tokio::fs::write(&test_file, payload).await.unwrap();

    // iOS node listening on ephemeral port (simulating dynamic bind)
    let ios_endpoint = create_endpoint("127.0.0.1:0".parse().unwrap()).unwrap();
    let ios_addr = ios_endpoint.local_addr().unwrap();

    let ios_dest_clone = ios_documents.clone();
    let ios_receiver_task = tokio::spawn(async move {
        let incoming = ios_endpoint.accept().await.unwrap();
        let conn = incoming.await.unwrap();
        let (mut send, mut recv) = conn.accept_bi().await.unwrap();
        let mut noise = handshake_responder(&mut send, &mut recv).await.unwrap();

        let result =
            receive_transfer(&mut send, &mut recv, &mut noise, &ios_dest_clone, true).await;
        conn.close(0u32.into(), b"done");
        ios_endpoint.close(0u32.into(), b"shutdown");
        result
    });

    // macOS node connecting to iOS
    let mac_endpoint = create_endpoint("127.0.0.1:0".parse().unwrap()).unwrap();
    let mac_conn = mac_endpoint
        .connect(ios_addr, "localhost")
        .unwrap()
        .await
        .unwrap();
    let (mut mac_send, mut mac_recv) = mac_conn.open_bi().await.unwrap();
    let mut mac_noise = handshake_initiator(&mut mac_send, &mut mac_recv)
        .await
        .unwrap();

    let mac_sender = macos_identity();
    let bytes_sent = send_transfer(
        &mut mac_send,
        &mut mac_recv,
        &mut mac_noise,
        std::slice::from_ref(&test_file),
        "xfer-mac-to-ios",
        &mac_sender,
        |_| {},
    )
    .await
    .unwrap();

    assert_eq!(bytes_sent, payload.len() as u64);

    let recv_result = ios_receiver_task.await.unwrap().unwrap();
    assert_eq!(recv_result.items_received, 1);
    assert_eq!(recv_result.bytes_received, payload.len() as u64);

    // Verify file exists in iOS sandboxed documents dir and content matches
    let received_file = ios_documents.join("photo.jpg");
    assert!(received_file.exists());
    let received_content = tokio::fs::read(&received_file).await.unwrap();
    assert_eq!(received_content, payload);

    mac_conn.close(0u32.into(), b"done");
    mac_endpoint.close(0u32.into(), b"shutdown");
    tokio::fs::remove_dir_all(mac_source).await.ok();
    tokio::fs::remove_dir_all(ios_documents).await.ok();
}

/// Test Direction 2: iOS -> macOS transfer over QUIC + Noise.
#[tokio::test]
async fn transfer_ios_to_macos_bidirectional() {
    let ios_source = temp_dir("ios-source");
    let mac_downloads = temp_dir("mac-downloads");

    let test_file = ios_source.join("document.pdf");
    let payload = b"%PDF-1.4 Fake PDF Payload sent from iOS to macOS";
    tokio::fs::write(&test_file, payload).await.unwrap();

    // macOS node listening on port 4242 or ephemeral port
    let mac_endpoint = create_endpoint("127.0.0.1:0".parse().unwrap()).unwrap();
    let mac_addr = mac_endpoint.local_addr().unwrap();

    let mac_dest_clone = mac_downloads.clone();
    let mac_receiver_task = tokio::spawn(async move {
        let incoming = mac_endpoint.accept().await.unwrap();
        let conn = incoming.await.unwrap();
        let (mut send, mut recv) = conn.accept_bi().await.unwrap();
        let mut noise = handshake_responder(&mut send, &mut recv).await.unwrap();

        let result =
            receive_transfer(&mut send, &mut recv, &mut noise, &mac_dest_clone, true).await;
        conn.close(0u32.into(), b"done");
        mac_endpoint.close(0u32.into(), b"shutdown");
        result
    });

    // iOS node connecting to macOS
    let ios_endpoint = create_endpoint("127.0.0.1:0".parse().unwrap()).unwrap();
    let ios_conn = ios_endpoint
        .connect(mac_addr, "localhost")
        .unwrap()
        .await
        .unwrap();
    let (mut ios_send, mut ios_recv) = ios_conn.open_bi().await.unwrap();
    let mut ios_noise = handshake_initiator(&mut ios_send, &mut ios_recv)
        .await
        .unwrap();

    let ios_sender = ios_identity();
    let bytes_sent = send_transfer(
        &mut ios_send,
        &mut ios_recv,
        &mut ios_noise,
        std::slice::from_ref(&test_file),
        "xfer-ios-to-mac",
        &ios_sender,
        |_| {},
    )
    .await
    .unwrap();

    assert_eq!(bytes_sent, payload.len() as u64);

    let recv_result = mac_receiver_task.await.unwrap().unwrap();
    assert_eq!(recv_result.items_received, 1);
    assert_eq!(recv_result.bytes_received, payload.len() as u64);

    // Verify file exists in macOS downloads dir and content matches
    let received_file = mac_downloads.join("document.pdf");
    assert!(received_file.exists());
    let received_content = tokio::fs::read(&received_file).await.unwrap();
    assert_eq!(received_content, payload);

    ios_conn.close(0u32.into(), b"done");
    ios_endpoint.close(0u32.into(), b"shutdown");
    tokio::fs::remove_dir_all(ios_source).await.ok();
    tokio::fs::remove_dir_all(mac_downloads).await.ok();
}

/// Test multi-file bidirectional transfer with nested folders between iOS and macOS.
#[tokio::test]
async fn multi_file_folder_transfer_between_ios_and_macos() {
    let ios_source = temp_dir("ios-multifile-source");
    let mac_downloads = temp_dir("mac-multifile-dest");

    let sub_folder = ios_source.join("vacation_photos");
    tokio::fs::create_dir_all(&sub_folder).await.unwrap();
    tokio::fs::write(sub_folder.join("beach.png"), b"beach image data")
        .await
        .unwrap();
    tokio::fs::write(sub_folder.join("sunset.png"), b"sunset image data")
        .await
        .unwrap();
    tokio::fs::write(ios_source.join("notes.txt"), b"trip notes")
        .await
        .unwrap();

    let mac_endpoint = create_endpoint("127.0.0.1:0".parse().unwrap()).unwrap();
    let mac_addr = mac_endpoint.local_addr().unwrap();

    let mac_dest_clone = mac_downloads.clone();
    let mac_receiver_task = tokio::spawn(async move {
        let incoming = mac_endpoint.accept().await.unwrap();
        let conn = incoming.await.unwrap();
        let (mut send, mut recv) = conn.accept_bi().await.unwrap();
        let mut noise = handshake_responder(&mut send, &mut recv).await.unwrap();

        let result =
            receive_transfer(&mut send, &mut recv, &mut noise, &mac_dest_clone, true).await;
        conn.close(0u32.into(), b"done");
        mac_endpoint.close(0u32.into(), b"shutdown");
        result
    });

    let ios_endpoint = create_endpoint("127.0.0.1:0".parse().unwrap()).unwrap();
    let ios_conn = ios_endpoint
        .connect(mac_addr, "localhost")
        .unwrap()
        .await
        .unwrap();
    let (mut ios_send, mut ios_recv) = ios_conn.open_bi().await.unwrap();
    let mut ios_noise = handshake_initiator(&mut ios_send, &mut ios_recv)
        .await
        .unwrap();

    let ios_sender = ios_identity();
    send_transfer(
        &mut ios_send,
        &mut ios_recv,
        &mut ios_noise,
        &[sub_folder, ios_source.join("notes.txt")],
        "xfer-ios-to-mac-multi",
        &ios_sender,
        |_| {},
    )
    .await
    .unwrap();

    let recv_result = mac_receiver_task.await.unwrap().unwrap();
    // 2 inner files in folder + 1 notes file = 3 items received
    assert_eq!(recv_result.items_received, 3);

    assert_eq!(
        tokio::fs::read(mac_downloads.join("vacation_photos/beach.png"))
            .await
            .unwrap(),
        b"beach image data"
    );
    assert_eq!(
        tokio::fs::read(mac_downloads.join("vacation_photos/sunset.png"))
            .await
            .unwrap(),
        b"sunset image data"
    );
    assert_eq!(
        tokio::fs::read(mac_downloads.join("notes.txt"))
            .await
            .unwrap(),
        b"trip notes"
    );

    ios_conn.close(0u32.into(), b"done");
    ios_endpoint.close(0u32.into(), b"shutdown");
    tokio::fs::remove_dir_all(ios_source).await.ok();
    tokio::fs::remove_dir_all(mac_downloads).await.ok();
}

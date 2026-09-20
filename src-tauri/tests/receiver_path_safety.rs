#![cfg(unix)]

use dukto_lib::crypto::noise::{handshake_initiator, handshake_responder};
use dukto_lib::state::device::DeviceIdentity;
use dukto_lib::transfer::quic::create_endpoint;
use dukto_lib::transfer::receiver::receive_transfer_with_accept;
use dukto_lib::transfer::sender::send_transfer;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};

struct TemporaryDirectory(PathBuf);

impl TemporaryDirectory {
    fn new(label: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "dukto-receiver-path-safety-{label}-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TemporaryDirectory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[tokio::test]
async fn receiver_rejects_symlinked_parent_without_writing_outside_destination() {
    // Mutation captured: bypassing safe destination resolution writes through this symlink.
    use std::os::unix::fs::symlink;

    let source = TemporaryDirectory::new("source");
    let destination = TemporaryDirectory::new("destination");
    let outside = TemporaryDirectory::new("outside");
    let source_subdirectory = source.path().join("escape");
    std::fs::create_dir_all(&source_subdirectory).unwrap();
    tokio::fs::write(
        source_subdirectory.join("payload.bin"),
        b"stay in destination",
    )
    .await
    .unwrap();

    let source_folder = source
        .path()
        .file_name()
        .unwrap()
        .to_string_lossy()
        .into_owned();
    let destination_source_folder = destination.path().join(source_folder);
    std::fs::create_dir_all(&destination_source_folder).unwrap();
    std::fs::write(outside.path().join("sentinel.txt"), b"preserve sentinel").unwrap();
    symlink(outside.path(), destination_source_folder.join("escape")).unwrap();

    let server_endpoint = create_endpoint("127.0.0.1:0".parse::<SocketAddr>().unwrap()).unwrap();
    let server_address = server_endpoint.local_addr().unwrap();
    let receiver_destination = destination.path().to_path_buf();
    let receiver = tokio::spawn(async move {
        let connection = server_endpoint.accept().await.unwrap().await.unwrap();
        let (mut send, mut recv) = connection.accept_bi().await.unwrap();
        let mut noise = handshake_responder(&mut send, &mut recv).await.unwrap();
        let result = receive_transfer_with_accept(
            &mut send,
            &mut recv,
            &mut noise,
            &receiver_destination,
            |_| async { true },
            |_| {},
        )
        .await;
        connection.close(0u32.into(), b"test finished");
        server_endpoint.close(0u32.into(), b"test finished");
        result
    });

    let client_endpoint = create_endpoint("127.0.0.1:0".parse::<SocketAddr>().unwrap()).unwrap();
    let connection = client_endpoint
        .connect(server_address, "localhost")
        .unwrap()
        .await
        .unwrap();
    let (mut send, mut recv) = connection.open_bi().await.unwrap();
    let mut noise = handshake_initiator(&mut send, &mut recv).await.unwrap();
    let sender = DeviceIdentity {
        device_id: "symlink-path-test-sender".into(),
        display_name: "Path Safety Test".into(),
        hostname: "path-safety-test.local".into(),
        platform: "test".into(),
    };
    let send_result = tokio::time::timeout(
        std::time::Duration::from_secs(10),
        send_transfer(
            &mut send,
            &mut recv,
            &mut noise,
            &[source.path().to_path_buf()],
            "symlink-path-safety",
            &sender,
            |_| {},
        ),
    )
    .await
    .expect("sender should terminate when the receiver rejects the path");
    let receive_result = tokio::time::timeout(std::time::Duration::from_secs(10), receiver)
        .await
        .expect("receiver should terminate")
        .unwrap();

    connection.close(0u32.into(), b"test finished");
    client_endpoint.close(0u32.into(), b"test finished");

    let outside_payload = outside.path().join("payload.bin");
    let outside_payload_exists = outside_payload.exists();
    let outside_sentinel = tokio::fs::read(outside.path().join("sentinel.txt"))
        .await
        .unwrap();

    assert!(
        receive_result.is_err(),
        "receiver accepted a path that crosses a symlink: {receive_result:?}; sender result: {send_result:?}"
    );
    assert_eq!(outside_sentinel, b"preserve sentinel");
    assert!(
        !outside_payload_exists,
        "receiver wrote the remote file outside the selected destination"
    );
}

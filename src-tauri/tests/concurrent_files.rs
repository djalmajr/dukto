use dukto_lib::crypto::noise::{handshake_initiator, handshake_responder};
use dukto_lib::state::device::DeviceIdentity;
use dukto_lib::transfer::quic::create_endpoint;
use dukto_lib::transfer::receiver::receive_transfer_with_accept;
use dukto_lib::transfer::sender::send_transfer;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Barrier;

fn temp_dir(suffix: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "dukto-concurrent-transfer-{}-{}",
        suffix,
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_same_name_large_transfers_are_preserved() {
    let destination = temp_dir("destination");
    let source_a = temp_dir("source-a");
    let source_b = temp_dir("source-b");
    let file_name = "large-shared.bin";
    let content_a = vec![b'A'; 2 * 1024 * 1024 + 17];
    let content_b = vec![b'B'; 2 * 1024 * 1024 + 31];
    tokio::fs::write(source_a.join(file_name), &content_a)
        .await
        .unwrap();
    tokio::fs::write(source_b.join(file_name), &content_b)
        .await
        .unwrap();

    let server_endpoint =
        create_endpoint("127.0.0.1:0".parse::<SocketAddr>().unwrap()).expect("server endpoint");
    let server_address = server_endpoint.local_addr().unwrap();
    let receiver_destination = destination.clone();
    let approval_barrier = Arc::new(Barrier::new(2));
    let receiver_barrier = approval_barrier.clone();
    let receiver = tokio::spawn(async move {
        let mut tasks = Vec::new();
        for _ in 0..2 {
            let incoming = server_endpoint.accept().await.unwrap();
            let destination = receiver_destination.clone();
            let approval_barrier = receiver_barrier.clone();
            tasks.push(tokio::spawn(async move {
                let connection = incoming.await.unwrap();
                let (mut send, mut recv) = connection.accept_bi().await.unwrap();
                let mut noise = handshake_responder(&mut send, &mut recv).await.unwrap();
                receive_transfer_with_accept(
                    &mut send,
                    &mut recv,
                    &mut noise,
                    &destination,
                    move |_| async move {
                        approval_barrier.wait().await;
                        true
                    },
                    |_| {},
                )
                .await
            }));
        }

        let mut results = Vec::new();
        for task in tasks {
            results.push(task.await.unwrap());
        }
        results
    });

    let mut senders = Vec::new();
    for (source, transfer_id) in [
        (source_a.clone(), "concurrent-a"),
        (source_b.clone(), "concurrent-b"),
    ] {
        senders.push(tokio::spawn(async move {
            let client_endpoint = create_endpoint("127.0.0.1:0".parse::<SocketAddr>().unwrap())
                .expect("client endpoint");
            let connection = client_endpoint
                .connect(server_address, "localhost")?
                .await?;
            let (mut send, mut recv) = connection.open_bi().await?;
            let mut noise = handshake_initiator(&mut send, &mut recv).await?;
            let sender = DeviceIdentity {
                device_id: "concurrent-sender".into(),
                display_name: "Test Sender".into(),
                hostname: "test-host.local".into(),
                platform: "linux".into(),
            };
            let result = send_transfer(
                &mut send,
                &mut recv,
                &mut noise,
                &[source.join(file_name)],
                transfer_id,
                &sender,
                |_| {},
            )
            .await;
            client_endpoint.close(0u32.into(), b"done");
            result
        }));
    }

    for sender in senders {
        sender.await.unwrap().unwrap();
    }
    let results = receiver.await.unwrap();
    for result in results {
        result.unwrap();
    }

    let original = tokio::fs::read(destination.join(file_name)).await.unwrap();
    let renamed = tokio::fs::read(destination.join("large-shared (1).bin"))
        .await
        .unwrap();
    let retained_both = (original == content_a && renamed == content_b)
        || (original == content_b && renamed == content_a);

    tokio::fs::remove_dir_all(destination).await.unwrap();
    tokio::fs::remove_dir_all(source_a).await.unwrap();
    tokio::fs::remove_dir_all(source_b).await.unwrap();

    assert!(
        retained_both,
        "concurrent same-name transfers must retain both complete payloads"
    );
}

use std::time::Duration;

use dukto_lib::{
    crypto::noise::{
        handshake_initiator_bound, handshake_responder_bound, recv_framed, send_framed,
    },
    internet::{
        endpoint::{InternetEndpoint, InternetEndpointConfig},
        pairing::{PairingOrigin, PairingTranscript},
    },
    protocol::{
        framing::decode_packet_header,
        types::{PacketType, TransferHeader},
    },
    transfer::channel::{AuthenticatedChannel, ChannelCloseReason},
};

const NOW: u64 = 1_800_000_000;
const SHARED_SECRET: [u8; 32] = [0x4D; 32];

#[tokio::test]
async fn pairing_transcript_binds_noise_before_the_first_transfer_header() {
    let server = loopback_endpoint().await;
    let client = loopback_endpoint().await;
    let server_id = server.endpoint_id().to_string();
    let client_id = client.endpoint_id().to_string();
    let transcript = transcript(&client_id, &server_id);
    let server_binding = transcript
        .derive_keys(&SHARED_SECRET)
        .expect("server pairing keys")
        .noise_binding()
        .to_owned();
    let client_binding = transcript
        .derive_keys(&SHARED_SECRET)
        .expect("client pairing keys")
        .noise_binding()
        .to_owned();
    let server_address = server.address();
    let expected_server_id = server.endpoint_id();

    let accepting = tokio::spawn(async move {
        let channel = server.accept().await.expect("incoming channel");
        let (mut send, mut receive) = channel.accept_bi().await.expect("incoming stream");
        let mut noise = handshake_responder_bound(&mut send, &mut receive, &server_binding)
            .await
            .expect("bound responder handshake");
        let encrypted = recv_framed(&mut receive).await.expect("encrypted header");
        let mut plaintext = vec![0_u8; 65_535];
        let length = noise
            .read_message(&encrypted, &mut plaintext)
            .expect("bound header decryption");
        let (packet_type, payload) =
            decode_packet_header(&plaintext[..length]).expect("transfer header packet");
        assert_eq!(packet_type, PacketType::TransferHeader);
        let header: TransferHeader = serde_json::from_slice(payload).unwrap();
        channel.close(ChannelCloseReason::Completed);
        server.close().await;
        header
    });

    let channel = client
        .connect_verified(server_address, expected_server_id)
        .await
        .expect("verified iroh channel");
    let (mut send, mut receive) = channel.open_bi().await.expect("outgoing stream");
    let mut noise = handshake_initiator_bound(&mut send, &mut receive, &client_binding)
        .await
        .expect("bound initiator handshake");
    let header = TransferHeader {
        transfer_id: "internet-bound".to_owned(),
        sender_device_id: "sender".to_owned(),
        sender: None,
        item_count: 1,
        total_size: 4,
    };
    let mut packet = vec![PacketType::TransferHeader as u8];
    packet.extend_from_slice(&serde_json::to_vec(&header).unwrap());
    let mut encrypted = vec![0_u8; 65_535];
    let length = noise.write_message(&packet, &mut encrypted).unwrap();
    send_framed(&mut send, &encrypted[..length])
        .await
        .expect("encrypted header");

    let received = accepting.await.expect("server task");
    assert_eq!(received.transfer_id, header.transfer_id);
    client.close().await;
}

#[tokio::test]
async fn substituted_endpoint_breaks_noise_before_any_transfer_header() {
    let server = loopback_endpoint().await;
    let client = loopback_endpoint().await;
    let server_id = server.endpoint_id().to_string();
    let client_id = client.endpoint_id().to_string();
    let client_binding = transcript(&client_id, &server_id)
        .derive_keys(&SHARED_SECRET)
        .unwrap()
        .noise_binding()
        .to_owned();
    let substituted_binding = transcript(&client_id, "substituted-endpoint")
        .derive_keys(&SHARED_SECRET)
        .unwrap()
        .noise_binding()
        .to_owned();
    let server_address = server.address();
    let expected_server_id = server.endpoint_id();

    let accepting = tokio::spawn(async move {
        let channel = server.accept().await.expect("incoming channel");
        let (mut send, mut receive) = channel.accept_bi().await.expect("incoming stream");
        let handshake =
            handshake_responder_bound(&mut send, &mut receive, &substituted_binding).await;
        server.close().await;
        handshake.is_err()
    });

    let channel = client
        .connect_verified(server_address, expected_server_id)
        .await
        .expect("transport-authenticated channel");
    let (mut send, mut receive) = channel.open_bi().await.expect("outgoing stream");
    let handshake = tokio::time::timeout(
        Duration::from_secs(3),
        handshake_initiator_bound(&mut send, &mut receive, &client_binding),
    )
    .await
    .expect("bound handshake must terminate");
    assert!(handshake.is_err());
    channel.close(ChannelCloseReason::AuthenticationFailed);
    assert!(accepting.await.expect("server task"));
    client.close().await;
}

async fn loopback_endpoint() -> InternetEndpoint {
    InternetEndpoint::bind(
        InternetEndpointConfig::default()
            .with_bind_addr("127.0.0.1:0".parse().unwrap())
            .with_handshake_timeout(Duration::from_secs(3)),
    )
    .await
    .expect("loopback internet endpoint")
}

fn transcript(initiator_endpoint: &str, responder_endpoint: &str) -> PairingTranscript {
    PairingTranscript::new(
        PairingOrigin::InviteLink,
        "018f1f3a-7730-7a2f-92d8-314278523b67",
        initiator_endpoint,
        responder_endpoint,
        [0x11; 32],
        [0x22; 32],
        NOW + 300,
        NOW,
    )
    .expect("pairing transcript")
}

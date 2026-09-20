use std::{
    path::PathBuf,
    process::Command,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use dukto_lib::{
    crypto::noise::{handshake_initiator_bound, handshake_responder_bound},
    internet::{
        endpoint::{InternetEndpoint, InternetEndpointConfig},
        pairing::{PairingOrigin, PairingTranscript, DUKTO_INTERNET_ALPN},
    },
    protocol::types::TransferProgress,
    state::device::DeviceIdentity,
    transfer::{
        channel::{AuthenticatedChannel, ChannelCloseReason, RouteKind},
        receiver::{receive_transfer_with_accept, receive_transfer_with_cancellation},
        sender::{send_transfer, send_transfer_with_cancellation, CancellableSendCallbacks},
    },
};
use iroh::{
    endpoint::presets, test_utils::run_relay_server, tls::CaTlsConfig, Endpoint, EndpointAddr,
    RelayMap, RelayMode, TransportAddr,
};

const NOW: u64 = 1_800_000_000;
const PAIRING_SECRET: [u8; 32] = [0x6D; 32];

#[tokio::test]
#[ignore = "requires an explicitly selected public HTTPS relay"]
async fn public_https_relay_transfers_a_verified_fixture_through_the_production_stack() {
    let relay_url = std::env::var("DUKTO_PUBLIC_RELAY_SMOKE_URL")
        .expect("set DUKTO_PUBLIC_RELAY_SMOKE_URL to run the public-relay transfer smoke test");
    let previous_relay_urls = std::env::var_os("DUKTO_RELAY_URLS");
    let previous_relay_only = std::env::var_os("DUKTO_FORCE_RELAY_ONLY");
    std::env::set_var("DUKTO_RELAY_URLS", relay_url);
    std::env::set_var("DUKTO_FORCE_RELAY_ONLY", "1");
    let server_config = InternetEndpointConfig::from_environment()
        .expect("valid relay-only server configuration")
        .with_handshake_timeout(Duration::from_secs(30));
    let client_config = InternetEndpointConfig::from_environment()
        .expect("valid relay-only client configuration")
        .with_handshake_timeout(Duration::from_secs(30));
    restore_environment("DUKTO_RELAY_URLS", previous_relay_urls);
    restore_environment("DUKTO_FORCE_RELAY_ONLY", previous_relay_only);

    let server = InternetEndpoint::bind(server_config)
        .await
        .expect("relay-only server endpoint");
    let client = InternetEndpoint::bind(client_config)
        .await
        .expect("relay-only client endpoint");
    server.wait_until_online().await.expect("server online");
    client.wait_until_online().await.expect("client online");

    let source = temp_dir("public-relay-source");
    let destination = temp_dir("public-relay-destination");
    let input = source.join("dukto-network-validation.bin");
    let payload: Vec<u8> = (0..128 * 1024)
        .map(|index| ((index * 31 + 17) % 251) as u8)
        .collect();
    tokio::fs::write(&input, &payload).await.unwrap();
    let expected_sha256 = sha256_hex(&payload);

    let server_id = server.endpoint_id();
    let client_id = client.endpoint_id();
    let binding = pairing_binding(&client_id.to_string(), &server_id.to_string());
    let server_address = server.address();
    let receiver_destination = destination.clone();
    let receiver = tokio::spawn(async move {
        let channel = server.accept().await.expect("public-relay connection");
        assert_eq!(channel.peer_id(), client_id.to_string());
        assert_eq!(channel.route_kind(), RouteKind::Relay);
        let (mut send, mut receive) = channel.accept_bi().await.expect("relayed stream");
        let mut noise = handshake_responder_bound(&mut send, &mut receive, &binding)
            .await
            .expect("bound responder handshake");
        let result = receive_transfer_with_accept(
            &mut send,
            &mut receive,
            &mut noise,
            &receiver_destination,
            |_| async { true },
            ignore_progress,
        )
        .await
        .expect("receive fixture");
        channel.close(ChannelCloseReason::Completed);
        server.close().await;
        result
    });

    let channel = client
        .connect_verified(server_address, server_id)
        .await
        .expect("verified public-relay connection");
    assert_eq!(channel.peer_id(), server_id.to_string());
    assert_eq!(channel.route_kind(), RouteKind::Relay);
    let (mut send, mut receive) = channel.open_bi().await.expect("outgoing stream");
    let mut noise = handshake_initiator_bound(
        &mut send,
        &mut receive,
        &pairing_binding(&client.endpoint_id().to_string(), &server_id.to_string()),
    )
    .await
    .expect("bound initiator handshake");
    let sent = send_transfer(
        &mut send,
        &mut receive,
        &mut noise,
        &[input],
        "public-relay-smoke",
        &identity(),
        ignore_progress,
    )
    .await
    .expect("send fixture");
    channel.close(ChannelCloseReason::Completed);
    client.close().await;

    let received = receiver.await.expect("receiver task");
    assert_eq!(sent, payload.len() as u64);
    assert_eq!(received.items_received, 1);
    assert_eq!(received.bytes_received, payload.len() as u64);
    let received_payload = tokio::fs::read(destination.join("dukto-network-validation.bin"))
        .await
        .unwrap();
    assert_eq!(sha256_hex(&received_payload), expected_sha256);
    assert_eq!(received_payload, payload);
    assert!(!std::fs::read_dir(&destination).unwrap().any(|entry| {
        entry
            .ok()
            .and_then(|entry| entry.file_name().into_string().ok())
            .is_some_and(|name| name.starts_with(".dukto-partial-"))
    }));

    tokio::fs::remove_dir_all(source).await.unwrap();
    tokio::fs::remove_dir_all(destination).await.unwrap();
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ExpectedRoute {
    Direct,
    Relay,
}

struct TransferOutcome {
    sender_result: Result<u64, String>,
    receiver_result: Result<(u32, u64), String>,
    sender_progress: Vec<u64>,
    receiver_progress: Vec<u64>,
}

#[tokio::test]
#[ignore = "manual transport-cost measurement; records observations without enforcing an SLO"]
async fn report_transport_cost_without_enforcing_thresholds() {
    const PAYLOAD_MIB: usize = 32;

    for route in [ExpectedRoute::Direct, ExpectedRoute::Relay] {
        let (setup_ms, idle_rss_kib, idle_inet_sockets) = measure_setup_cost(route).await;
        let source = temp_dir("metrics-source");
        let destination = temp_dir("metrics-destination");
        let payload = source.join("payload.bin");
        tokio::fs::write(&payload, vec![0x4D; PAYLOAD_MIB * 1024 * 1024])
            .await
            .unwrap();

        let started = Instant::now();
        let outcome = run_transfer_case(route, vec![payload], destination.clone(), true).await;
        let elapsed = started.elapsed();
        assert_eq!(
            outcome.sender_result.unwrap(),
            (PAYLOAD_MIB * 1024 * 1024) as u64
        );
        assert_eq!(
            outcome.receiver_result.unwrap(),
            (1, (PAYLOAD_MIB * 1024 * 1024) as u64)
        );

        let throughput_mib_s = PAYLOAD_MIB as f64 / elapsed.as_secs_f64();
        println!(
            "DUKTO_TRANSPORT_METRIC {}",
            serde_json::json!({
                "route": match route {
                    ExpectedRoute::Direct => "direct",
                    ExpectedRoute::Relay => "relay",
                },
                "payload_mib": PAYLOAD_MIB,
                "setup_ms": setup_ms,
                "end_to_end_ms": elapsed.as_millis(),
                "end_to_end_throughput_mib_s": (throughput_mib_s * 100.0).round() / 100.0,
                "idle_process_rss_kib": idle_rss_kib,
                "idle_process_inet_sockets": idle_inet_sockets,
            })
        );

        tokio::fs::remove_dir_all(source).await.unwrap();
        tokio::fs::remove_dir_all(destination).await.unwrap();
    }
}

#[tokio::test]
async fn single_file_and_batch_keep_checksums_and_progress_on_direct_and_relay_paths() {
    for route in [ExpectedRoute::Direct, ExpectedRoute::Relay] {
        let source = temp_dir("accepted-source");
        let single_destination = temp_dir("single-destination");
        let batch_destination = temp_dir("batch-destination");
        let first = source.join("alpha.bin");
        let second = source.join("beta.bin");
        let first_bytes: Vec<u8> = (0..96 * 1024).map(|index| (index % 251) as u8).collect();
        let second_bytes: Vec<u8> = (0..41 * 1024).map(|index| (index % 199) as u8).collect();
        tokio::fs::write(&first, &first_bytes).await.unwrap();
        tokio::fs::write(&second, &second_bytes).await.unwrap();

        let single =
            run_transfer_case(route, vec![first.clone()], single_destination.clone(), true).await;
        assert_eq!(
            single.sender_result.unwrap(),
            first_bytes.len() as u64,
            "{route:?}"
        );
        assert_eq!(
            single.receiver_result.unwrap(),
            (1, first_bytes.len() as u64),
            "{route:?}"
        );
        assert_eq!(
            single.sender_progress.last(),
            Some(&(first_bytes.len() as u64)),
            "{route:?}"
        );
        assert_eq!(
            tokio::fs::read(single_destination.join("alpha.bin"))
                .await
                .unwrap(),
            first_bytes
        );

        let batch = run_transfer_case(
            route,
            vec![first.clone(), second.clone()],
            batch_destination.clone(),
            true,
        )
        .await;
        let expected_bytes = (first_bytes.len() + second_bytes.len()) as u64;
        assert_eq!(batch.sender_result.unwrap(), expected_bytes, "{route:?}");
        assert_eq!(
            batch.receiver_result.unwrap(),
            (2, expected_bytes),
            "{route:?}"
        );
        assert_eq!(
            batch.sender_progress.last(),
            Some(&expected_bytes),
            "{route:?}"
        );
        assert_eq!(
            batch.receiver_progress.last(),
            Some(&expected_bytes),
            "{route:?}"
        );
        assert_eq!(
            tokio::fs::read(batch_destination.join("alpha.bin"))
                .await
                .unwrap(),
            first_bytes
        );
        assert_eq!(
            tokio::fs::read(batch_destination.join("beta.bin"))
                .await
                .unwrap(),
            second_bytes
        );

        tokio::fs::remove_dir_all(source).await.unwrap();
        tokio::fs::remove_dir_all(single_destination).await.unwrap();
        tokio::fs::remove_dir_all(batch_destination).await.unwrap();
    }
}

#[tokio::test]
async fn rejection_and_cancellation_are_bounded_on_direct_and_relay_paths() {
    for route in [ExpectedRoute::Direct, ExpectedRoute::Relay] {
        let source = temp_dir("negative-source");
        let rejected_destination = temp_dir("rejected-destination");
        let cancelled_destination = temp_dir("cancelled-destination");
        let payload = source.join("private.bin");
        tokio::fs::write(&payload, vec![0xA5; 128 * 1024])
            .await
            .unwrap();

        let rejected = run_transfer_case(
            route,
            vec![payload.clone()],
            rejected_destination.clone(),
            false,
        )
        .await;
        assert!(rejected.sender_result.is_err(), "{route:?}");
        assert!(rejected.receiver_result.is_err(), "{route:?}");
        assert_eq!(std::fs::read_dir(&rejected_destination).unwrap().count(), 0);

        run_cancelled_transfer(route, payload, cancelled_destination.clone(), false).await;
        assert_eq!(
            std::fs::read_dir(&cancelled_destination).unwrap().count(),
            0
        );

        tokio::fs::remove_dir_all(source).await.unwrap();
        tokio::fs::remove_dir_all(rejected_destination)
            .await
            .unwrap();
        tokio::fs::remove_dir_all(cancelled_destination)
            .await
            .unwrap();
    }
}

#[tokio::test]
async fn directories_empty_folders_and_conflicts_match_lan_behavior_on_both_routes() {
    for route in [ExpectedRoute::Direct, ExpectedRoute::Relay] {
        let source = temp_dir("directory-source");
        let project = source.join("project");
        tokio::fs::create_dir_all(project.join("sub"))
            .await
            .unwrap();
        tokio::fs::create_dir_all(project.join("empty"))
            .await
            .unwrap();
        tokio::fs::write(project.join("root.txt"), b"new root")
            .await
            .unwrap();
        tokio::fs::write(project.join("sub/nested.txt"), b"nested")
            .await
            .unwrap();
        let destination = temp_dir("directory-destination");
        tokio::fs::create_dir_all(destination.join("project"))
            .await
            .unwrap();
        tokio::fs::write(destination.join("project/root.txt"), b"existing root")
            .await
            .unwrap();

        let outcome = run_transfer_case(route, vec![project], destination.clone(), true).await;
        assert_eq!(outcome.sender_result.unwrap(), 14, "{route:?}");
        assert_eq!(outcome.receiver_result.unwrap(), (3, 14), "{route:?}");
        assert_eq!(
            tokio::fs::read(destination.join("project/root.txt"))
                .await
                .unwrap(),
            b"existing root"
        );
        assert_eq!(
            tokio::fs::read(destination.join("project/root (1).txt"))
                .await
                .unwrap(),
            b"new root"
        );
        assert_eq!(
            tokio::fs::read(destination.join("project/sub/nested.txt"))
                .await
                .unwrap(),
            b"nested"
        );
        assert!(destination.join("project/empty").is_dir());

        tokio::fs::remove_dir_all(source).await.unwrap();
        tokio::fs::remove_dir_all(destination).await.unwrap();
    }
}

#[tokio::test]
async fn interruption_after_payload_start_removes_partial_files_on_both_routes() {
    for route in [ExpectedRoute::Direct, ExpectedRoute::Relay] {
        let source = temp_dir("partial-source");
        let destination = temp_dir("partial-destination");
        let payload = source.join("large.bin");
        tokio::fs::write(&payload, vec![0x5C; 8 * 1024 * 1024])
            .await
            .unwrap();

        run_cancelled_transfer(route, payload, destination.clone(), true).await;
        assert_eq!(
            std::fs::read_dir(&destination).unwrap().count(),
            0,
            "{route:?}"
        );

        tokio::fs::remove_dir_all(source).await.unwrap();
        tokio::fs::remove_dir_all(destination).await.unwrap();
    }
}

async fn run_transfer_case(
    route: ExpectedRoute,
    inputs: Vec<PathBuf>,
    destination: PathBuf,
    accept: bool,
) -> TransferOutcome {
    let relay_fixture = match route {
        ExpectedRoute::Direct => None,
        ExpectedRoute::Relay => Some(run_relay_server().await.expect("disposable relay")),
    };
    let relay_map = relay_fixture.as_ref().map(|fixture| fixture.0.clone());
    let server = endpoint(relay_map.clone(), true).await;
    let client = endpoint(relay_map, false).await;
    if route == ExpectedRoute::Relay {
        server.online().await;
        client.online().await;
    }

    let server_id = server.id();
    let client_id = client.id();
    let binding = pairing_binding(&client_id.to_string(), &server_id.to_string());
    let address = dial_address(&server, route);
    let receiver_progress = Arc::new(Mutex::new(Vec::new()));
    let receiver_snapshots = receiver_progress.clone();
    let accepting_server = server.clone();
    let receiver = tokio::spawn(async move {
        let incoming = accepting_server
            .accept()
            .await
            .expect("incoming connection");
        let connection = incoming.await.expect("authenticated connection");
        assert_eq!(connection.remote_id(), client_id);
        let (mut send, mut receive) = connection.accept_bi().await.expect("incoming stream");
        let mut noise = handshake_responder_bound(&mut send, &mut receive, &binding)
            .await
            .expect("bound responder handshake");
        let result = receive_transfer_with_accept(
            &mut send,
            &mut receive,
            &mut noise,
            &destination,
            move |_| async move { accept },
            move |progress| receiver_snapshots.lock().unwrap().push(progress.bytes_sent),
        )
        .await
        .map(|result| (result.items_received, result.bytes_received))
        .map_err(|error| error.to_string());
        connection.close(0u32.into(), b"case complete");
        result
    });

    let connection = tokio::time::timeout(
        Duration::from_secs(5),
        client.connect(address, DUKTO_INTERNET_ALPN),
    )
    .await
    .expect("bounded connection")
    .expect("connected endpoint");
    assert_eq!(connection.remote_id(), server_id);
    assert_route(&connection, route);
    let (mut send, mut receive) = connection.open_bi().await.expect("outgoing stream");
    let mut noise = handshake_initiator_bound(
        &mut send,
        &mut receive,
        &pairing_binding(&client.id().to_string(), &server_id.to_string()),
    )
    .await
    .expect("bound initiator handshake");
    let sender_progress = Arc::new(Mutex::new(Vec::new()));
    let sender_snapshots = sender_progress.clone();
    let sender_result = send_transfer(
        &mut send,
        &mut receive,
        &mut noise,
        &inputs,
        "internet-e2e",
        &identity(),
        move |progress| sender_snapshots.lock().unwrap().push(progress.bytes_sent),
    )
    .await
    .map_err(|error| error.to_string());
    let receiver_result = receiver.await.expect("receiver task");

    connection.close(0u32.into(), b"case complete");
    client.close().await;
    server.close().await;
    drop(relay_fixture);
    TransferOutcome {
        sender_result,
        receiver_result,
        sender_progress: Arc::try_unwrap(sender_progress)
            .unwrap()
            .into_inner()
            .unwrap(),
        receiver_progress: Arc::try_unwrap(receiver_progress)
            .unwrap()
            .into_inner()
            .unwrap(),
    }
}

async fn run_cancelled_transfer(
    route: ExpectedRoute,
    input: PathBuf,
    destination: PathBuf,
    after_progress: bool,
) {
    let relay_fixture = match route {
        ExpectedRoute::Direct => None,
        ExpectedRoute::Relay => Some(run_relay_server().await.expect("disposable relay")),
    };
    let relay_map = relay_fixture.as_ref().map(|fixture| fixture.0.clone());
    let server = endpoint(relay_map.clone(), true).await;
    let client = endpoint(relay_map, false).await;
    if route == ExpectedRoute::Relay {
        server.online().await;
        client.online().await;
    }

    let server_id = server.id();
    let client_id = client.id();
    let binding = pairing_binding(&client_id.to_string(), &server_id.to_string());
    let address = dial_address(&server, route);
    let (approval_started_tx, approval_started_rx) = tokio::sync::oneshot::channel();
    let (progress_started_tx, progress_started_rx) = tokio::sync::oneshot::channel();
    let accepting_server = server.clone();
    let receiver = tokio::spawn(async move {
        let connection = accepting_server.accept().await.unwrap().await.unwrap();
        let (mut send, mut receive) = connection.accept_bi().await.unwrap();
        let mut noise = handshake_responder_bound(&mut send, &mut receive, &binding)
            .await
            .unwrap();
        let mut progress_started_tx = Some(progress_started_tx);
        let result = receive_transfer_with_cancellation(
            &mut send,
            &mut receive,
            &mut noise,
            &destination,
            move |_, mut peer_cancelled| async move {
                let _ = approval_started_tx.send(());
                if after_progress {
                    return true;
                }
                while !*peer_cancelled.borrow() {
                    if peer_cancelled.changed().await.is_err() {
                        break;
                    }
                }
                false
            },
            move |progress| {
                if progress.bytes_sent > 0 {
                    if let Some(progress_started_tx) = progress_started_tx.take() {
                        let _ = progress_started_tx.send(());
                    }
                }
            },
            std::future::pending(),
        )
        .await;
        connection.close(0u32.into(), b"cancel complete");
        result.map_err(|error| error.to_string())
    });

    let connection = client.connect(address, DUKTO_INTERNET_ALPN).await.unwrap();
    assert_route(&connection, route);
    let (mut send, mut receive) = connection.open_bi().await.unwrap();
    let mut noise = handshake_initiator_bound(
        &mut send,
        &mut receive,
        &pairing_binding(&client.id().to_string(), &server_id.to_string()),
    )
    .await
    .unwrap();
    let sender_result = send_transfer_with_cancellation(
        &mut send,
        &mut receive,
        &mut noise,
        &[input],
        "internet-cancel",
        &identity(),
        CancellableSendCallbacks {
            cancelled: async move {
                if after_progress {
                    let _ = progress_started_rx.await;
                } else {
                    let _ = approval_started_rx.await;
                }
            },
            on_progress: ignore_progress,
            on_accepted: || {},
        },
    )
    .await;
    assert!(
        sender_result.unwrap_err().to_string().contains("cancelled"),
        "{route:?}"
    );
    let receiver_error = tokio::time::timeout(Duration::from_secs(5), receiver)
        .await
        .expect("bounded receiver cancellation")
        .unwrap()
        .unwrap_err();
    assert!(
        receiver_error.contains("cancelled"),
        "{route:?}: {receiver_error}"
    );

    connection.close(0u32.into(), b"cancel complete");
    client.close().await;
    server.close().await;
    drop(relay_fixture);
}

async fn endpoint(relay_map: Option<RelayMap>, accepts_internet: bool) -> Endpoint {
    let mut builder = Endpoint::builder(presets::Minimal);
    builder = if let Some(relay_map) = relay_map {
        builder
            .clear_ip_transports()
            .relay_mode(RelayMode::Custom(relay_map))
            .ca_tls_config(CaTlsConfig::insecure_skip_verify())
    } else {
        builder
            .clear_relay_transports()
            .clear_ip_transports()
            .bind_addr("127.0.0.1:0")
            .unwrap()
    };
    if accepts_internet {
        builder = builder.alpns(vec![DUKTO_INTERNET_ALPN.to_vec()]);
    }
    builder.bind().await.unwrap()
}

fn dial_address(server: &Endpoint, route: ExpectedRoute) -> EndpointAddr {
    match route {
        ExpectedRoute::Direct => server.addr(),
        ExpectedRoute::Relay => EndpointAddr::from_parts(
            server.id(),
            server
                .addr()
                .addrs
                .into_iter()
                .filter(|address| matches!(address, TransportAddr::Relay(_))),
        ),
    }
}

fn assert_route(connection: &iroh::endpoint::Connection, expected: ExpectedRoute) {
    let uses_relay = connection
        .paths()
        .iter()
        .any(|path| path.is_selected() && path.is_relay());
    assert_eq!(uses_relay, expected == ExpectedRoute::Relay, "{expected:?}");
}

fn pairing_binding(initiator: &str, responder: &str) -> [u8; 32] {
    PairingTranscript::new(
        PairingOrigin::InviteLink,
        "018f1f3a-7730-7a2f-92d8-314278523b67",
        initiator,
        responder,
        [0x11; 32],
        [0x22; 32],
        NOW + 300,
        NOW,
    )
    .unwrap()
    .derive_keys(&PAIRING_SECRET)
    .unwrap()
    .noise_binding()
    .to_owned()
}

fn identity() -> DeviceIdentity {
    DeviceIdentity {
        device_id: "internet-sender".to_owned(),
        display_name: "Internet Sender".to_owned(),
        hostname: "internet-sender.local".to_owned(),
        platform: "test".to_owned(),
    }
}

fn ignore_progress(_: &TransferProgress) {}

fn restore_environment(name: &str, previous: Option<std::ffi::OsString>) {
    match previous {
        Some(value) => std::env::set_var(name, value),
        None => std::env::remove_var(name),
    }
}

fn sha256_hex(payload: &[u8]) -> String {
    ring::digest::digest(&ring::digest::SHA256, payload)
        .as_ref()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

async fn measure_setup_cost(route: ExpectedRoute) -> (u128, Option<u64>, Option<usize>) {
    let relay_fixture = match route {
        ExpectedRoute::Direct => None,
        ExpectedRoute::Relay => Some(run_relay_server().await.expect("disposable relay")),
    };
    let relay_map = relay_fixture.as_ref().map(|fixture| fixture.0.clone());
    let server = endpoint(relay_map.clone(), true).await;
    let client = endpoint(relay_map, false).await;
    if route == ExpectedRoute::Relay {
        server.online().await;
        client.online().await;
    }

    let idle_rss_kib = process_rss_kib();
    let idle_inet_sockets = process_inet_socket_count();
    let server_id = server.id();
    let client_id = client.id();
    let binding = pairing_binding(&client_id.to_string(), &server_id.to_string());
    let accepting_server = server.clone();
    let (ready_tx, ready_rx) = tokio::sync::oneshot::channel();
    let server_task = tokio::spawn(async move {
        let connection = accepting_server.accept().await.unwrap().await.unwrap();
        let (mut send, mut receive) = connection.accept_bi().await.unwrap();
        let _noise = handshake_responder_bound(&mut send, &mut receive, &binding)
            .await
            .unwrap();
        let _ = ready_tx.send(());
        connection
    });

    let started = Instant::now();
    let connection = client
        .connect(dial_address(&server, route), DUKTO_INTERNET_ALPN)
        .await
        .unwrap();
    assert_route(&connection, route);
    let (mut send, mut receive) = connection.open_bi().await.unwrap();
    let _noise = handshake_initiator_bound(
        &mut send,
        &mut receive,
        &pairing_binding(&client.id().to_string(), &server_id.to_string()),
    )
    .await
    .unwrap();
    ready_rx.await.unwrap();
    let setup_ms = started.elapsed().as_millis();

    connection.close(0u32.into(), b"metrics setup complete");
    let server_connection = server_task.await.unwrap();
    server_connection.close(0u32.into(), b"metrics setup complete");
    client.close().await;
    server.close().await;
    drop(relay_fixture);

    (setup_ms, idle_rss_kib, idle_inet_sockets)
}

fn process_rss_kib() -> Option<u64> {
    let output = Command::new("ps")
        .args(["-o", "rss=", "-p", &std::process::id().to_string()])
        .output()
        .ok()?;
    output.status.success().then(|| {
        String::from_utf8_lossy(&output.stdout)
            .trim()
            .parse::<u64>()
            .ok()
    })?
}

fn process_inet_socket_count() -> Option<usize> {
    let output = Command::new("lsof")
        .args(["-nP", "-a", "-p", &std::process::id().to_string(), "-i"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    Some(
        String::from_utf8_lossy(&output.stdout)
            .lines()
            .skip(1)
            .count(),
    )
}

fn temp_dir(suffix: &str) -> PathBuf {
    let directory = std::env::temp_dir().join(format!(
        "dukto-internet-e2e-{suffix}-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&directory).unwrap();
    directory
}

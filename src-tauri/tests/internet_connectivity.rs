use std::time::Duration;

use iroh::{
    endpoint::{presets, Connection, ConnectionError},
    test_utils::run_relay_server,
    tls::CaTlsConfig,
    Endpoint, EndpointAddr, EndpointId, RelayMap, RelayMode, SecretKey, TransportAddr,
};
use tokio::time::timeout;

const DUKTO_INTERNET_ALPN: &[u8] = b"dukto/internet-spike/1";
const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(5);
const USER_CANCELLED_CODE: u32 = 499;

#[tokio::test]
async fn direct_connection_authenticates_the_expected_endpoint_before_payload() {
    let server = loopback_endpoint(true).await;
    let client = loopback_endpoint(false).await;

    let expected_server = server.id();
    let expected_client = client.id();
    let accepting_server = server.clone();
    let server_task = tokio::spawn(async move {
        let incoming = accepting_server
            .accept()
            .await
            .expect("server should receive the direct connection");
        let connection = incoming
            .await
            .expect("server should authenticate the incoming endpoint");

        assert_eq!(connection.remote_id(), expected_client);

        let (mut send, mut receive) = connection
            .accept_bi()
            .await
            .expect("client should open an application stream");
        let payload = receive
            .read_to_end(1024)
            .await
            .expect("server should read the bounded spike payload");
        assert_eq!(payload, b"dukto-spike-payload");

        send.write_all(b"dukto-spike-ack")
            .await
            .expect("server should write the acknowledgement");
        send.finish()
            .expect("server should finish the acknowledgement stream");
        send.stopped()
            .await
            .expect("client should acknowledge the complete response stream");
    });

    let connection = connect_verified(&client, server.addr(), expected_server)
        .await
        .expect("the expected endpoint identity should connect");

    assert_eq!(connection.remote_id(), expected_server);

    let (mut send, mut receive) = connection
        .open_bi()
        .await
        .expect("client should open an application stream");
    send.write_all(b"dukto-spike-payload")
        .await
        .expect("client should write the spike payload");
    send.finish()
        .expect("client should finish the spike payload stream");
    let acknowledgement = receive
        .read_to_end(1024)
        .await
        .expect("client should read the bounded acknowledgement");
    assert_eq!(acknowledgement, b"dukto-spike-ack");

    server_task
        .await
        .expect("server task should complete without panicking");

    client.close().await;
    server.close().await;
}

#[tokio::test]
async fn wrong_expected_endpoint_is_rejected_before_payload() {
    let server = loopback_endpoint(true).await;
    let client = loopback_endpoint(false).await;

    let wrong_endpoint = SecretKey::from_bytes(&[0xA5; 32]).public();
    let accepting_server = server.clone();
    let server_task = tokio::spawn(async move {
        if let Some(incoming) = accepting_server.accept().await {
            let _ = incoming.await;
        }
    });

    let attempt = timeout(
        HANDSHAKE_TIMEOUT,
        connect_verified(&client, server.addr(), wrong_endpoint),
    )
    .await
    .expect("the expected-identity check should finish within the handshake timeout");

    assert!(
        attempt.is_err(),
        "an authenticated peer must not be accepted under a different invitation identity"
    );

    client.close().await;
    server.close().await;
    let _ = timeout(HANDSHAKE_TIMEOUT, server_task).await;
}

#[tokio::test]
async fn transport_rejects_a_forged_endpoint_id_for_the_real_socket() {
    let server = loopback_endpoint(true).await;
    let client = loopback_endpoint(false).await;

    let actual_addr = server.addr();
    let forged_endpoint = SecretKey::from_bytes(&[0x5A; 32]).public();
    let forged_addr = EndpointAddr::from_parts(forged_endpoint, actual_addr.addrs.clone());
    let accepting_server = server.clone();
    let server_task = tokio::spawn(async move {
        if let Some(incoming) = accepting_server.accept().await {
            let _ = incoming.await;
        }
    });

    let attempt = timeout(
        HANDSHAKE_TIMEOUT,
        client.connect(forged_addr, DUKTO_INTERNET_ALPN),
    )
    .await
    .expect("the forged transport handshake should fail within the timeout");

    assert!(
        attempt.is_err(),
        "iroh must bind the QUIC handshake to the endpoint public key"
    );

    client.close().await;
    server.close().await;
    let _ = timeout(HANDSHAKE_TIMEOUT, server_task).await;
}

#[tokio::test]
async fn user_cancellation_reaches_the_authenticated_peer_before_payload() {
    let server = loopback_endpoint(true).await;
    let client = loopback_endpoint(false).await;

    let expected_server = server.id();
    let expected_client = client.id();
    let accepting_server = server.clone();
    let server_task = tokio::spawn(async move {
        let incoming = accepting_server
            .accept()
            .await
            .expect("server should receive the connection before cancellation");
        let connection = incoming
            .await
            .expect("server should authenticate the cancelling peer");
        assert_eq!(connection.remote_id(), expected_client);
        connection.closed().await
    });

    let connection = connect_verified(&client, server.addr(), expected_server)
        .await
        .expect("authenticated connection should complete before cancellation");
    connection.close(USER_CANCELLED_CODE.into(), b"user cancelled");

    let remote_error = timeout(HANDSHAKE_TIMEOUT, server_task)
        .await
        .expect("cancellation should reach the peer within the timeout")
        .expect("server cancellation task should not panic");
    match remote_error {
        ConnectionError::ApplicationClosed(close) => {
            assert_eq!(
                close.error_code.into_inner(),
                u64::from(USER_CANCELLED_CODE)
            );
            assert_eq!(close.reason.as_ref(), b"user cancelled");
        }
        other => panic!("expected an application cancellation, got {other:?}"),
    }

    client.close().await;
    server.close().await;
}

#[tokio::test]
async fn encrypted_payload_crosses_a_forced_relay_path() {
    let (relay_map, _relay_url, relay_server) = run_relay_server()
        .await
        .expect("the disposable relay should start");
    let server = relay_only_endpoint(relay_map.clone(), true).await;
    let client = relay_only_endpoint(relay_map.clone(), false).await;

    server.online().await;
    client.online().await;

    let expected_server = server.id();
    let expected_client = client.id();
    let accepting_server = server.clone();
    let server_task = tokio::spawn(async move {
        let incoming = accepting_server
            .accept()
            .await
            .expect("server should receive the relayed connection");
        let connection = incoming
            .await
            .expect("server should authenticate the relayed endpoint");

        assert_eq!(connection.remote_id(), expected_client);

        let (mut send, mut receive) = connection
            .accept_bi()
            .await
            .expect("client should open a stream through the relay");
        let payload = receive
            .read_to_end(1024)
            .await
            .expect("server should read the bounded relayed payload");
        assert_eq!(payload, b"encrypted-relay-payload");

        send.write_all(b"encrypted-relay-ack")
            .await
            .expect("server should write the relayed acknowledgement");
        send.finish()
            .expect("server should finish the relayed acknowledgement");
        send.stopped()
            .await
            .expect("client should acknowledge the relayed response");

        connection
    });

    let relay_address = EndpointAddr::from_parts(
        expected_server,
        server
            .addr()
            .addrs
            .into_iter()
            .filter(|address| matches!(address, TransportAddr::Relay(_))),
    );
    assert!(
        !relay_address.addrs.is_empty()
            && relay_address
                .addrs
                .iter()
                .all(|address| matches!(address, TransportAddr::Relay(_))),
        "the forced-relay invitation must contain a relay and no direct IP address"
    );

    let retry_address = relay_address.clone();
    let connection = connect_verified(&client, relay_address, expected_server)
        .await
        .expect("the relay-only endpoint should connect");
    assert!(
        connection
            .paths()
            .iter()
            .any(|path| path.is_selected() && path.is_relay()),
        "application data should initially use the relay path"
    );

    let (mut send, mut receive) = connection
        .open_bi()
        .await
        .expect("client should open the relayed application stream");
    send.write_all(b"encrypted-relay-payload")
        .await
        .expect("client should write the relayed payload");
    send.finish()
        .expect("client should finish the relayed payload stream");
    let acknowledgement = receive
        .read_to_end(1024)
        .await
        .expect("client should read the relayed acknowledgement");
    assert_eq!(acknowledgement, b"encrypted-relay-ack");

    let server_connection = server_task
        .await
        .expect("relay server task should complete without panicking");

    drop(relay_server);
    let retry_client = relay_only_endpoint(relay_map, false).await;
    let reconnect_attempt = connect_verified(&retry_client, retry_address, expected_server).await;
    assert!(
        reconnect_attempt.is_err(),
        "a new relay-only connection should fail within the bounded handshake timeout after service loss"
    );

    connection.close(0u32.into(), b"spike complete");
    server_connection.close(0u32.into(), b"spike complete");
    retry_client.close().await;
    client.close().await;
    server.close().await;
}

async fn loopback_endpoint(accepts_dukto: bool) -> Endpoint {
    let mut builder = Endpoint::builder(presets::Minimal)
        .clear_ip_transports()
        .bind_addr("127.0.0.1:0")
        .expect("loopback bind configuration should be valid");

    if accepts_dukto {
        builder = builder.alpns(vec![DUKTO_INTERNET_ALPN.to_vec()]);
    }

    builder.bind().await.expect("loopback endpoint should bind")
}

async fn relay_only_endpoint(relay_map: RelayMap, accepts_dukto: bool) -> Endpoint {
    let mut builder = Endpoint::builder(presets::Minimal)
        .clear_ip_transports()
        .relay_mode(RelayMode::Custom(relay_map))
        .ca_tls_config(CaTlsConfig::insecure_skip_verify());

    if accepts_dukto {
        builder = builder.alpns(vec![DUKTO_INTERNET_ALPN.to_vec()]);
    }

    builder
        .bind()
        .await
        .expect("relay-only endpoint should bind")
}

async fn connect_verified(
    endpoint: &Endpoint,
    remote_address: EndpointAddr,
    expected_endpoint: EndpointId,
) -> Result<Connection, String> {
    if remote_address.id != expected_endpoint {
        return Err("remote address is not bound to the expected endpoint identity".to_owned());
    }

    let connection = timeout(
        HANDSHAKE_TIMEOUT,
        endpoint.connect(remote_address, DUKTO_INTERNET_ALPN),
    )
    .await
    .map_err(|_| "connection handshake timed out".to_owned())?
    .map_err(|error| format!("connection handshake failed: {error}"))?;

    if connection.remote_id() != expected_endpoint {
        connection.close(1u32.into(), b"unexpected endpoint identity");
        return Err("authenticated endpoint identity did not match the invitation".to_owned());
    }

    Ok(connection)
}

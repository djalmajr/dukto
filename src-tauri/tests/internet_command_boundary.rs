#![cfg(all(feature = "desktop", not(target_os = "windows")))]

use dukto_lib::internet::{
    endpoint::{InternetEndpoint, InternetEndpointConfig},
    session::{
        authenticate_initiator, authenticate_responder, InternetSessionCredentials,
        InternetSessionRole,
    },
};
use dukto_lib::state::{
    app_state::AppState,
    device::DeviceIdentity,
    internet_session::{RemoteSessionSecret, RemoteSessionStatus},
};
use dukto_lib::transfer::receiver::receive_transfer_with_accept;
use dukto_lib::{internet::invite::InternetInvite, internet::rendezvous::RendezvousSlot};
use serde_json::json;
use tauri::{test, Manager, WebviewWindowBuilder};
use zeroize::Zeroizing;

const NOW: u64 = 1_800_000_000;

#[test]
fn internet_commands_are_reachable_through_the_tauri_ipc_boundary() {
    let data_dir = std::env::temp_dir().join(format!(
        "dukto-internet-command-boundary-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&data_dir).unwrap();
    let app_state = AppState::new(
        DeviceIdentity {
            device_id: "ipc-device".to_owned(),
            display_name: "IPC device".to_owned(),
            hostname: "ipc-device.local".to_owned(),
            platform: "test".to_owned(),
        },
        data_dir.clone(),
    );
    app_state
        .remote_sessions
        .insert(
            "cancel-through-ipc",
            "remote-endpoint",
            NOW + 300,
            RemoteSessionSecret::new([0x42; 32]),
            NOW,
        )
        .unwrap();

    let app = test::mock_builder()
        .manage(app_state)
        .invoke_handler(tauri::generate_handler![
            dukto_lib::commands::internet::cancel_internet_invite,
            dukto_lib::commands::internet::import_internet_invite,
            dukto_lib::commands::internet::confirm_internet_pairing_code
        ])
        .build(test::mock_context(test::noop_assets()))
        .unwrap();
    let webview = WebviewWindowBuilder::new(&app, "main", Default::default())
        .build()
        .unwrap();

    let cancelled = test::get_ipc_response(
        &webview,
        request(
            "cancel_internet_invite",
            json!({ "sessionId": "cancel-through-ipc" }),
        ),
    );
    assert!(cancelled.is_ok());
    assert!(app
        .state::<AppState>()
        .remote_sessions
        .view("cancel-through-ipc")
        .is_none());

    let duplicate_cancel_error = test::get_ipc_response(
        &webview,
        request(
            "cancel_internet_invite",
            json!({ "sessionId": "cancel-through-ipc" }),
        ),
    )
    .unwrap_err();
    assert_eq!(
        duplicate_cancel_error,
        json!("internet invitation was not found")
    );

    let now_unix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let slot = RendezvousSlot::generate().unwrap();
    let invite = InternetInvite::new(
        "ipc-remote-endpoint",
        vec!["ip:203.0.113.20:4242".to_owned()],
        Some(slot.expose().to_owned()),
        now_unix,
        300,
    )
    .unwrap();
    let imported = test::get_ipc_response(
        &webview,
        request(
            "import_internet_invite",
            json!({ "invitation": invite.to_canonical_json().unwrap() }),
        ),
    )
    .unwrap()
    .deserialize::<serde_json::Value>()
    .unwrap();
    assert_eq!(imported["session_id"], invite.session_id());
    assert_eq!(imported["status"]["state"], "invited");

    let invalid_code = test::get_ipc_response(
        &webview,
        request(
            "confirm_internet_pairing_code",
            json!({ "sessionId": invite.session_id(), "code": "not-a-code" }),
        ),
    )
    .unwrap_err();
    assert_eq!(invalid_code, json!("manual pairing code is invalid"));
    assert!(app
        .state::<AppState>()
        .remote_sessions
        .view(invite.session_id())
        .is_some());

    drop(webview);
    drop(app);
    std::fs::remove_dir_all(data_dir).unwrap();
}

#[test]
fn send_command_transfers_verified_bytes_through_the_tauri_ipc_boundary() {
    let root = std::env::temp_dir().join(format!(
        "dukto-internet-send-command-boundary-{}",
        uuid::Uuid::new_v4()
    ));
    let source = root.join("source");
    let destination = root.join("destination");
    std::fs::create_dir_all(&source).unwrap();
    std::fs::create_dir_all(&destination).unwrap();
    let fixture = source.join("ipc-verified.bin");
    let expected: Vec<u8> = (0..128 * 1024).map(|index| (index % 251) as u8).collect();
    std::fs::write(&fixture, &expected).unwrap();

    let now_unix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let expires_at_unix = now_unix + 300;
    let session_id = uuid::Uuid::new_v4().to_string();
    let session_secret = [0x6D; 32];
    let (server, client, receiver_session, sender_session) =
        tauri::async_runtime::block_on(async {
            let server = InternetEndpoint::bind(
                InternetEndpointConfig::default().with_bind_addr("127.0.0.1:0".parse().unwrap()),
            )
            .await
            .unwrap();
            let client = InternetEndpoint::bind(
                InternetEndpointConfig::default().with_bind_addr("127.0.0.1:0".parse().unwrap()),
            )
            .await
            .unwrap();
            let server_id = server.endpoint_id();
            let client_id = client.endpoint_id();
            let expected_session_id = session_id.clone();
            let responder_session_id = session_id.clone();
            let responder_server_id = server_id.to_string();
            let credential_server_id = responder_server_id.clone();
            let accepting_server = server.clone();
            let accepting = tokio::spawn(async move {
                let channel = accepting_server.accept().await.unwrap();
                authenticate_responder(
                    channel,
                    &responder_server_id,
                    now_unix,
                    move |requested_session_id| {
                        assert_eq!(requested_session_id, responder_session_id);
                        Ok(InternetSessionCredentials {
                            session_id: responder_session_id,
                            advertised_endpoint_id: credential_server_id,
                            addresses: Vec::new(),
                            expires_at_unix,
                            role: InternetSessionRole::InvitationOwner,
                            secret: Zeroizing::new(session_secret),
                            manual_code: None,
                        })
                    },
                )
                .await
                .unwrap()
                .1
            });

            let channel = client
                .connect_verified(server.address(), server_id)
                .await
                .unwrap();
            let sender_session = authenticate_initiator(
                channel,
                &client_id.to_string(),
                InternetSessionCredentials {
                    session_id: expected_session_id,
                    advertised_endpoint_id: server_id.to_string(),
                    addresses: Vec::new(),
                    expires_at_unix,
                    role: InternetSessionRole::InvitationJoiner,
                    secret: Zeroizing::new(session_secret),
                    manual_code: None,
                },
                now_unix,
            )
            .await
            .unwrap();
            let receiver_session = accepting.await.unwrap();
            (server, client, receiver_session, sender_session)
        });

    let app_state = AppState::new(
        DeviceIdentity {
            device_id: "ipc-sender".to_owned(),
            display_name: "IPC sender".to_owned(),
            hostname: "ipc-sender.local".to_owned(),
            platform: "test".to_owned(),
        },
        root.join("app-state"),
    );
    app_state
        .remote_sessions
        .insert_invitation(
            session_id.clone(),
            server.endpoint_id().to_string(),
            Vec::new(),
            expires_at_unix,
            RemoteSessionSecret::new(session_secret),
            InternetSessionRole::InvitationJoiner,
            now_unix,
        )
        .unwrap();
    app_state
        .remote_sessions
        .begin_pairing(&session_id, now_unix)
        .unwrap();
    app_state
        .remote_sessions
        .mark_ready(&session_id, sender_session, now_unix)
        .unwrap();

    let receiver = tauri::async_runtime::spawn(async move {
        let (_channel, mut send, mut receive, mut noise) =
            receiver_session.next_transfer_parts().await.unwrap();
        let receipt = receive_transfer_with_accept(
            &mut send,
            &mut receive,
            &mut noise,
            &destination,
            |_| async { true },
            |_| {},
        )
        .await;
        receipt
    });

    let app = test::mock_builder()
        .manage(app_state)
        .invoke_handler(tauri::generate_handler![
            dukto_lib::commands::internet::send_to_internet_session,
            dukto_lib::commands::internet::disconnect_internet_session
        ])
        .build(test::mock_context(test::noop_assets()))
        .unwrap();
    let webview = WebviewWindowBuilder::new(&app, "main", Default::default())
        .build()
        .unwrap();

    let transfer_id = test::get_ipc_response(
        &webview,
        request(
            "send_to_internet_session",
            json!({
                "sessionId": session_id,
                "paths": [fixture.to_string_lossy()]
            }),
        ),
    )
    .unwrap()
    .deserialize::<String>()
    .unwrap();
    assert!(!transfer_id.is_empty());

    let receipt = tauri::async_runtime::block_on(async {
        tokio::time::timeout(std::time::Duration::from_secs(10), receiver)
            .await
            .unwrap()
            .unwrap()
            .unwrap()
    });
    assert_eq!(receipt.bytes_received, expected.len() as u64);
    assert_eq!(
        std::fs::read(root.join("destination/ipc-verified.bin")).unwrap(),
        expected
    );
    tauri::async_runtime::block_on(async {
        tokio::time::timeout(std::time::Duration::from_secs(5), async {
            while !matches!(
                app.state::<AppState>()
                    .remote_sessions
                    .view(&session_id)
                    .map(|view| view.status),
                Some(RemoteSessionStatus::Ready { .. })
            ) {
                tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap();
        let cancelled = test::get_ipc_response(
            &webview,
            request(
                "disconnect_internet_session",
                json!({ "sessionId": session_id }),
            ),
        );
        assert!(cancelled.unwrap().deserialize::<()>().is_ok());
        assert!(app
            .state::<AppState>()
            .remote_sessions
            .view(&session_id)
            .is_none());
        client.close().await;
        server.close().await;
    });

    drop(webview);
    drop(app);
    std::fs::remove_dir_all(root).unwrap();
}

fn request(command: &str, body: serde_json::Value) -> tauri::webview::InvokeRequest {
    tauri::webview::InvokeRequest {
        cmd: command.to_owned(),
        callback: tauri::ipc::CallbackFn(0),
        error: tauri::ipc::CallbackFn(1),
        url: "http://tauri.localhost".parse().unwrap(),
        body: body.into(),
        headers: Default::default(),
        invoke_key: test::INVOKE_KEY.to_owned(),
    }
}

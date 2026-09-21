use std::{
    path::PathBuf,
    str::FromStr,
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

use iroh::{EndpointAddr, EndpointId, RelayUrl, TransportAddr};
use serde::Serialize;
use tauri::{Emitter, Manager, State};
use zeroize::Zeroizing;

use crate::internet::endpoint::{InternetEndpoint, InternetEndpointConfig};
use crate::internet::invite::InternetInvite;
use crate::internet::pairing::ManualPairingCode;
use crate::internet::rendezvous::{
    HttpRendezvousTransport, RendezvousClient, RendezvousPayload, RendezvousSlot,
};
use crate::internet::session::{
    authenticate_initiator, authenticate_responder, InternetSessionError, InternetSessionRole,
};
use crate::internet::share::{create_share_artifacts, parse_deep_link};
use crate::state::app_state::AppState;
use crate::state::internet_session::{
    RemoteSessionRegistry, RemoteSessionSecret, RemoteSessionStatus,
};
use crate::transfer::channel::{AuthenticatedChannel, ChannelCloseReason};
use crate::transfer::receiver::is_transfer_rejected;
use crate::transfer::sender::{send_transfer_with_cancellation, CancellableSendCallbacks};
use crate::transfer::server::TransferServer;

const INVITE_TTL_SECS: u64 = 5 * 60;
const MAX_RENDEZVOUS_ENVELOPE_BYTES: usize = 16 * 1024;

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct InternetInviteView {
    pub can_send: bool,
    pub session_id: String,
    pub peer_id: String,
    pub peer: Option<crate::state::device::DeviceIdentity>,
    pub expires_at_unix: u64,
    pub status: RemoteSessionStatus,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct InternetInviteShareView {
    #[serde(flatten)]
    pub invite: InternetInviteView,
    pub qr_svg_data_url: String,
    pub manual_code: String,
}

#[derive(Serialize)]
struct SendStarted {
    transfer_id: String,
    peer_device_id: String,
}

#[derive(Serialize)]
struct SendAccepted {
    transfer_id: String,
}

#[derive(Serialize)]
struct SendComplete {
    transfer_id: String,
    bytes_sent: u64,
}

#[derive(Serialize)]
struct SendError {
    transfer_id: String,
    error: String,
}

fn with_status(view: &InternetInviteView, status: RemoteSessionStatus) -> InternetInviteView {
    InternetInviteView {
        can_send: view.can_send,
        session_id: view.session_id.clone(),
        peer_id: view.peer_id.clone(),
        peer: view.peer.clone(),
        expires_at_unix: view.expires_at_unix,
        status,
    }
}

fn view_for(invite: &InternetInvite, role: InternetSessionRole) -> InternetInviteView {
    InternetInviteView {
        can_send: role == InternetSessionRole::InvitationJoiner,
        session_id: invite.session_id().to_owned(),
        peer_id: invite.endpoint_id().to_owned(),
        peer: None,
        expires_at_unix: invite.expires_at_unix(),
        status: RemoteSessionStatus::Invited,
    }
}

pub(crate) fn create_invite_in_registry(
    registry: &RemoteSessionRegistry,
    endpoint_id: &str,
    addresses: Vec<String>,
    now_unix: u64,
) -> Result<(InternetInviteView, InternetInvite), String> {
    let slot = RendezvousSlot::generate()
        .map_err(|_| "could not create internet invitation".to_owned())?;
    let invite = InternetInvite::new(
        endpoint_id,
        addresses,
        Some(slot.expose().to_owned()),
        now_unix,
        INVITE_TTL_SECS,
    )
    .map_err(|_| "could not create internet invitation".to_owned())?;
    registry
        .insert_invitation(
            invite.session_id(),
            invite.endpoint_id(),
            invite.addresses().to_vec(),
            invite.expires_at_unix(),
            RemoteSessionSecret::new(invite.secret_copy_for_registry()),
            InternetSessionRole::InvitationOwner,
            now_unix,
        )
        .map_err(|_| "could not register internet invitation".to_owned())?;
    Ok((
        view_for(&invite, InternetSessionRole::InvitationOwner),
        invite,
    ))
}

#[cfg(test)]
pub(crate) fn import_invite_in_registry(
    registry: &RemoteSessionRegistry,
    encoded: String,
    now_unix: u64,
) -> Result<InternetInviteView, String> {
    let invite = decode_invite(encoded, now_unix)?;
    register_imported_invite(registry, &invite, invite.addresses().to_vec(), now_unix)
}

fn decode_invite(encoded: String, now_unix: u64) -> Result<InternetInvite, String> {
    let encoded = Zeroizing::new(encoded);
    if encoded.starts_with("dukto://") {
        parse_deep_link(encoded.as_str(), now_unix)
            .map_err(|_| "internet invitation is invalid or expired".to_owned())
    } else {
        InternetInvite::from_canonical_json(encoded.as_str(), now_unix)
            .map_err(|_| "internet invitation is invalid or expired".to_owned())
    }
}

fn register_imported_invite(
    registry: &RemoteSessionRegistry,
    invite: &InternetInvite,
    addresses: Vec<String>,
    now_unix: u64,
) -> Result<InternetInviteView, String> {
    registry
        .insert_invitation(
            invite.session_id(),
            invite.endpoint_id(),
            addresses,
            invite.expires_at_unix(),
            RemoteSessionSecret::new(invite.secret_copy_for_registry()),
            InternetSessionRole::InvitationJoiner,
            now_unix,
        )
        .map_err(|_| "could not register internet invitation".to_owned())?;
    Ok(view_for(invite, InternetSessionRole::InvitationJoiner))
}

fn create_share_view(
    registry: &RemoteSessionRegistry,
    invite: InternetInviteView,
    invitation: &InternetInvite,
) -> Result<InternetInviteShareView, String> {
    let artifacts = create_share_artifacts(invitation)
        .map_err(|_| "could not prepare internet invitation sharing".to_owned())?;
    registry
        .set_manual_code(&invite.session_id, artifacts.manual_code.clone())
        .map_err(|_| "could not register internet invitation".to_owned())?;
    registry
        .set_share_link(&invite.session_id, artifacts.deep_link)
        .map_err(|_| "could not register internet invitation".to_owned())?;
    Ok(InternetInviteShareView {
        invite,
        qr_svg_data_url: artifacts.qr_svg_data_url,
        manual_code: artifacts.manual_code,
    })
}

#[tauri::command]
pub fn get_internet_invitation_link(
    state: State<'_, AppState>,
    session_id: String,
) -> Result<String, String> {
    state
        .remote_sessions
        .with_share_link(&session_id, unix_now()?, str::to_owned)
        .map_err(|_| "internet invitation link is unavailable".to_owned())
}

pub(crate) fn cancel_invite_in_registry(
    registry: &RemoteSessionRegistry,
    session_id: &str,
) -> Result<(), String> {
    if registry.cancel(session_id) {
        Ok(())
    } else {
        Err("internet invitation was not found".to_owned())
    }
}

pub(crate) fn confirm_pairing_code_in_registry(
    registry: &RemoteSessionRegistry,
    session_id: &str,
    code: String,
    now_unix: u64,
) -> Result<InternetInviteView, String> {
    let code = Zeroizing::new(code);
    ManualPairingCode::parse(code.as_str())
        .map_err(|_| "manual pairing code is invalid".to_owned())?;
    registry
        .transition(session_id, RemoteSessionStatus::Pairing, now_unix)
        .map_err(|_| "internet session cannot start pairing".to_owned())?;
    registry
        .set_manual_code(session_id, code.to_string())
        .map_err(|_| "internet session cannot start pairing".to_owned())?;
    let view = registry
        .view(session_id)
        .ok_or_else(|| "internet session was not found".to_owned())?;
    Ok(InternetInviteView {
        can_send: view.can_send,
        session_id: view.session_id,
        peer_id: view.peer_id,
        peer: view.peer,
        expires_at_unix: view.expires_at_unix,
        status: view.status,
    })
}

fn unix_now() -> Result<u64, String> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .map_err(|_| "system clock is invalid".to_owned())
}

async fn endpoint_for(state: &AppState) -> Result<&InternetEndpoint, String> {
    state
        .internet_endpoint
        .get_or_try_init(|| async {
            let config = InternetEndpointConfig::from_environment()?
                .with_secret_key(state.internet_secret_key.clone());
            InternetEndpoint::bind(config).await
        })
        .await
        .map_err(|error| error.to_string())
}

fn rendezvous_transport() -> Result<Option<HttpRendezvousTransport>, String> {
    HttpRendezvousTransport::configured_from_environment().map_err(|error| error.to_string())
}

fn endpoint_address(
    credentials: &crate::internet::session::InternetSessionCredentials,
) -> Result<(EndpointAddr, EndpointId), String> {
    let endpoint_id = EndpointId::from_str(&credentials.advertised_endpoint_id)
        .map_err(|_| "internet invitation endpoint is invalid".to_owned())?;
    let mut addresses = Vec::with_capacity(credentials.addresses.len());
    for address in &credentials.addresses {
        let transport = if let Some(address) = address.strip_prefix("ip:") {
            TransportAddr::Ip(
                address
                    .parse()
                    .map_err(|_| "internet invitation route is invalid".to_owned())?,
            )
        } else if let Some(address) = address.strip_prefix("relay:") {
            TransportAddr::Relay(
                RelayUrl::from_str(address)
                    .map_err(|_| "internet invitation route is invalid".to_owned())?,
            )
        } else {
            return Err("internet invitation route is invalid".to_owned());
        };
        addresses.push(transport);
    }
    if addresses.is_empty() {
        return Err("internet invitation has no usable route".to_owned());
    }
    Ok((
        EndpointAddr::from_parts(endpoint_id, addresses),
        endpoint_id,
    ))
}

async fn connect_internet_session_in_registry(
    registry: &RemoteSessionRegistry,
    endpoint: &InternetEndpoint,
    local_device: &crate::state::device::DeviceIdentity,
    session_id: &str,
    now_unix: u64,
) -> Result<InternetInviteView, String> {
    registry
        .begin_pairing(session_id, now_unix)
        .map_err(|_| "internet session cannot start pairing".to_owned())?;
    let credentials = registry
        .credentials(session_id, InternetSessionRole::InvitationJoiner, now_unix)
        .map_err(|_| "internet session cannot load pairing data".to_owned())?;
    let (address, expected_endpoint) = endpoint_address(&credentials)?;
    let channel = endpoint
        .connect_verified(address, expected_endpoint)
        .await
        .map_err(|_| "internet session could not connect".to_owned())?;
    let established = authenticate_initiator(
        channel,
        &endpoint.endpoint_id().to_string(),
        credentials,
        now_unix,
    )
    .await
    .map_err(|_| "internet session authentication failed".to_owned())?;
    established
        .exchange_peer_identity(local_device)
        .await
        .map_err(|_| "internet peer identity exchange failed".to_owned())?;
    registry
        .mark_ready(session_id, established, now_unix)
        .map(InternetInviteView::from)
        .map_err(|_| "internet session could not become ready".to_owned())
}

async fn authenticate_incoming_channel(
    registry: &RemoteSessionRegistry,
    endpoint_id: &str,
    local_device: &crate::state::device::DeviceIdentity,
    channel: crate::internet::channel::IrohAuthenticatedChannel,
    now_unix: u64,
) -> Result<InternetInviteView, String> {
    let (session_id, established) =
        authenticate_responder(channel, endpoint_id, now_unix, |session_id| {
            registry
                .begin_pairing(session_id, now_unix)
                .map_err(|_| InternetSessionError::AuthenticationFailed)?;
            registry
                .credentials(session_id, InternetSessionRole::InvitationOwner, now_unix)
                .map_err(|_| InternetSessionError::AuthenticationFailed)
        })
        .await
        .map_err(|_| "internet session authentication failed".to_owned())?;
    established
        .exchange_peer_identity(local_device)
        .await
        .map_err(|_| "internet peer identity exchange failed".to_owned())?;
    registry
        .mark_ready(&session_id, established, now_unix)
        .map(InternetInviteView::from)
        .map_err(|_| "internet session could not become ready".to_owned())
}

#[cfg(test)]
async fn accept_one_internet_session(
    registry: &RemoteSessionRegistry,
    endpoint: &InternetEndpoint,
    local_device: &crate::state::device::DeviceIdentity,
    now_unix: u64,
) -> Result<InternetInviteView, String> {
    let channel = endpoint
        .accept()
        .await
        .map_err(|_| "internet session could not accept a connection".to_owned())?;
    authenticate_incoming_channel(
        registry,
        &endpoint.endpoint_id().to_string(),
        local_device,
        channel,
        now_unix,
    )
    .await
}

async fn receive_ready_internet_session(
    app_handle: tauri::AppHandle,
    registry: Arc<RemoteSessionRegistry>,
    ready: InternetInviteView,
) -> Result<(), String> {
    let established = registry
        .established(&ready.session_id)
        .map_err(|_| "internet session transfer channel is unavailable".to_owned())?;
    let route = established.route_kind();
    let server = app_handle.state::<Arc<TransferServer>>().inner().clone();
    loop {
        let transferring = with_status(&ready, RemoteSessionStatus::Transferring { route });
        let transfer_registry = Arc::clone(&registry);
        let transfer_session_id = ready.session_id.clone();
        let transfer_handle = app_handle.clone();
        let result = server
            .receive_internet_session(app_handle.clone(), established.clone(), move || {
                let Ok(now_unix) = unix_now() else {
                    return false;
                };
                if transfer_registry
                    .transition(
                        &transfer_session_id,
                        RemoteSessionStatus::Transferring { route },
                        now_unix,
                    )
                    .is_err()
                {
                    return false;
                }
                let _ = transfer_handle.emit("internet:session-updated", &transferring);
                true
            })
            .await;

        if transfer_keeps_session_ready(&result, false) {
            if registry
                .transition(
                    &ready.session_id,
                    RemoteSessionStatus::Ready { route },
                    unix_now()?,
                )
                .is_err()
            {
                return Ok(());
            }
            let _ = app_handle.emit(
                "internet:session-updated",
                with_status(&ready, RemoteSessionStatus::Ready { route }),
            );
            continue;
        }

        established.close(ChannelCloseReason::ProtocolError);
        if registry
            .transition(&ready.session_id, RemoteSessionStatus::Failed, unix_now()?)
            .is_ok()
        {
            let _ = app_handle.emit(
                "internet:session-updated",
                with_status(&ready, RemoteSessionStatus::Failed),
            );
        }
        return result.map_err(|error| error.to_string());
    }
}

fn transfer_keeps_session_ready<T>(
    result: &Result<T, Box<dyn std::error::Error + Send + Sync>>,
    locally_cancelled: bool,
) -> bool {
    result.is_ok()
        || locally_cancelled
        || result.as_ref().is_err_and(|error| {
            error.is::<crate::crypto::noise::TransferCancelled>()
                || crate::crypto::noise::is_remote_transfer_cancellation(error.as_ref())
                || is_transfer_rejected(error.as_ref())
        })
}

fn monitor_internet_session<R: tauri::Runtime>(
    app_handle: tauri::AppHandle<R>,
    registry: Arc<RemoteSessionRegistry>,
    ready: InternetInviteView,
) {
    let Ok(established) = registry.established(&ready.session_id) else {
        return;
    };
    tauri::async_runtime::spawn(async move {
        established.closed().await;
        if registry
            .transition(
                &ready.session_id,
                RemoteSessionStatus::Failed,
                unix_now().unwrap_or(0),
            )
            .is_ok()
        {
            let _ = app_handle.emit(
                "internet:session-updated",
                with_status(&ready, RemoteSessionStatus::Failed),
            );
        }
    });
}

fn ensure_accept_loop(
    app_handle: tauri::AppHandle,
    registry: Arc<RemoteSessionRegistry>,
    endpoint: InternetEndpoint,
) {
    if !registry.start_accept_loop() {
        return;
    }
    tauri::async_runtime::spawn(async move {
        loop {
            let channel = match endpoint.accept().await {
                Ok(channel) => channel,
                Err(_) => break,
            };
            let handle = app_handle.clone();
            let registry = Arc::clone(&registry);
            let endpoint_id = endpoint.endpoint_id().to_string();
            tauri::async_runtime::spawn(async move {
                let result = unix_now().map_err(|_| "internet session clock is invalid".to_owned());
                let result = match result {
                    Ok(now_unix) => {
                        let local_device = handle.state::<AppState>().get_device();
                        authenticate_incoming_channel(
                            &registry,
                            &endpoint_id,
                            &local_device,
                            channel,
                            now_unix,
                        )
                        .await
                    }
                    Err(error) => Err(error),
                };
                match result {
                    Ok(view) => {
                        let _ = handle.emit("internet:session-updated", &view);
                        monitor_internet_session(
                            handle.clone(),
                            Arc::clone(&registry),
                            view.clone(),
                        );
                        if let Err(error) =
                            receive_ready_internet_session(handle, Arc::clone(&registry), view)
                                .await
                        {
                            tracing::warn!(%error, "Incoming internet transfer failed");
                        }
                    }
                    Err(error) => {
                        tracing::warn!(%error, "Incoming internet session failed");
                    }
                }
            });
        }
    });
}

impl From<crate::state::internet_session::RemoteSessionView> for InternetInviteView {
    fn from(view: crate::state::internet_session::RemoteSessionView) -> Self {
        Self {
            can_send: view.can_send,
            session_id: view.session_id,
            peer_id: view.peer_id,
            peer: view.peer,
            expires_at_unix: view.expires_at_unix,
            status: view.status,
        }
    }
}

#[tauri::command]
pub async fn create_internet_invite(
    app_handle: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<InternetInviteShareView, String> {
    let address = if let Some(endpoint) = state.internet_endpoint.get() {
        endpoint.address()
    } else {
        let config =
            InternetEndpointConfig::from_environment().map_err(|error| error.to_string())?;
        if config.relay_enabled() {
            config.bootstrap_address(state.internet_secret_key.public())
        } else {
            endpoint_for(&state).await?.address()
        }
    };
    let addresses = address.addrs.iter().map(ToString::to_string).collect();
    let now_unix = unix_now()?;
    let (view, invite) = create_invite_in_registry(
        &state.remote_sessions,
        &address.id.to_string(),
        addresses,
        now_unix,
    )?;
    let prepared = async {
        if let Some(transport) = rendezvous_transport()? {
            let payload = RendezvousPayload::generate(
                address.id.to_string(),
                address.addrs.iter().map(ToString::to_string).collect(),
            )
            .map_err(|error| error.to_string())?;
            let client = RendezvousClient::new(&transport, MAX_RENDEZVOUS_ENVELOPE_BYTES)
                .map_err(|error| error.to_string())?;
            client
                .publish(&invite, &payload, now_unix)
                .await
                .map_err(|error| error.to_string())?;
        }
        let endpoint = endpoint_for(&state).await?;
        if endpoint.relay_enabled() {
            endpoint
                .wait_until_online()
                .await
                .map_err(|error| error.to_string())?;
        }
        Ok::<_, String>((
            create_share_view(&state.remote_sessions, view, &invite)?,
            endpoint.clone(),
        ))
    }
    .await;
    let (share, endpoint) = match prepared {
        Ok(prepared) => prepared,
        Err(error) => {
            state.remote_sessions.cancel(invite.session_id());
            return Err(error);
        }
    };
    ensure_accept_loop(app_handle, Arc::clone(&state.remote_sessions), endpoint);
    Ok(share)
}

#[tauri::command]
pub async fn import_internet_invite<R: tauri::Runtime>(
    app_handle: tauri::AppHandle<R>,
    state: State<'_, AppState>,
    invitation: String,
) -> Result<InternetInviteView, String> {
    let connect_immediately = invitation.starts_with("dukto://connect#");
    let now_unix = unix_now()?;
    let invite = decode_invite(invitation, now_unix)?;
    let transport = rendezvous_transport()?;
    let addresses = if let Some(transport) = transport {
        let client = RendezvousClient::new(&transport, MAX_RENDEZVOUS_ENVELOPE_BYTES)
            .map_err(|error| error.to_string())?;
        let payload = client
            .consume(
                &invite,
                &state
                    .internet_endpoint
                    .get()
                    .map(InternetEndpoint::endpoint_id)
                    .unwrap_or_else(|| state.internet_secret_key.public())
                    .to_string(),
                now_unix,
            )
            .await
            .map_err(|error| error.to_string())?;
        if payload.endpoint_id() != invite.endpoint_id() {
            return Err("rendezvous endpoint does not match the invitation".to_owned());
        }
        payload.addresses().to_vec()
    } else {
        invite.addresses().to_vec()
    };
    let view = register_imported_invite(&state.remote_sessions, &invite, addresses, now_unix)?;
    if !connect_immediately {
        return Ok(view);
    }
    let endpoint = endpoint_for(&state).await?;
    if endpoint.relay_enabled() {
        endpoint
            .wait_until_online()
            .await
            .map_err(|error| error.to_string())?;
    }
    let local_device = state.get_device();
    let result = connect_internet_session_in_registry(
        &state.remote_sessions,
        endpoint,
        &local_device,
        &view.session_id,
        now_unix,
    )
    .await;
    if result.is_err() {
        state.remote_sessions.cancel(&view.session_id);
    }
    if let Ok(ready) = &result {
        monitor_internet_session(
            app_handle,
            Arc::clone(&state.remote_sessions),
            ready.clone(),
        );
    }
    result
}

#[tauri::command]
pub async fn send_to_internet_session<R: tauri::Runtime>(
    app_handle: tauri::AppHandle<R>,
    state: State<'_, AppState>,
    session_id: String,
    paths: Vec<String>,
) -> Result<String, String> {
    if paths.is_empty() {
        return Err("select at least one file or directory to send".to_owned());
    }
    let ready = state
        .remote_sessions
        .view(&session_id)
        .map(InternetInviteView::from)
        .ok_or_else(|| "internet session was not found".to_owned())?;
    if !ready.can_send {
        return Err("internet invitation owner is receive-only".to_owned());
    }
    let route = match ready.status {
        RemoteSessionStatus::Ready { route } => route,
        _ => return Err("internet session is not ready to send".to_owned()),
    };
    let transfer_id = uuid::Uuid::new_v4().to_string();
    let control = state
        .transfer_registry
        .register(transfer_id.clone())
        .await?;
    let established = match state.remote_sessions.established(&session_id) {
        Ok(established) => established,
        Err(_) => {
            state.transfer_registry.remove(&transfer_id, &control).await;
            return Err("internet session transfer channel is unavailable".to_owned());
        }
    };
    if state
        .remote_sessions
        .transition(
            &session_id,
            RemoteSessionStatus::Transferring { route },
            unix_now()?,
        )
        .is_err()
    {
        state.transfer_registry.remove(&transfer_id, &control).await;
        return Err("internet session could not start sending".to_owned());
    }

    let (channel, mut send, mut receive, mut noise) = match established.next_transfer_parts().await
    {
        Ok(parts) => parts,
        Err(_) => {
            state.transfer_registry.remove(&transfer_id, &control).await;
            established.close(ChannelCloseReason::ProtocolError);
            let _ = state.remote_sessions.transition(
                &session_id,
                RemoteSessionStatus::Failed,
                unix_now().unwrap_or(0),
            );
            let _ = app_handle.emit(
                "internet:session-updated",
                with_status(&ready, RemoteSessionStatus::Failed),
            );
            return Err("internet session transfer channel is unavailable".to_owned());
        }
    };

    let transferring = with_status(&ready, RemoteSessionStatus::Transferring { route });
    let _ = app_handle.emit("internet:session-updated", &transferring);
    let _ = app_handle.emit(
        "transfer:send-started",
        &SendStarted {
            transfer_id: transfer_id.clone(),
            peer_device_id: ready
                .peer
                .as_ref()
                .map(|peer| peer.device_id.clone())
                .unwrap_or_else(|| ready.peer_id.clone()),
        },
    );

    let input_paths: Vec<PathBuf> = paths.iter().map(PathBuf::from).collect();
    #[cfg(any(target_os = "android", target_os = "ios"))]
    let selected_paths = paths;
    let sender = state.get_device();
    let transfer_registry = Arc::clone(&state.transfer_registry);
    let remote_sessions = Arc::clone(&state.remote_sessions);
    let handle = app_handle.clone();
    let tid = transfer_id.clone();
    tauri::async_runtime::spawn(async move {
        let progress_handle = handle.clone();
        let accepted_handle = handle.clone();
        let accepted_id = tid.clone();
        let result = send_transfer_with_cancellation(
            &mut send,
            &mut receive,
            &mut noise,
            &input_paths,
            &tid,
            &sender,
            CancellableSendCallbacks {
                cancelled: control.cancelled(),
                on_progress: move |progress: &crate::protocol::types::TransferProgress| {
                    let _ = progress_handle.emit("transfer:progress", progress);
                },
                on_accepted: move || {
                    let _ = accepted_handle.emit(
                        "transfer:send-accepted",
                        &SendAccepted {
                            transfer_id: accepted_id,
                        },
                    );
                },
            },
        )
        .await;

        transfer_registry.remove(&tid, &control).await;
        let reusable = transfer_keeps_session_ready(&result, control.is_cancelled());
        let next_status = if reusable {
            RemoteSessionStatus::Ready { route }
        } else {
            channel.close(ChannelCloseReason::ProtocolError);
            RemoteSessionStatus::Failed
        };
        if remote_sessions
            .transition(&session_id, next_status.clone(), unix_now().unwrap_or(0))
            .is_ok()
        {
            let _ = handle.emit("internet:session-updated", with_status(&ready, next_status));
        }

        #[cfg(target_os = "android")]
        if let Err(error) = super::release_android_selected_files(&handle, &selected_paths).await {
            tracing::warn!(%error, transfer_id = %tid, "Could not release selected Android files");
        }
        #[cfg(target_os = "ios")]
        if let Err(error) = super::release_ios_selected_files(&selected_paths).await {
            tracing::warn!(%error, transfer_id = %tid, "Could not release selected iOS files");
        }

        if control.is_cancelled() {
            return;
        }
        match result {
            Ok(bytes_sent) => {
                let _ = handle.emit(
                    "transfer:send-complete",
                    &SendComplete {
                        transfer_id: tid,
                        bytes_sent,
                    },
                );
            }
            Err(error) => {
                tracing::error!(transfer_id = %tid, %error, "Internet transfer send failed");
                let _ = handle.emit(
                    "transfer:send-error",
                    &SendError {
                        transfer_id: tid,
                        error: error.to_string(),
                    },
                );
            }
        }
    });

    Ok(transfer_id)
}

#[tauri::command]
pub fn cancel_internet_invite(
    state: State<'_, AppState>,
    session_id: String,
) -> Result<(), String> {
    cancel_invite_in_registry(&state.remote_sessions, &session_id)
}

#[tauri::command]
pub fn disconnect_internet_session(
    state: State<'_, AppState>,
    session_id: String,
) -> Result<(), String> {
    cancel_invite_in_registry(&state.remote_sessions, &session_id)
}

#[tauri::command]
pub async fn confirm_internet_pairing_code<R: tauri::Runtime>(
    app_handle: tauri::AppHandle<R>,
    state: State<'_, AppState>,
    session_id: String,
    code: String,
) -> Result<InternetInviteView, String> {
    let now_unix = unix_now()?;
    confirm_pairing_code_in_registry(&state.remote_sessions, &session_id, code, now_unix)?;
    let endpoint = endpoint_for(&state).await?;
    let local_device = state.get_device();
    let result = connect_internet_session_in_registry(
        &state.remote_sessions,
        endpoint,
        &local_device,
        &session_id,
        now_unix,
    )
    .await;
    if result.is_err() {
        state.remote_sessions.cancel(&session_id);
    }
    if let Ok(ready) = &result {
        monitor_internet_session(
            app_handle,
            Arc::clone(&state.remote_sessions),
            ready.clone(),
        );
    }
    result
}

#[cfg(test)]
mod tests {
    use std::{path::PathBuf, sync::Arc, time::Duration};

    use crate::crypto::noise::TransferCancelled;
    use crate::internet::endpoint::{InternetEndpoint, InternetEndpointConfig};
    use crate::internet::pairing::{
        ManualCodePairing, ManualPairingCode, PairingOrigin, PairingRole, PairingTranscript,
    };
    use crate::state::device::DeviceIdentity;
    use crate::state::internet_session::{RemoteSessionRegistry, RemoteSessionStatus};
    use crate::transfer::{
        receiver::{receive_transfer, receive_transfer_with_cancellation, TransferRejected},
        sender::{send_transfer, send_transfer_with_cancellation, CancellableSendCallbacks},
    };

    use super::{
        accept_one_internet_session, cancel_invite_in_registry, confirm_pairing_code_in_registry,
        connect_internet_session_in_registry, create_invite_in_registry, create_share_view,
        import_invite_in_registry, transfer_keeps_session_ready,
    };

    const NOW: u64 = 1_800_000_000;

    #[test]
    fn only_per_transfer_outcomes_keep_the_authenticated_session_ready() {
        // Mutation captured: classifying rejection or cancellation as a connection failure
        // removes a healthy peer, while treating arbitrary I/O failures as reusable hides loss.
        let success: Result<(), Box<dyn std::error::Error + Send + Sync>> = Ok(());
        let cancelled: Result<(), Box<dyn std::error::Error + Send + Sync>> =
            Err(Box::new(TransferCancelled));
        let rejected: Result<(), Box<dyn std::error::Error + Send + Sync>> =
            Err(Box::new(TransferRejected));
        let failed: Result<(), Box<dyn std::error::Error + Send + Sync>> =
            Err(std::io::Error::other("connection lost").into());

        assert!(transfer_keeps_session_ready(&success, false));
        assert!(transfer_keeps_session_ready(&cancelled, false));
        assert!(transfer_keeps_session_ready(&rejected, false));
        assert!(transfer_keeps_session_ready(&failed, true));
        assert!(!transfer_keeps_session_ready(&failed, false));
    }

    #[test]
    fn create_import_and_cancel_return_only_redacted_views() {
        // Mutation captured: treating InvitationOwner as a sender fails this role projection.
        let creator_registry = RemoteSessionRegistry::default();
        let (view, invite) = create_invite_in_registry(
            &creator_registry,
            "creator-endpoint",
            vec!["ip:203.0.113.10:4242".to_owned()],
            NOW,
        )
        .unwrap();
        let encoded = invite.to_canonical_json().unwrap();
        let secret_hex = invite
            .secret_copy_for_registry()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        let view_json = serde_json::to_string(&view).unwrap();
        assert!(!view.can_send);
        assert!(!creator_registry.view(&view.session_id).unwrap().can_send);
        assert!(!view_json.contains(&secret_hex));
        assert!(!view_json.contains("private"));

        let importer_registry = RemoteSessionRegistry::default();
        let imported =
            import_invite_in_registry(&importer_registry, encoded.clone(), NOW + 1).unwrap();
        assert!(imported.can_send);
        assert!(
            importer_registry
                .view(&imported.session_id)
                .unwrap()
                .can_send
        );
        assert_eq!(imported.session_id, view.session_id);
        assert!(!serde_json::to_string(&imported)
            .unwrap()
            .contains(&secret_hex));

        cancel_invite_in_registry(&importer_registry, &imported.session_id).unwrap();
        assert!(importer_registry.view(&imported.session_id).is_none());
    }

    #[test]
    fn qr_deep_link_import_is_single_use_and_share_view_has_no_plaintext_bearer() {
        let creator_registry = RemoteSessionRegistry::default();
        let (view, invite) = create_invite_in_registry(
            &creator_registry,
            "creator-endpoint",
            vec!["ip:203.0.113.10:4242".to_owned()],
            NOW,
        )
        .unwrap();
        let canonical = invite.to_canonical_json().unwrap();
        let secret_hex = invite
            .secret_copy_for_registry()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        let share = create_share_view(&creator_registry, view, &invite).unwrap();
        let link = crate::internet::share::create_share_artifacts(&invite)
            .unwrap()
            .deep_link_for_test()
            .to_owned();
        let serialized = serde_json::to_string(&share).unwrap();
        assert!(!serialized.contains(&canonical));
        assert!(!serialized.contains(&secret_hex));
        assert_eq!(
            creator_registry.with_manual_code(&share.invite.session_id, str::to_owned),
            Some(share.manual_code.clone())
        );
        assert_eq!(
            creator_registry
                .with_share_link(&share.invite.session_id, NOW + 1, str::to_owned)
                .unwrap(),
            link
        );

        let code = creator_registry
            .with_manual_code(&share.invite.session_id, |code| {
                ManualPairingCode::parse(code).unwrap()
            })
            .unwrap();
        let transcript = PairingTranscript::new(
            PairingOrigin::ManualCode,
            &share.invite.session_id,
            "creator-endpoint",
            "remote-endpoint",
            [0x11; 32],
            [0x22; 32],
            NOW + 300,
            NOW,
        )
        .unwrap();
        let (mut initiator, initiator_message) =
            ManualCodePairing::start(PairingRole::Initiator, &code, &transcript).unwrap();
        let (mut responder, responder_message) =
            ManualCodePairing::start(PairingRole::Responder, &code, &transcript).unwrap();
        let initiator_keys = initiator.finish(&responder_message).unwrap();
        let responder_keys = responder.finish(&initiator_message).unwrap();
        assert_eq!(initiator_keys, responder_keys);

        let prefix = "data:image/svg+xml;base64,";
        let encoded_svg = share.qr_svg_data_url.strip_prefix(prefix).unwrap();
        let svg = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, encoded_svg)
            .unwrap();
        assert!(!String::from_utf8(svg).unwrap().contains(&canonical));

        let importer_registry = RemoteSessionRegistry::default();
        import_invite_in_registry(&importer_registry, link.clone(), NOW + 1).unwrap();
        assert!(import_invite_in_registry(&importer_registry, link, NOW + 1).is_err());
    }

    #[test]
    fn malformed_input_errors_never_echo_secret_material() {
        let registry = RemoteSessionRegistry::default();
        let sentinel = "super-secret-invite-material";
        let error =
            import_invite_in_registry(&registry, format!("{{\"secret\":\"{sentinel}\"}}"), NOW)
                .unwrap_err();

        assert!(!error.contains(sentinel));
    }

    #[test]
    fn imported_session_accepts_only_a_valid_manual_pake_code() {
        let creator_registry = RemoteSessionRegistry::default();
        let (_, invite) = create_invite_in_registry(
            &creator_registry,
            "creator-endpoint",
            vec!["ip:203.0.113.10:4242".to_owned()],
            NOW,
        )
        .unwrap();
        let importer_registry = RemoteSessionRegistry::default();
        let imported = import_invite_in_registry(
            &importer_registry,
            invite.to_canonical_json().unwrap(),
            NOW + 1,
        )
        .unwrap();

        assert!(confirm_pairing_code_in_registry(
            &importer_registry,
            &imported.session_id,
            "not-secret".to_owned(),
            NOW + 2,
        )
        .is_err());
        let pairing = confirm_pairing_code_in_registry(
            &importer_registry,
            &imported.session_id,
            "31415926".to_owned(),
            NOW + 2,
        )
        .unwrap();
        assert_eq!(pairing.status, RemoteSessionStatus::Pairing);
    }

    #[tokio::test]
    async fn production_session_command_path_reaches_ready_only_after_bound_authentication() {
        let owner_endpoint = loopback_endpoint().await;
        let joiner_endpoint = loopback_endpoint().await;
        let owner_registry = Arc::new(RemoteSessionRegistry::default());
        let joiner_registry = Arc::new(RemoteSessionRegistry::default());
        let owner_device = test_device("owner");
        let joiner_device = test_device("joiner");
        let addresses = owner_endpoint
            .address()
            .addrs
            .iter()
            .map(ToString::to_string)
            .collect();
        let (owner_view, invite) = create_invite_in_registry(
            &owner_registry,
            &owner_endpoint.endpoint_id().to_string(),
            addresses,
            NOW,
        )
        .unwrap();
        let imported = import_invite_in_registry(
            &joiner_registry,
            invite.to_canonical_json().unwrap(),
            NOW + 1,
        )
        .unwrap();

        let accepting_registry = Arc::clone(&owner_registry);
        let accepting_endpoint = owner_endpoint.clone();
        let accepting_device = owner_device.clone();
        let accepting = tokio::spawn(async move {
            accept_one_internet_session(
                &accepting_registry,
                &accepting_endpoint,
                &accepting_device,
                NOW + 2,
            )
            .await
        });
        let joiner_view = connect_internet_session_in_registry(
            &joiner_registry,
            &joiner_endpoint,
            &joiner_device,
            &imported.session_id,
            NOW + 2,
        )
        .await
        .unwrap();
        let owner_ready = accepting.await.unwrap().unwrap();

        assert_eq!(
            joiner_view.status,
            RemoteSessionStatus::Ready {
                route: crate::transfer::channel::RouteKind::Direct
            }
        );
        assert_eq!(owner_ready.status, joiner_view.status);
        assert_eq!(joiner_view.peer, Some(owner_device));
        assert_eq!(owner_ready.peer, Some(joiner_device));
        assert_eq!(
            joiner_view.peer_id,
            owner_endpoint.endpoint_id().to_string()
        );
        assert_eq!(
            owner_ready.peer_id,
            joiner_endpoint.endpoint_id().to_string()
        );
        assert_eq!(owner_ready.session_id, owner_view.session_id);
        assert!(joiner_registry.established(&joiner_view.session_id).is_ok());
        assert!(owner_registry.established(&owner_ready.session_id).is_ok());
        assert!(joiner_registry
            .with_secret(&joiner_view.session_id, |_| ())
            .is_none());
        assert!(owner_registry
            .with_secret(&owner_ready.session_id, |_| ())
            .is_none());

        owner_endpoint.close().await;
        joiner_endpoint.close().await;
    }

    #[tokio::test]
    async fn production_session_path_transfers_verified_bytes_on_direct_and_relay() {
        let owner_endpoint = loopback_endpoint().await;
        let joiner_endpoint = loopback_endpoint().await;
        run_production_transfer_case(
            owner_endpoint,
            joiner_endpoint,
            crate::transfer::channel::RouteKind::Direct,
        )
        .await;

        let (relay_map, _relay_url, relay_server) = iroh::test_utils::run_relay_server()
            .await
            .expect("test relay");
        let owner_endpoint = relay_only_endpoint(relay_map.clone()).await;
        let joiner_endpoint = relay_only_endpoint(relay_map).await;
        owner_endpoint.wait_until_online().await.unwrap();
        joiner_endpoint.wait_until_online().await.unwrap();
        run_production_transfer_case(
            owner_endpoint,
            joiner_endpoint,
            crate::transfer::channel::RouteKind::Relay,
        )
        .await;
        drop(relay_server);
    }

    async fn run_production_transfer_case(
        owner_endpoint: InternetEndpoint,
        joiner_endpoint: InternetEndpoint,
        expected_route: crate::transfer::channel::RouteKind,
    ) {
        let owner_registry = Arc::new(RemoteSessionRegistry::default());
        let joiner_registry = Arc::new(RemoteSessionRegistry::default());
        let owner_device = test_device("owner");
        let joiner_device = test_device("joiner");
        let addresses = owner_endpoint
            .address()
            .addrs
            .iter()
            .map(ToString::to_string)
            .collect();
        let (_, invite) = create_invite_in_registry(
            &owner_registry,
            &owner_endpoint.endpoint_id().to_string(),
            addresses,
            NOW,
        )
        .unwrap();
        let imported = import_invite_in_registry(
            &joiner_registry,
            invite.to_canonical_json().unwrap(),
            NOW + 1,
        )
        .unwrap();

        let accepting_registry = Arc::clone(&owner_registry);
        let accepting_endpoint = owner_endpoint.clone();
        let accepting_device = owner_device.clone();
        let accepting = tokio::spawn(async move {
            accept_one_internet_session(
                &accepting_registry,
                &accepting_endpoint,
                &accepting_device,
                NOW + 2,
            )
            .await
        });
        let joiner_ready = connect_internet_session_in_registry(
            &joiner_registry,
            &joiner_endpoint,
            &joiner_device,
            &imported.session_id,
            NOW + 2,
        )
        .await
        .unwrap();
        let owner_ready = accepting.await.unwrap().unwrap();
        assert_eq!(
            owner_ready.status,
            RemoteSessionStatus::Ready {
                route: expected_route
            }
        );
        assert_eq!(
            joiner_ready.status,
            RemoteSessionStatus::Ready {
                route: expected_route
            }
        );
        assert_eq!(joiner_ready.peer, Some(owner_device));
        assert_eq!(owner_ready.peer, Some(joiner_device));
        let route = expected_route;
        let owner_session = owner_registry.established(&owner_ready.session_id).unwrap();
        let joiner_session = joiner_registry
            .established(&joiner_ready.session_id)
            .unwrap();
        let source_dir = test_dir("internet-command-source");
        let destination_dir = test_dir("internet-command-destination");
        tokio::fs::create_dir_all(&source_dir).await.unwrap();
        tokio::fs::create_dir_all(&destination_dir).await.unwrap();
        let sender = DeviceIdentity {
            device_id: "runtime-sender".to_owned(),
            display_name: "Runtime sender".to_owned(),
            hostname: "runtime-sender.local".to_owned(),
            platform: "test".to_owned(),
        };

        owner_registry
            .transition(
                &owner_ready.session_id,
                RemoteSessionStatus::Transferring { route },
                NOW + 3,
            )
            .unwrap();
        joiner_registry
            .transition(
                &joiner_ready.session_id,
                RemoteSessionStatus::Transferring { route },
                NOW + 3,
            )
            .unwrap();
        let (owner_parts, joiner_parts) = tokio::join!(
            owner_session.next_transfer_parts(),
            joiner_session.next_transfer_parts()
        );
        let (_, mut owner_send, mut owner_receive, mut owner_noise) = owner_parts.unwrap();
        let (_, mut joiner_send, mut joiner_receive, mut joiner_noise) = joiner_parts.unwrap();
        let cancelled_source = source_dir.join("cancelled.bin");
        tokio::fs::write(&cancelled_source, vec![0xCA; 128 * 1024])
            .await
            .unwrap();
        let cancellation_destination = destination_dir.clone();
        let receiving_cancel = tokio::spawn(async move {
            receive_transfer_with_cancellation(
                &mut owner_send,
                &mut owner_receive,
                &mut owner_noise,
                &cancellation_destination,
                |_, _| std::future::pending::<bool>(),
                |_| {},
                std::future::pending::<()>(),
            )
            .await
        });
        let sending_cancel = send_transfer_with_cancellation(
            &mut joiner_send,
            &mut joiner_receive,
            &mut joiner_noise,
            &[cancelled_source],
            "runtime-cancelled-transfer",
            &sender,
            CancellableSendCallbacks {
                cancelled: tokio::time::sleep(Duration::from_millis(25)),
                on_progress: |_: &crate::protocol::types::TransferProgress| {},
                on_accepted: || {},
            },
        )
        .await;
        let receiving_cancel = receiving_cancel.await.unwrap();
        assert!(sending_cancel.is_err());
        assert!(receiving_cancel.is_err());
        owner_registry
            .transition(
                &owner_ready.session_id,
                RemoteSessionStatus::Ready { route },
                NOW + 4,
            )
            .unwrap();
        joiner_registry
            .transition(
                &joiner_ready.session_id,
                RemoteSessionStatus::Ready { route },
                NOW + 4,
            )
            .unwrap();

        // Mutation captured: closing the authenticated channel after cancellation or the first
        // completed transfer makes the subsequent fresh Noise stream fail.
        for sequence in 1_u64..=2 {
            owner_registry
                .transition(
                    &owner_ready.session_id,
                    RemoteSessionStatus::Transferring { route },
                    NOW + sequence + 2,
                )
                .unwrap();
            joiner_registry
                .transition(
                    &joiner_ready.session_id,
                    RemoteSessionStatus::Transferring { route },
                    NOW + sequence + 2,
                )
                .unwrap();

            let (owner_parts, joiner_parts) = tokio::join!(
                owner_session.next_transfer_parts(),
                joiner_session.next_transfer_parts()
            );
            let (_, mut owner_send, mut owner_receive, mut owner_noise) = owner_parts.unwrap();
            let (_, mut joiner_send, mut joiner_receive, mut joiner_noise) = joiner_parts.unwrap();
            let file_name = format!("verified-{sequence}.bin");
            let source_path = source_dir.join(&file_name);
            let payload: Vec<u8> = (0..128 * 1024)
                .map(|index| ((index + sequence as usize) % 251) as u8)
                .collect();
            tokio::fs::write(&source_path, &payload).await.unwrap();
            let receiving_destination = destination_dir.clone();
            let receiving = tokio::spawn(async move {
                receive_transfer(
                    &mut owner_send,
                    &mut owner_receive,
                    &mut owner_noise,
                    &receiving_destination,
                    true,
                )
                .await
            });
            let bytes_sent = send_transfer(
                &mut joiner_send,
                &mut joiner_receive,
                &mut joiner_noise,
                &[source_path],
                &format!("runtime-transfer-{sequence}"),
                &sender,
                |_| {},
            )
            .await
            .unwrap();
            let receipt = receiving.await.unwrap().unwrap();
            let received = tokio::fs::read(destination_dir.join(file_name))
                .await
                .unwrap();
            assert_eq!(bytes_sent, payload.len() as u64);
            assert_eq!(receipt.bytes_received, bytes_sent);
            assert_eq!(
                ring::digest::digest(&ring::digest::SHA256, &received).as_ref(),
                ring::digest::digest(&ring::digest::SHA256, &payload).as_ref()
            );

            owner_registry
                .transition(
                    &owner_ready.session_id,
                    RemoteSessionStatus::Ready { route },
                    NOW + sequence + 10,
                )
                .unwrap();
            joiner_registry
                .transition(
                    &joiner_ready.session_id,
                    RemoteSessionStatus::Ready { route },
                    NOW + sequence + 10,
                )
                .unwrap();
            assert!(matches!(
                owner_registry.view(&owner_ready.session_id).unwrap().status,
                RemoteSessionStatus::Ready { .. }
            ));
            assert!(matches!(
                joiner_registry
                    .view(&joiner_ready.session_id)
                    .unwrap()
                    .status,
                RemoteSessionStatus::Ready { .. }
            ));
        }

        assert!(owner_registry.cancel(&owner_ready.session_id));
        assert!(joiner_registry.cancel(&joiner_ready.session_id));
        assert!(owner_registry.view(&owner_ready.session_id).is_none());
        assert!(joiner_registry.view(&joiner_ready.session_id).is_none());

        owner_endpoint.close().await;
        joiner_endpoint.close().await;
        tokio::fs::remove_dir_all(source_dir).await.unwrap();
        tokio::fs::remove_dir_all(destination_dir).await.unwrap();
    }

    #[tokio::test]
    async fn production_session_command_path_rejects_a_wrong_manual_code() {
        let owner_endpoint = loopback_endpoint().await;
        let joiner_endpoint = loopback_endpoint().await;
        let owner_registry = Arc::new(RemoteSessionRegistry::default());
        let joiner_registry = Arc::new(RemoteSessionRegistry::default());
        let owner_device = test_device("owner");
        let joiner_device = test_device("joiner");
        let addresses = owner_endpoint
            .address()
            .addrs
            .iter()
            .map(ToString::to_string)
            .collect();
        let (owner_view, invite) = create_invite_in_registry(
            &owner_registry,
            &owner_endpoint.endpoint_id().to_string(),
            addresses,
            NOW,
        )
        .unwrap();
        let share = create_share_view(&owner_registry, owner_view, &invite).unwrap();
        let imported = import_invite_in_registry(
            &joiner_registry,
            invite.to_canonical_json().unwrap(),
            NOW + 1,
        )
        .unwrap();
        let mut wrong_code = "00000000".to_owned();
        if wrong_code == share.manual_code {
            wrong_code = "99999999".to_owned();
        }
        confirm_pairing_code_in_registry(
            &joiner_registry,
            &imported.session_id,
            wrong_code,
            NOW + 2,
        )
        .unwrap();

        let accepting_registry = Arc::clone(&owner_registry);
        let accepting_endpoint = owner_endpoint.clone();
        let accepting_device = owner_device;
        let accepting = tokio::spawn(async move {
            accept_one_internet_session(
                &accepting_registry,
                &accepting_endpoint,
                &accepting_device,
                NOW + 2,
            )
            .await
        });
        let joining = connect_internet_session_in_registry(
            &joiner_registry,
            &joiner_endpoint,
            &joiner_device,
            &imported.session_id,
            NOW + 2,
        )
        .await;
        let owning = accepting.await.unwrap();

        assert!(joining.is_err());
        assert!(owning.is_err());
        assert!(joiner_registry.established(&imported.session_id).is_err());

        owner_endpoint.close().await;
        joiner_endpoint.close().await;
    }

    async fn loopback_endpoint() -> InternetEndpoint {
        InternetEndpoint::bind(
            InternetEndpointConfig::default()
                .with_bind_addr("127.0.0.1:0".parse().unwrap())
                .with_handshake_timeout(Duration::from_secs(3)),
        )
        .await
        .unwrap()
    }

    async fn relay_only_endpoint(relay_map: iroh::RelayMap) -> InternetEndpoint {
        InternetEndpoint::bind(
            InternetEndpointConfig::default()
                .with_relay_map(relay_map)
                .without_direct_transports_for_test()
                .with_insecure_relay_tls_for_test()
                .with_handshake_timeout(Duration::from_secs(5)),
        )
        .await
        .unwrap()
    }

    fn test_dir(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!("dukto-{label}-{}", uuid::Uuid::new_v4()))
    }

    fn test_device(label: &str) -> crate::state::device::DeviceIdentity {
        crate::state::device::DeviceIdentity {
            device_id: format!("{label}-device-id"),
            display_name: label.to_owned(),
            hostname: format!("{label}.local"),
            platform: "test".to_owned(),
        }
    }
}

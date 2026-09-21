use std::{
    fmt,
    sync::{Arc, OnceLock},
    time::Duration,
};

use iroh::endpoint::{RecvStream, SendStream};
use ring::rand::SecureRandom;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use snow::TransportState;
use thiserror::Error;
use tokio::{sync::Mutex, time::timeout};
use zeroize::Zeroizing;

use crate::{
    crypto::noise::{
        handshake_initiator_bound, handshake_responder_bound, recv_framed, send_framed,
    },
    internet::{
        channel::IrohAuthenticatedChannel,
        pairing::{
            ManualCodePairing, ManualPairingCode, PairingKeys, PairingOrigin, PairingRole,
            PairingTranscript, PAIRING_NONCE_BYTES,
        },
    },
    state::device::DeviceIdentity,
    transfer::channel::{AuthenticatedChannel, ChannelCloseReason, RouteKind},
};

const SESSION_PROTOCOL_VERSION: u8 = 1;
const SESSION_AUTH_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InternetSessionRole {
    InvitationOwner,
    InvitationJoiner,
}

pub struct InternetSessionCredentials {
    pub session_id: String,
    pub advertised_endpoint_id: String,
    pub addresses: Vec<String>,
    pub expires_at_unix: u64,
    pub role: InternetSessionRole,
    pub secret: Zeroizing<[u8; 32]>,
    pub manual_code: Option<ManualPairingCode>,
}

impl fmt::Debug for InternetSessionCredentials {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("InternetSessionCredentials")
            .field("session_id", &self.session_id)
            .field("advertised_endpoint_id", &self.advertised_endpoint_id)
            .field("addresses", &self.addresses)
            .field("expires_at_unix", &self.expires_at_unix)
            .field("role", &self.role)
            .field("secret", &"[REDACTED]")
            .field(
                "manual_code",
                &self.manual_code.as_ref().map(|_| "[REDACTED]"),
            )
            .finish()
    }
}

type InternetTransferParts = (SendStream, RecvStream, TransportState);

struct EstablishedInternetSessionInner {
    binding: Zeroizing<[u8; 32]>,
    channel: IrohAuthenticatedChannel,
    initial_transfer: Mutex<Option<InternetTransferParts>>,
    peer_device: OnceLock<DeviceIdentity>,
    role: InternetSessionRole,
}

#[derive(Clone)]
pub struct EstablishedInternetSession {
    inner: Arc<EstablishedInternetSessionInner>,
}

impl EstablishedInternetSession {
    pub fn peer_id(&self) -> &str {
        self.inner.channel.peer_id()
    }

    pub fn route_kind(&self) -> RouteKind {
        self.inner.channel.route_kind()
    }

    pub fn peer_device(&self) -> Option<&DeviceIdentity> {
        self.inner.peer_device.get()
    }

    pub async fn exchange_peer_identity(
        &self,
        local_device: &DeviceIdentity,
    ) -> Result<(), InternetSessionError> {
        let (mut send, mut receive) = match self.inner.role {
            InternetSessionRole::InvitationJoiner => self.inner.channel.open_bi().await,
            InternetSessionRole::InvitationOwner => self.inner.channel.accept_bi().await,
        }
        .map_err(|_| InternetSessionError::Transport)?;
        let mut noise = match self.inner.role {
            InternetSessionRole::InvitationJoiner => {
                handshake_initiator_bound(&mut send, &mut receive, &self.inner.binding).await
            }
            InternetSessionRole::InvitationOwner => {
                handshake_responder_bound(&mut send, &mut receive, &self.inner.binding).await
            }
        }
        .map_err(|_| InternetSessionError::AuthenticationFailed)?;

        let encoded = serde_json::to_vec(local_device)
            .map_err(|_| InternetSessionError::InvalidControlMessage)?;
        let mut ciphertext = vec![0_u8; 65_535];
        let written = noise
            .write_message(&encoded, &mut ciphertext)
            .map_err(|_| InternetSessionError::AuthenticationFailed)?;
        send_framed(&mut send, &ciphertext[..written])
            .await
            .map_err(|_| InternetSessionError::Transport)?;

        let encrypted = recv_framed(&mut receive)
            .await
            .map_err(|_| InternetSessionError::Transport)?;
        let read = noise
            .read_message(&encrypted, &mut ciphertext)
            .map_err(|_| InternetSessionError::AuthenticationFailed)?;
        let peer: DeviceIdentity = serde_json::from_slice(&ciphertext[..read])
            .map_err(|_| InternetSessionError::InvalidControlMessage)?;
        validate_device_identity(&peer)?;
        self.inner
            .peer_device
            .set(peer)
            .map_err(|_| InternetSessionError::InvalidControlMessage)
    }

    pub async fn next_transfer_parts(
        &self,
    ) -> Result<
        (
            IrohAuthenticatedChannel,
            SendStream,
            RecvStream,
            TransportState,
        ),
        InternetSessionError,
    > {
        if let Some((send, receive, noise)) = self.inner.initial_transfer.lock().await.take() {
            return Ok((self.inner.channel.clone(), send, receive, noise));
        }

        let (mut send, mut receive) = match self.inner.role {
            InternetSessionRole::InvitationJoiner => self.inner.channel.open_bi().await,
            InternetSessionRole::InvitationOwner => self.inner.channel.accept_bi().await,
        }
        .map_err(|_| InternetSessionError::Transport)?;
        let noise = match self.inner.role {
            InternetSessionRole::InvitationJoiner => {
                handshake_initiator_bound(&mut send, &mut receive, &self.inner.binding).await
            }
            InternetSessionRole::InvitationOwner => {
                handshake_responder_bound(&mut send, &mut receive, &self.inner.binding).await
            }
        }
        .map_err(|_| InternetSessionError::AuthenticationFailed)?;
        Ok((self.inner.channel.clone(), send, receive, noise))
    }

    pub async fn closed(&self) {
        self.inner.channel.closed().await;
    }

    pub fn close(&self, reason: ChannelCloseReason) {
        self.inner.channel.close(reason);
    }
}

impl fmt::Debug for EstablishedInternetSession {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("EstablishedInternetSession")
            .field("peer_id", &self.peer_id())
            .field("route", &self.route_kind())
            .field("role", &self.inner.role)
            .finish()
    }
}

fn established_session(
    binding: [u8; 32],
    channel: IrohAuthenticatedChannel,
    noise: TransportState,
    receive: RecvStream,
    role: InternetSessionRole,
    send: SendStream,
) -> EstablishedInternetSession {
    EstablishedInternetSession {
        inner: Arc::new(EstablishedInternetSessionInner {
            binding: Zeroizing::new(binding),
            channel,
            initial_transfer: Mutex::new(Some((send, receive, noise))),
            peer_device: OnceLock::new(),
            role,
        }),
    }
}

fn validate_device_identity(device: &DeviceIdentity) -> Result<(), InternetSessionError> {
    let fields = [
        (&device.device_id, 128_usize),
        (&device.display_name, 256_usize),
        (&device.hostname, 256_usize),
        (&device.platform, 64_usize),
    ];
    if fields.iter().any(|(value, max_len)| {
        value.trim().is_empty() || value.len() > *max_len || value.chars().any(char::is_control)
    }) {
        return Err(InternetSessionError::InvalidControlMessage);
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum InternetSessionError {
    #[error("internet session role is invalid")]
    InvalidRole,
    #[error("internet session peer identity does not match")]
    PeerIdentityMismatch,
    #[error("internet session control message is invalid")]
    InvalidControlMessage,
    #[error("internet session authentication failed")]
    AuthenticationFailed,
    #[error("internet session authentication timed out")]
    Timeout,
    #[error("internet session transport failed")]
    Transport,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum PairingMode {
    InviteLink,
    ManualCode,
}

impl PairingMode {
    fn origin(self) -> PairingOrigin {
        match self {
            Self::InviteLink => PairingOrigin::InviteLink,
            Self::ManualCode => PairingOrigin::ManualCode,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct InitiatorHello {
    version: u8,
    session_id: String,
    endpoint_id: String,
    mode: PairingMode,
    nonce: [u8; PAIRING_NONCE_BYTES],
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ResponderHello {
    version: u8,
    nonce: [u8; PAIRING_NONCE_BYTES],
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PairingMessage {
    version: u8,
    message: Vec<u8>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PairingConfirmation {
    version: u8,
    value: [u8; 32],
}

pub async fn authenticate_initiator(
    channel: IrohAuthenticatedChannel,
    local_endpoint_id: &str,
    credentials: InternetSessionCredentials,
    now_unix: u64,
) -> Result<EstablishedInternetSession, InternetSessionError> {
    if credentials.role != InternetSessionRole::InvitationJoiner {
        return Err(InternetSessionError::InvalidRole);
    }
    if channel.peer_id() != credentials.advertised_endpoint_id {
        channel.close(ChannelCloseReason::AuthenticationFailed);
        return Err(InternetSessionError::PeerIdentityMismatch);
    }

    let close_channel = channel.clone();
    match timeout(
        SESSION_AUTH_TIMEOUT,
        authenticate_initiator_inner(channel, local_endpoint_id, credentials, now_unix),
    )
    .await
    {
        Ok(Ok(session)) => Ok(session),
        Ok(Err(error)) => {
            close_channel.close(ChannelCloseReason::AuthenticationFailed);
            Err(error)
        }
        Err(_) => {
            close_channel.close(ChannelCloseReason::AuthenticationFailed);
            Err(InternetSessionError::Timeout)
        }
    }
}

async fn authenticate_initiator_inner(
    channel: IrohAuthenticatedChannel,
    local_endpoint_id: &str,
    credentials: InternetSessionCredentials,
    now_unix: u64,
) -> Result<EstablishedInternetSession, InternetSessionError> {
    let (mut send, mut receive) = channel
        .open_bi()
        .await
        .map_err(|_| InternetSessionError::Transport)?;
    let initiator_nonce = generate_nonce()?;
    let mode = if credentials.manual_code.is_some() {
        PairingMode::ManualCode
    } else {
        PairingMode::InviteLink
    };
    send_control(
        &mut send,
        &InitiatorHello {
            version: SESSION_PROTOCOL_VERSION,
            session_id: credentials.session_id.clone(),
            endpoint_id: local_endpoint_id.to_owned(),
            mode,
            nonce: initiator_nonce,
        },
    )
    .await?;
    let responder: ResponderHello = receive_control(&mut receive).await?;
    validate_version_and_nonce(responder.version, &responder.nonce)?;

    let transcript = PairingTranscript::new(
        mode.origin(),
        &credentials.session_id,
        local_endpoint_id,
        &credentials.advertised_endpoint_id,
        initiator_nonce,
        responder.nonce,
        credentials.expires_at_unix,
        now_unix,
    )
    .map_err(|_| InternetSessionError::AuthenticationFailed)?;
    let keys = initiator_keys(&mut send, &mut receive, &credentials, &transcript).await?;

    send_control(
        &mut send,
        &PairingConfirmation {
            version: SESSION_PROTOCOL_VERSION,
            value: *keys.confirmation(PairingRole::Initiator),
        },
    )
    .await?;
    let confirmation: PairingConfirmation = receive_control(&mut receive).await?;
    validate_confirmation(confirmation, keys.confirmation(PairingRole::Responder))?;
    let noise = handshake_initiator_bound(&mut send, &mut receive, keys.noise_binding())
        .await
        .map_err(|_| InternetSessionError::AuthenticationFailed)?;
    Ok(established_session(
        *keys.noise_binding(),
        channel,
        noise,
        receive,
        InternetSessionRole::InvitationJoiner,
        send,
    ))
}

pub async fn authenticate_responder<F>(
    channel: IrohAuthenticatedChannel,
    local_endpoint_id: &str,
    now_unix: u64,
    credentials_for: F,
) -> Result<(String, EstablishedInternetSession), InternetSessionError>
where
    F: FnOnce(&str) -> Result<InternetSessionCredentials, InternetSessionError>,
{
    let close_channel = channel.clone();
    match timeout(
        SESSION_AUTH_TIMEOUT,
        authenticate_responder_inner(channel, local_endpoint_id, now_unix, credentials_for),
    )
    .await
    {
        Ok(Ok(session)) => Ok(session),
        Ok(Err(error)) => {
            close_channel.close(ChannelCloseReason::AuthenticationFailed);
            Err(error)
        }
        Err(_) => {
            close_channel.close(ChannelCloseReason::AuthenticationFailed);
            Err(InternetSessionError::Timeout)
        }
    }
}

async fn authenticate_responder_inner<F>(
    channel: IrohAuthenticatedChannel,
    local_endpoint_id: &str,
    now_unix: u64,
    credentials_for: F,
) -> Result<(String, EstablishedInternetSession), InternetSessionError>
where
    F: FnOnce(&str) -> Result<InternetSessionCredentials, InternetSessionError>,
{
    let (mut send, mut receive) = channel
        .accept_bi()
        .await
        .map_err(|_| InternetSessionError::Transport)?;
    let hello: InitiatorHello = receive_control(&mut receive).await?;
    validate_version_and_nonce(hello.version, &hello.nonce)?;
    if hello.endpoint_id != channel.peer_id() {
        return Err(InternetSessionError::PeerIdentityMismatch);
    }
    let credentials = credentials_for(&hello.session_id)?;
    if credentials.role != InternetSessionRole::InvitationOwner
        || credentials.advertised_endpoint_id != local_endpoint_id
        || credentials.session_id != hello.session_id
    {
        return Err(InternetSessionError::InvalidRole);
    }
    let responder_nonce = generate_nonce()?;
    send_control(
        &mut send,
        &ResponderHello {
            version: SESSION_PROTOCOL_VERSION,
            nonce: responder_nonce,
        },
    )
    .await?;

    let transcript = PairingTranscript::new(
        hello.mode.origin(),
        &credentials.session_id,
        channel.peer_id(),
        local_endpoint_id,
        hello.nonce,
        responder_nonce,
        credentials.expires_at_unix,
        now_unix,
    )
    .map_err(|_| InternetSessionError::AuthenticationFailed)?;
    let keys = responder_keys(
        &mut send,
        &mut receive,
        &credentials,
        &transcript,
        hello.mode,
    )
    .await?;

    let confirmation: PairingConfirmation = receive_control(&mut receive).await?;
    validate_confirmation(confirmation, keys.confirmation(PairingRole::Initiator))?;
    send_control(
        &mut send,
        &PairingConfirmation {
            version: SESSION_PROTOCOL_VERSION,
            value: *keys.confirmation(PairingRole::Responder),
        },
    )
    .await?;
    let noise = handshake_responder_bound(&mut send, &mut receive, keys.noise_binding())
        .await
        .map_err(|_| InternetSessionError::AuthenticationFailed)?;
    let session_id = credentials.session_id;
    Ok((
        session_id,
        established_session(
            *keys.noise_binding(),
            channel,
            noise,
            receive,
            InternetSessionRole::InvitationOwner,
            send,
        ),
    ))
}

async fn initiator_keys(
    send: &mut SendStream,
    receive: &mut RecvStream,
    credentials: &InternetSessionCredentials,
    transcript: &PairingTranscript,
) -> Result<PairingKeys, InternetSessionError> {
    let Some(code) = credentials.manual_code.as_ref() else {
        return transcript
            .derive_keys(credentials.secret.as_ref())
            .map_err(|_| InternetSessionError::AuthenticationFailed);
    };
    let (mut pairing, message) = ManualCodePairing::start(PairingRole::Initiator, code, transcript)
        .map_err(|_| InternetSessionError::AuthenticationFailed)?;
    send_control(
        send,
        &PairingMessage {
            version: SESSION_PROTOCOL_VERSION,
            message,
        },
    )
    .await?;
    let peer: PairingMessage = receive_control(receive).await?;
    validate_pairing_message(&peer)?;
    pairing
        .finish(&peer.message)
        .map_err(|_| InternetSessionError::AuthenticationFailed)
}

async fn responder_keys(
    send: &mut SendStream,
    receive: &mut RecvStream,
    credentials: &InternetSessionCredentials,
    transcript: &PairingTranscript,
    mode: PairingMode,
) -> Result<PairingKeys, InternetSessionError> {
    if mode == PairingMode::InviteLink {
        return transcript
            .derive_keys(credentials.secret.as_ref())
            .map_err(|_| InternetSessionError::AuthenticationFailed);
    }
    let code = credentials
        .manual_code
        .as_ref()
        .ok_or(InternetSessionError::AuthenticationFailed)?;
    let peer: PairingMessage = receive_control(receive).await?;
    validate_pairing_message(&peer)?;
    let (mut pairing, message) = ManualCodePairing::start(PairingRole::Responder, code, transcript)
        .map_err(|_| InternetSessionError::AuthenticationFailed)?;
    send_control(
        send,
        &PairingMessage {
            version: SESSION_PROTOCOL_VERSION,
            message,
        },
    )
    .await?;
    pairing
        .finish(&peer.message)
        .map_err(|_| InternetSessionError::AuthenticationFailed)
}

fn validate_pairing_message(message: &PairingMessage) -> Result<(), InternetSessionError> {
    if message.version != SESSION_PROTOCOL_VERSION || message.message.is_empty() {
        return Err(InternetSessionError::InvalidControlMessage);
    }
    Ok(())
}

#[allow(deprecated)]
fn validate_confirmation(
    confirmation: PairingConfirmation,
    expected: &[u8; 32],
) -> Result<(), InternetSessionError> {
    if confirmation.version != SESSION_PROTOCOL_VERSION
        || ring::constant_time::verify_slices_are_equal(&confirmation.value, expected).is_err()
    {
        return Err(InternetSessionError::AuthenticationFailed);
    }
    Ok(())
}

fn validate_version_and_nonce(
    version: u8,
    nonce: &[u8; PAIRING_NONCE_BYTES],
) -> Result<(), InternetSessionError> {
    if version != SESSION_PROTOCOL_VERSION || nonce.iter().all(|byte| *byte == 0) {
        return Err(InternetSessionError::InvalidControlMessage);
    }
    Ok(())
}

fn generate_nonce() -> Result<[u8; PAIRING_NONCE_BYTES], InternetSessionError> {
    let mut nonce = [0_u8; PAIRING_NONCE_BYTES];
    ring::rand::SystemRandom::new()
        .fill(&mut nonce)
        .map_err(|_| InternetSessionError::AuthenticationFailed)?;
    Ok(nonce)
}

async fn send_control<T: Serialize>(
    send: &mut SendStream,
    message: &T,
) -> Result<(), InternetSessionError> {
    let encoded =
        serde_json::to_vec(message).map_err(|_| InternetSessionError::InvalidControlMessage)?;
    send_framed(send, &encoded)
        .await
        .map_err(|_| InternetSessionError::Transport)
}

async fn receive_control<T: DeserializeOwned>(
    receive: &mut RecvStream,
) -> Result<T, InternetSessionError> {
    let encoded = recv_framed(receive)
        .await
        .map_err(|_| InternetSessionError::Transport)?;
    serde_json::from_slice(&encoded).map_err(|_| InternetSessionError::InvalidControlMessage)
}

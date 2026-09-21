use std::{fmt, future::Future, pin::Pin};

use dukto_admission::{AdmissionCapability, AdmissionRegistration, ConsumeRequest, PublishRequest};
use futures_util::StreamExt;
use ring::{
    aead::{Aad, LessSafeKey, Nonce, UnboundKey, AES_256_GCM},
    rand::{SecureRandom, SystemRandom},
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
    crypto::hkdf_sha256,
    internet::invite::{InternetInvite, InviteError},
};

const RENDEZVOUS_ENVELOPE_VERSION: u8 = 1;
const RENDEZVOUS_SLOT_BYTES: usize = 32;
const RENDEZVOUS_NONCE_BYTES: usize = 12;
const ENVELOPE_KEY_LABEL: &[u8] = b"dukto/internet/v1/rendezvous-envelope";
const MAX_ENDPOINT_ID_LEN: usize = 256;
const MAX_ADDRESS_COUNT: usize = 16;
const MAX_ADDRESS_LEN: usize = 512;
const MAX_HTTP_RESPONSE_BYTES: usize = 20 * 1024;
const RENDEZVOUS_URL_ENV: &str = "DUKTO_RENDEZVOUS_URL";
const DISABLE_OPERATED_SERVICES_ENV: &str = "DUKTO_DISABLE_OPERATED_SERVICES";
const OFFICIAL_RENDEZVOUS_URL: &str = "https://rendezvous.djalmajr.dev/v1";

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum RendezvousError {
    #[error("rendezvous client configuration is invalid")]
    InvalidConfiguration,
    #[error("rendezvous slot is invalid")]
    InvalidSlot,
    #[error("rendezvous payload is invalid")]
    InvalidPayload,
    #[error("rendezvous envelope is invalid")]
    InvalidEnvelope,
    #[error("rendezvous invitation has expired")]
    Expired,
    #[error("rendezvous envelope exceeds the configured limit")]
    EnvelopeTooLarge,
    #[error("rendezvous slot is unavailable")]
    SlotUnavailable,
    #[error("rendezvous envelope was already consumed")]
    AlreadyConsumed,
    #[error("rendezvous envelope was not found")]
    NotFound,
    #[error("rendezvous encryption failed")]
    Encryption,
    #[error("rendezvous decryption failed")]
    Decryption,
    #[error("rendezvous request failed")]
    Network,
    #[error("rendezvous request was rate limited")]
    RateLimited,
    #[error("rendezvous service is unavailable")]
    Unavailable,
    #[error("rendezvous admission was denied")]
    AdmissionDenied,
}

#[derive(Clone, PartialEq, Eq)]
pub struct RendezvousSlot(String);

impl RendezvousSlot {
    pub fn generate() -> Result<Self, RendezvousError> {
        let mut bytes = [0_u8; RENDEZVOUS_SLOT_BYTES];
        SystemRandom::new()
            .fill(&mut bytes)
            .map_err(|_| RendezvousError::Encryption)?;
        Ok(Self(hex_encode(&bytes)))
    }

    pub fn parse(slot: impl Into<String>) -> Result<Self, RendezvousError> {
        let slot = slot.into();
        if slot.len() != RENDEZVOUS_SLOT_BYTES * 2
            || !slot.bytes().all(|byte| byte.is_ascii_hexdigit())
            || slot.bytes().any(|byte| byte.is_ascii_uppercase())
        {
            return Err(RendezvousError::InvalidSlot);
        }
        Ok(Self(slot))
    }

    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for RendezvousSlot {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("RendezvousSlot([REDACTED])")
    }
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RendezvousPayload {
    endpoint_id: String,
    addresses: Vec<String>,
    pairing_nonce: [u8; 32],
}

impl RendezvousPayload {
    pub fn generate(
        endpoint_id: impl Into<String>,
        addresses: Vec<String>,
    ) -> Result<Self, RendezvousError> {
        let mut pairing_nonce = [0_u8; 32];
        SystemRandom::new()
            .fill(&mut pairing_nonce)
            .map_err(|_| RendezvousError::Encryption)?;
        Self::new(endpoint_id, addresses, pairing_nonce)
    }

    pub fn new(
        endpoint_id: impl Into<String>,
        mut addresses: Vec<String>,
        pairing_nonce: [u8; 32],
    ) -> Result<Self, RendezvousError> {
        addresses.sort_unstable();
        addresses.dedup();
        let payload = Self {
            endpoint_id: endpoint_id.into(),
            addresses,
            pairing_nonce,
        };
        payload.validate()?;
        Ok(payload)
    }

    fn validate(&self) -> Result<(), RendezvousError> {
        if !is_safe_token(&self.endpoint_id, MAX_ENDPOINT_ID_LEN)
            || self.addresses.is_empty()
            || self.addresses.len() > MAX_ADDRESS_COUNT
            || self
                .addresses
                .iter()
                .any(|address| !is_safe_token(address, MAX_ADDRESS_LEN))
            || self
                .addresses
                .windows(2)
                .any(|pair| pair[0].as_str() >= pair[1].as_str())
            || self.pairing_nonce.iter().all(|byte| *byte == 0)
        {
            return Err(RendezvousError::InvalidPayload);
        }
        Ok(())
    }

    pub fn endpoint_id(&self) -> &str {
        &self.endpoint_id
    }

    pub fn addresses(&self) -> &[String] {
        &self.addresses
    }
}

impl fmt::Debug for RendezvousPayload {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RendezvousPayload")
            .field("endpoint_id", &self.endpoint_id)
            .field("addresses", &self.addresses)
            .field("pairing_nonce", &"[REDACTED]")
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RendezvousEnvelope {
    version: u8,
    expires_at_unix: u64,
    nonce: [u8; RENDEZVOUS_NONCE_BYTES],
    ciphertext: Vec<u8>,
}

impl fmt::Debug for RendezvousEnvelope {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RendezvousEnvelope")
            .field("version", &self.version)
            .field("expires_at_unix", &self.expires_at_unix)
            .field("nonce", &"[REDACTED]")
            .field("ciphertext_bytes", &self.ciphertext.len())
            .finish()
    }
}

impl RendezvousEnvelope {
    fn encoded_len(&self) -> usize {
        self.ciphertext.len() + self.nonce.len() + 32
    }

    #[cfg(test)]
    fn tamper_for_test(&mut self) {
        if let Some(byte) = self.ciphertext.first_mut() {
            *byte ^= 1;
        }
    }
}

pub trait RendezvousTransport: Send + Sync {
    fn publish<'a>(
        &'a self,
        slot: &'a str,
        request: PublishRequest<RendezvousEnvelope>,
    ) -> Pin<Box<dyn Future<Output = Result<(), RendezvousError>> + Send + 'a>>;

    fn consume<'a>(
        &'a self,
        slot: &'a str,
        request: ConsumeRequest,
    ) -> Pin<Box<dyn Future<Output = Result<RendezvousEnvelope, RendezvousError>> + Send + 'a>>;
}

#[derive(Clone)]
pub struct HttpRendezvousTransport {
    client: reqwest::Client,
    base_url: reqwest::Url,
}

impl fmt::Debug for HttpRendezvousTransport {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("HttpRendezvousTransport")
            .field("origin", &self.base_url.origin().ascii_serialization())
            .finish()
    }
}

impl HttpRendezvousTransport {
    pub fn configured_from_environment() -> Result<Option<Self>, RendezvousError> {
        let url = optional_unicode_environment(RENDEZVOUS_URL_ENV)?;
        let disabled = optional_unicode_environment(DISABLE_OPERATED_SERVICES_ENV)?;
        Self::configured_from_values(url.as_deref(), disabled.as_deref())
    }

    pub fn new(base_url: &str) -> Result<Self, RendezvousError> {
        Self::new_inner(base_url, false)
    }

    #[cfg(test)]
    fn new_for_test(base_url: &str) -> Result<Self, RendezvousError> {
        Self::new_inner(base_url, true)
    }

    fn new_inner(base_url: &str, allow_http: bool) -> Result<Self, RendezvousError> {
        let mut base_url =
            reqwest::Url::parse(base_url).map_err(|_| RendezvousError::InvalidConfiguration)?;
        if ((!allow_http && base_url.scheme() != "https")
            || (allow_http && !matches!(base_url.scheme(), "http" | "https")))
            || base_url.host_str().is_none()
            || !base_url.username().is_empty()
            || base_url.password().is_some()
            || base_url.query().is_some()
            || base_url.fragment().is_some()
        {
            return Err(RendezvousError::InvalidConfiguration);
        }
        let path = format!("{}/", base_url.path().trim_end_matches('/'));
        base_url.set_path(&path);
        let _ = rustls::crypto::ring::default_provider().install_default();
        let client = reqwest::Client::builder()
            .connect_timeout(std::time::Duration::from_secs(5))
            .timeout(std::time::Duration::from_secs(10))
            .redirect(reqwest::redirect::Policy::none())
            .user_agent(concat!("dukto/", env!("CARGO_PKG_VERSION")))
            .build()
            .map_err(|_| RendezvousError::InvalidConfiguration)?;
        Ok(Self { client, base_url })
    }

    fn configured_from_values(
        url: Option<&str>,
        disable_operated_services: Option<&str>,
    ) -> Result<Option<Self>, RendezvousError> {
        let disabled = match disable_operated_services {
            None | Some("0" | "false") => false,
            Some("1" | "true") => true,
            Some(_) => return Err(RendezvousError::InvalidConfiguration),
        };
        match url {
            Some(url) if !url.trim().is_empty() => Self::new(url.trim()).map(Some),
            Some(_) => Err(RendezvousError::InvalidConfiguration),
            None if disabled => Ok(None),
            None => Self::new(OFFICIAL_RENDEZVOUS_URL).map(Some),
        }
    }

    fn slot_url(&self, slot: &str, consume: bool) -> Result<reqwest::Url, RendezvousError> {
        if slot.len() != RENDEZVOUS_SLOT_BYTES * 2
            || !slot.bytes().all(|byte| byte.is_ascii_hexdigit())
            || slot.bytes().any(|byte| byte.is_ascii_uppercase())
        {
            return Err(RendezvousError::InvalidSlot);
        }
        let suffix = if consume {
            format!("slots/{slot}/consume")
        } else {
            format!("slots/{slot}")
        };
        self.base_url
            .join(&suffix)
            .map_err(|_| RendezvousError::InvalidConfiguration)
    }

    async fn bounded_envelope(
        response: reqwest::Response,
    ) -> Result<RendezvousEnvelope, RendezvousError> {
        if response
            .content_length()
            .is_some_and(|length| length > MAX_HTTP_RESPONSE_BYTES as u64)
        {
            return Err(RendezvousError::EnvelopeTooLarge);
        }
        let mut body = Vec::new();
        let mut chunks = response.bytes_stream();
        while let Some(chunk) = chunks.next().await {
            let chunk = chunk.map_err(|_| RendezvousError::Network)?;
            if body.len().saturating_add(chunk.len()) > MAX_HTTP_RESPONSE_BYTES {
                return Err(RendezvousError::EnvelopeTooLarge);
            }
            body.extend_from_slice(&chunk);
        }
        serde_json::from_slice(&body).map_err(|_| RendezvousError::InvalidEnvelope)
    }
}

fn optional_unicode_environment(name: &str) -> Result<Option<String>, RendezvousError> {
    match std::env::var(name) {
        Ok(value) => Ok(Some(value)),
        Err(std::env::VarError::NotPresent) => Ok(None),
        Err(std::env::VarError::NotUnicode(_)) => Err(RendezvousError::InvalidConfiguration),
    }
}

impl RendezvousTransport for HttpRendezvousTransport {
    fn publish<'a>(
        &'a self,
        slot: &'a str,
        request: PublishRequest<RendezvousEnvelope>,
    ) -> Pin<Box<dyn Future<Output = Result<(), RendezvousError>> + Send + 'a>> {
        Box::pin(async move {
            let response = self
                .client
                .put(self.slot_url(slot, false)?)
                .header(reqwest::header::CACHE_CONTROL, "no-store")
                .json(&request)
                .send()
                .await
                .map_err(|_| RendezvousError::Network)?;
            match response.status() {
                reqwest::StatusCode::CREATED => Ok(()),
                reqwest::StatusCode::CONFLICT => Err(RendezvousError::SlotUnavailable),
                reqwest::StatusCode::GONE => Err(RendezvousError::Expired),
                reqwest::StatusCode::PAYLOAD_TOO_LARGE => Err(RendezvousError::EnvelopeTooLarge),
                reqwest::StatusCode::TOO_MANY_REQUESTS => Err(RendezvousError::RateLimited),
                reqwest::StatusCode::SERVICE_UNAVAILABLE => Err(RendezvousError::Unavailable),
                _ => Err(RendezvousError::Network),
            }
        })
    }

    fn consume<'a>(
        &'a self,
        slot: &'a str,
        request: ConsumeRequest,
    ) -> Pin<Box<dyn Future<Output = Result<RendezvousEnvelope, RendezvousError>> + Send + 'a>>
    {
        Box::pin(async move {
            let response = self
                .client
                .post(self.slot_url(slot, true)?)
                .header(reqwest::header::CACHE_CONTROL, "no-store")
                .json(&request)
                .send()
                .await
                .map_err(|_| RendezvousError::Network)?;
            match response.status() {
                reqwest::StatusCode::OK => Self::bounded_envelope(response).await,
                reqwest::StatusCode::NOT_FOUND => Err(RendezvousError::NotFound),
                reqwest::StatusCode::CONFLICT => Err(RendezvousError::AlreadyConsumed),
                reqwest::StatusCode::GONE => Err(RendezvousError::Expired),
                reqwest::StatusCode::FORBIDDEN => Err(RendezvousError::AdmissionDenied),
                reqwest::StatusCode::PAYLOAD_TOO_LARGE => Err(RendezvousError::EnvelopeTooLarge),
                reqwest::StatusCode::TOO_MANY_REQUESTS => Err(RendezvousError::RateLimited),
                reqwest::StatusCode::SERVICE_UNAVAILABLE => Err(RendezvousError::Unavailable),
                _ => Err(RendezvousError::Network),
            }
        })
    }
}

pub struct RendezvousClient<'a, T: RendezvousTransport + ?Sized> {
    transport: &'a T,
    max_envelope_bytes: usize,
}

impl<'a, T: RendezvousTransport + ?Sized> RendezvousClient<'a, T> {
    pub fn new(transport: &'a T, max_envelope_bytes: usize) -> Result<Self, RendezvousError> {
        if max_envelope_bytes == 0 {
            return Err(RendezvousError::InvalidConfiguration);
        }
        Ok(Self {
            transport,
            max_envelope_bytes,
        })
    }

    pub async fn publish(
        &self,
        invite: &InternetInvite,
        payload: &RendezvousPayload,
        now_unix: u64,
    ) -> Result<(), RendezvousError> {
        validate_invite(invite, now_unix)?;
        payload.validate()?;
        let slot = invite_slot(invite)?;
        let envelope = seal(invite, &slot, payload)?;
        self.ensure_size(&envelope)?;
        let capability = AdmissionCapability::derive(invite.secret_bytes(), invite.session_id())
            .map_err(|_| RendezvousError::Encryption)?;
        self.transport
            .publish(
                slot.expose(),
                PublishRequest {
                    envelope,
                    admission: AdmissionRegistration {
                        endpoint_id: payload.endpoint_id().to_owned(),
                        capability: capability.encode(),
                    },
                },
            )
            .await
    }

    pub async fn consume(
        &self,
        invite: &InternetInvite,
        joining_endpoint_id: &str,
        now_unix: u64,
    ) -> Result<RendezvousPayload, RendezvousError> {
        validate_invite(invite, now_unix)?;
        let slot = invite_slot(invite)?;
        let capability = AdmissionCapability::derive(invite.secret_bytes(), invite.session_id())
            .map_err(|_| RendezvousError::Decryption)?;
        let proof = capability
            .join_proof(slot.expose(), joining_endpoint_id, invite.expires_at_unix())
            .map_err(|_| RendezvousError::InvalidEnvelope)?;
        let envelope = self
            .transport
            .consume(
                slot.expose(),
                ConsumeRequest {
                    endpoint_id: joining_endpoint_id.to_owned(),
                    proof,
                },
            )
            .await?;
        self.ensure_size(&envelope)?;
        if envelope.version != RENDEZVOUS_ENVELOPE_VERSION
            || envelope.expires_at_unix != invite.expires_at_unix()
            || now_unix >= envelope.expires_at_unix
        {
            return Err(if now_unix >= envelope.expires_at_unix {
                RendezvousError::Expired
            } else {
                RendezvousError::InvalidEnvelope
            });
        }
        open(invite, &slot, envelope)
    }

    fn ensure_size(&self, envelope: &RendezvousEnvelope) -> Result<(), RendezvousError> {
        if envelope.encoded_len() > self.max_envelope_bytes {
            return Err(RendezvousError::EnvelopeTooLarge);
        }
        Ok(())
    }
}

fn seal(
    invite: &InternetInvite,
    slot: &RendezvousSlot,
    payload: &RendezvousPayload,
) -> Result<RendezvousEnvelope, RendezvousError> {
    let key = envelope_key(invite)?;
    let key = LessSafeKey::new(
        UnboundKey::new(&AES_256_GCM, &key).map_err(|_| RendezvousError::Encryption)?,
    );
    let mut nonce = [0_u8; RENDEZVOUS_NONCE_BYTES];
    SystemRandom::new()
        .fill(&mut nonce)
        .map_err(|_| RendezvousError::Encryption)?;
    let mut ciphertext =
        serde_json::to_vec(payload).map_err(|_| RendezvousError::InvalidPayload)?;
    key.seal_in_place_append_tag(
        Nonce::assume_unique_for_key(nonce),
        Aad::from(aad(slot, invite.expires_at_unix())),
        &mut ciphertext,
    )
    .map_err(|_| RendezvousError::Encryption)?;
    Ok(RendezvousEnvelope {
        version: RENDEZVOUS_ENVELOPE_VERSION,
        expires_at_unix: invite.expires_at_unix(),
        nonce,
        ciphertext,
    })
}

fn open(
    invite: &InternetInvite,
    slot: &RendezvousSlot,
    envelope: RendezvousEnvelope,
) -> Result<RendezvousPayload, RendezvousError> {
    let key = envelope_key(invite)?;
    let key = LessSafeKey::new(
        UnboundKey::new(&AES_256_GCM, &key).map_err(|_| RendezvousError::Decryption)?,
    );
    let mut ciphertext = envelope.ciphertext;
    let plaintext = key
        .open_in_place(
            Nonce::assume_unique_for_key(envelope.nonce),
            Aad::from(aad(slot, envelope.expires_at_unix)),
            &mut ciphertext,
        )
        .map_err(|_| RendezvousError::Decryption)?;
    let payload: RendezvousPayload =
        serde_json::from_slice(plaintext).map_err(|_| RendezvousError::InvalidPayload)?;
    payload.validate()?;
    Ok(payload)
}

fn envelope_key(invite: &InternetInvite) -> Result<[u8; 32], RendezvousError> {
    hkdf_sha256(
        invite.secret_bytes(),
        invite.session_id().as_bytes(),
        ENVELOPE_KEY_LABEL,
    )
    .map_err(|_| RendezvousError::Encryption)
}

fn invite_slot(invite: &InternetInvite) -> Result<RendezvousSlot, RendezvousError> {
    RendezvousSlot::parse(
        invite
            .rendezvous_slot()
            .ok_or(RendezvousError::InvalidSlot)?,
    )
}

fn validate_invite(invite: &InternetInvite, now_unix: u64) -> Result<(), RendezvousError> {
    invite.validate_at(now_unix).map_err(|error| match error {
        InviteError::Expired => RendezvousError::Expired,
        _ => RendezvousError::InvalidEnvelope,
    })
}

fn aad(slot: &RendezvousSlot, expires_at_unix: u64) -> Vec<u8> {
    let mut output = Vec::with_capacity(1 + slot.expose().len() + 8);
    output.push(RENDEZVOUS_ENVELOPE_VERSION);
    output.extend_from_slice(slot.expose().as_bytes());
    output.extend_from_slice(&expires_at_unix.to_be_bytes());
    output
}

fn is_safe_token(value: &str, max_len: usize) -> bool {
    !value.is_empty()
        && value.len() <= max_len
        && value.trim() == value
        && value.chars().all(|character| {
            character.is_ascii() && !character.is_ascii_control() && !character.is_whitespace()
        })
}

fn hex_encode(bytes: &[u8]) -> String {
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use std::fmt::Write;
        write!(&mut encoded, "{byte:02x}").expect("writing to a string cannot fail");
    }
    encoded
}

#[cfg(test)]
mod tests {
    use std::{
        collections::{HashMap, HashSet},
        future::Future,
        pin::Pin,
        sync::{Arc, Mutex},
    };

    use dukto_admission::{
        AdmissionCapability, AdmissionRegistration, ConsumeRequest, PublishRequest,
    };
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    use super::{
        hex_encode, seal, HttpRendezvousTransport, RendezvousClient, RendezvousEnvelope,
        RendezvousError, RendezvousPayload, RendezvousSlot, RendezvousTransport,
        MAX_HTTP_RESPONSE_BYTES,
    };
    use crate::internet::invite::InternetInvite;

    const NOW: u64 = 1_800_000_000;
    const OWNER: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
    const JOINER: &str = "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc";

    #[test]
    fn normal_runtime_uses_operated_rendezvous_with_an_explicit_disable_escape() {
        let normal = HttpRendezvousTransport::configured_from_values(None, None).unwrap();
        assert!(normal.is_some());
        let disabled = HttpRendezvousTransport::configured_from_values(None, Some("true")).unwrap();
        assert!(disabled.is_none());
        assert_eq!(
            HttpRendezvousTransport::configured_from_values(None, Some("yes")).unwrap_err(),
            RendezvousError::InvalidConfiguration
        );
    }

    #[tokio::test]
    async fn server_observes_only_opaque_slot_and_ciphertext_then_consumes_once() {
        let server = TestServer::default();
        let client = RendezvousClient::new(&server, 16 * 1024).unwrap();
        let slot = RendezvousSlot::generate().expect("opaque slot");
        let invite = invite(&slot, 300);
        let payload = payload();

        client
            .publish(&invite, &payload, NOW)
            .await
            .expect("publish envelope");

        let (observed_slot, observed_envelope) = server.observed();
        let observed_json = serde_json::to_string(&observed_envelope).unwrap();
        assert_eq!(observed_slot, slot.expose());
        assert!(!observed_json.contains("endpoint-public-key"));
        assert!(!observed_json.contains("203.0.113.7"));
        assert!(!observed_json.contains("pairing"));
        assert!(format!("{slot:?}").contains("[REDACTED]"));

        let consumed = client.consume(&invite, JOINER, NOW + 1).await.unwrap();
        assert_eq!(consumed, payload);
        assert_eq!(
            client.consume(&invite, JOINER, NOW + 2).await,
            Err(RendezvousError::AlreadyConsumed)
        );
    }

    #[tokio::test]
    async fn expiration_size_limits_and_ciphertext_tampering_are_typed() {
        let server = TestServer::default();
        let slot = RendezvousSlot::generate().unwrap();
        let short_invite = invite(&slot, 2);
        let client = RendezvousClient::new(&server, 16 * 1024).unwrap();
        client
            .publish(&short_invite, &payload(), NOW)
            .await
            .unwrap();
        assert_eq!(
            client.consume(&short_invite, JOINER, NOW + 2).await,
            Err(RendezvousError::Expired)
        );

        let small_client = RendezvousClient::new(&server, 32).unwrap();
        let fresh_slot = RendezvousSlot::generate().unwrap();
        assert_eq!(
            small_client
                .publish(&invite(&fresh_slot, 60), &payload(), NOW)
                .await,
            Err(RendezvousError::EnvelopeTooLarge)
        );

        let tampered_slot = RendezvousSlot::generate().unwrap();
        let tampered_invite = invite(&tampered_slot, 60);
        client
            .publish(&tampered_invite, &payload(), NOW)
            .await
            .unwrap();
        server.tamper(&tampered_slot);
        assert_eq!(
            client.consume(&tampered_invite, JOINER, NOW + 1).await,
            Err(RendezvousError::Decryption)
        );
    }

    #[tokio::test]
    async fn http_transport_is_bounded_redacted_and_maps_admission_statuses() {
        let slot = RendezvousSlot::generate().unwrap();
        let invite = invite(&slot, 300);
        let payload = payload();
        let envelope = seal(&invite, &slot, &payload).unwrap();
        let envelope_json = serde_json::to_vec(&envelope).unwrap();
        let responses = vec![
            http_response(201, b""),
            http_response(200, &envelope_json),
            http_response(429, b""),
            http_response(409, b""),
        ];
        let (base_url, requests, server) = http_fixture(responses).await;
        let transport = HttpRendezvousTransport::new_for_test(&base_url).unwrap();
        let client = RendezvousClient::new(&transport, 16 * 1024).unwrap();

        client.publish(&invite, &payload, NOW).await.unwrap();
        assert_eq!(
            client.consume(&invite, JOINER, NOW + 1).await.unwrap(),
            payload
        );
        let capability =
            AdmissionCapability::derive(invite.secret_bytes(), invite.session_id()).unwrap();
        assert_eq!(
            transport
                .publish(
                    slot.expose(),
                    PublishRequest {
                        envelope: envelope.clone(),
                        admission: AdmissionRegistration {
                            endpoint_id: OWNER.to_owned(),
                            capability: capability.encode(),
                        },
                    },
                )
                .await,
            Err(RendezvousError::RateLimited)
        );
        let proof = capability
            .join_proof(slot.expose(), JOINER, invite.expires_at_unix())
            .unwrap();
        assert_eq!(
            transport
                .consume(
                    slot.expose(),
                    ConsumeRequest {
                        endpoint_id: JOINER.to_owned(),
                        proof,
                    },
                )
                .await,
            Err(RendezvousError::AlreadyConsumed)
        );
        server.await.unwrap();

        let requests = requests.lock().await;
        let publish = String::from_utf8_lossy(&requests[0]);
        assert!(publish.starts_with(&format!("PUT /v1/slots/{} ", slot.expose())));
        assert!(publish.contains(OWNER));
        assert!(!publish.contains("203.0.113.7"));
        assert!(!publish.contains(&hex_encode(invite.secret_bytes())));
        assert!(String::from_utf8_lossy(&requests[1])
            .starts_with(&format!("POST /v1/slots/{}/consume ", slot.expose())));
    }

    #[tokio::test]
    async fn http_transport_rejects_chunked_or_declared_oversized_responses() {
        let slot = RendezvousSlot::generate().unwrap();
        let oversized = vec![b'x'; MAX_HTTP_RESPONSE_BYTES + 1];
        let (base_url, _, server) = http_fixture(vec![http_response(200, &oversized)]).await;
        let transport = HttpRendezvousTransport::new_for_test(&base_url).unwrap();
        let capability = AdmissionCapability::derive(&[9; 32], "session-a").unwrap();
        let request = ConsumeRequest {
            endpoint_id: JOINER.to_owned(),
            proof: capability
                .join_proof(slot.expose(), JOINER, NOW + 300)
                .unwrap(),
        };

        assert_eq!(
            transport.consume(slot.expose(), request).await,
            Err(RendezvousError::EnvelopeTooLarge)
        );
        server.await.unwrap();
        assert!(HttpRendezvousTransport::new("http://example.com/v1").is_err());
        assert!(HttpRendezvousTransport::new("https://user@example.com/v1").is_err());
    }

    async fn http_fixture(
        responses: Vec<Vec<u8>>,
    ) -> (
        String,
        Arc<tokio::sync::Mutex<Vec<Vec<u8>>>>,
        tokio::task::JoinHandle<()>,
    ) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let requests = Arc::new(tokio::sync::Mutex::new(Vec::new()));
        let captured = Arc::clone(&requests);
        let server = tokio::spawn(async move {
            for response in responses {
                let (mut stream, _) = listener.accept().await.unwrap();
                let request = read_http_request(&mut stream).await;
                captured.lock().await.push(request);
                stream.write_all(&response).await.unwrap();
                stream.shutdown().await.unwrap();
            }
        });
        (format!("http://{address}/v1"), requests, server)
    }

    async fn read_http_request(stream: &mut tokio::net::TcpStream) -> Vec<u8> {
        let mut request = Vec::new();
        let mut buffer = [0_u8; 4096];
        let (header_end, content_length) = loop {
            let read = stream.read(&mut buffer).await.unwrap();
            assert!(read > 0);
            request.extend_from_slice(&buffer[..read]);
            if let Some(header_end) = request.windows(4).position(|part| part == b"\r\n\r\n") {
                let header_end = header_end + 4;
                let headers = String::from_utf8_lossy(&request[..header_end]);
                let content_length = headers
                    .lines()
                    .find_map(|line| {
                        line.split_once(':').and_then(|(name, value)| {
                            name.eq_ignore_ascii_case("content-length")
                                .then(|| value.trim().parse::<usize>().unwrap())
                        })
                    })
                    .unwrap_or(0);
                break (header_end, content_length);
            }
        };
        while request.len() < header_end + content_length {
            let read = stream.read(&mut buffer).await.unwrap();
            assert!(read > 0);
            request.extend_from_slice(&buffer[..read]);
        }
        request
    }

    fn http_response(status: u16, body: &[u8]) -> Vec<u8> {
        let reason = match status {
            200 => "OK",
            201 => "Created",
            409 => "Conflict",
            429 => "Too Many Requests",
            _ => "Error",
        };
        let mut response = format!(
            "HTTP/1.1 {status} {reason}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            body.len()
        )
        .into_bytes();
        response.extend_from_slice(body);
        response
    }

    fn invite(slot: &RendezvousSlot, ttl: u64) -> InternetInvite {
        InternetInvite::new(
            OWNER,
            vec!["ip:203.0.113.7:4242".to_owned()],
            Some(slot.expose().to_owned()),
            NOW,
            ttl,
        )
        .unwrap()
    }

    fn payload() -> RendezvousPayload {
        RendezvousPayload::new(OWNER, vec!["ip:203.0.113.7:4242".to_owned()], [0x51; 32]).unwrap()
    }

    #[derive(Default)]
    struct TestServer {
        state: Mutex<ServerState>,
    }

    #[derive(Default)]
    struct ServerState {
        records: HashMap<String, (RendezvousEnvelope, AdmissionCapability)>,
        consumed: HashSet<String>,
        observed: Option<(String, RendezvousEnvelope)>,
    }

    impl TestServer {
        fn observed(&self) -> (String, RendezvousEnvelope) {
            self.state.lock().unwrap().observed.clone().unwrap()
        }

        fn tamper(&self, slot: &RendezvousSlot) {
            self.state
                .lock()
                .unwrap()
                .records
                .get_mut(slot.expose())
                .unwrap()
                .0
                .tamper_for_test();
        }
    }

    impl RendezvousTransport for TestServer {
        fn publish<'a>(
            &'a self,
            slot: &'a str,
            request: PublishRequest<RendezvousEnvelope>,
        ) -> Pin<Box<dyn Future<Output = Result<(), RendezvousError>> + Send + 'a>> {
            Box::pin(async move {
                let mut state = self.state.lock().unwrap();
                if state.records.contains_key(slot) || state.consumed.contains(slot) {
                    return Err(RendezvousError::SlotUnavailable);
                }
                let capability = AdmissionCapability::decode(&request.admission.capability)
                    .map_err(|_| RendezvousError::AdmissionDenied)?;
                state.observed = Some((slot.to_owned(), request.envelope.clone()));
                state
                    .records
                    .insert(slot.to_owned(), (request.envelope, capability));
                Ok(())
            })
        }

        fn consume<'a>(
            &'a self,
            slot: &'a str,
            request: ConsumeRequest,
        ) -> Pin<Box<dyn Future<Output = Result<RendezvousEnvelope, RendezvousError>> + Send + 'a>>
        {
            Box::pin(async move {
                let mut state = self.state.lock().unwrap();
                if state.consumed.contains(slot) {
                    return Err(RendezvousError::AlreadyConsumed);
                }
                let (envelope, capability) =
                    state.records.get(slot).ok_or(RendezvousError::NotFound)?;
                capability
                    .verify_join_proof(
                        slot,
                        &request.endpoint_id,
                        envelope.expires_at_unix,
                        &request.proof,
                    )
                    .map_err(|_| RendezvousError::AdmissionDenied)?;
                let (envelope, _) = state
                    .records
                    .remove(slot)
                    .ok_or(RendezvousError::NotFound)?;
                state.consumed.insert(slot.to_owned());
                Ok(envelope)
            })
        }
    }
}

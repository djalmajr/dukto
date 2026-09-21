use std::{
    collections::HashMap,
    net::IpAddr,
    sync::{Arc, Mutex},
    time::{SystemTime, UNIX_EPOCH},
};

use axum::{
    body::Body,
    extract::{DefaultBodyLimit, Path, State},
    http::{
        header::{AUTHORIZATION, CACHE_CONTROL},
        HeaderMap, HeaderValue, Request, StatusCode,
    },
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post, put},
    Json, Router,
};
use dukto_admission::{
    is_valid_endpoint_id, is_valid_slot, AdmissionCapability, ConsumeRequest, PublishRequest,
    MAX_ADMISSION_TTL_SECS,
};
use ring::hmac;
use serde::{Deserialize, Serialize};
use thiserror::Error;

const MAX_BODY_BYTES: usize = 20 * 1024;
const MAX_CIPHERTEXT_BYTES: usize = 16 * 1024;
const MAX_PUBLIC_REQUESTS_PER_MINUTE: u32 = 20;
// iroh-relay 1.2 uses this legacy wire name even though the value is an EndpointId.
const RELAY_ENDPOINT_HEADER: &str = "x-iroh-nodeid";
const FORWARDED_FOR_HEADER: &str = "x-forwarded-for";
const RELAY_BEARER_PROOF_MESSAGE: &[u8] = b"dukto/gateway/v1/relay-bearer";

pub trait Clock: Send + Sync + 'static {
    fn now_unix(&self) -> u64;
}

#[derive(Debug)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now_unix(&self) -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |duration| duration.as_secs())
    }
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RendezvousEnvelope {
    pub version: u8,
    pub expires_at_unix: u64,
    pub nonce: [u8; 12],
    pub ciphertext: Vec<u8>,
}

impl std::fmt::Debug for RendezvousEnvelope {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("RendezvousEnvelope")
            .field("version", &self.version)
            .field("expires_at_unix", &self.expires_at_unix)
            .field("nonce", &"[REDACTED]")
            .field("ciphertext_bytes", &self.ciphertext.len())
            .finish()
    }
}

#[derive(Clone)]
pub struct GatewayState {
    inner: Arc<Mutex<Store>>,
    relay_bearer_proof: [u8; 32],
    clock: Arc<dyn Clock>,
}

impl std::fmt::Debug for GatewayState {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("GatewayState")
            .field("relay_bearer_proof", &"[REDACTED]")
            .finish_non_exhaustive()
    }
}

#[derive(Default)]
struct Store {
    slots: HashMap<String, SlotRecord>,
    consumed: HashMap<String, u64>,
    admissions: HashMap<String, u64>,
    rate_limits: HashMap<IpAddr, RateWindow>,
}

struct SlotRecord {
    envelope: RendezvousEnvelope,
    capability: AdmissionCapability,
}

#[derive(Clone, Copy)]
struct RateWindow {
    minute: u64,
    count: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum GatewayConfigurationError {
    #[error("gateway relay bearer is invalid")]
    InvalidRelayBearer,
}

impl GatewayState {
    pub fn new(
        relay_bearer: impl Into<Vec<u8>>,
        clock: Arc<dyn Clock>,
    ) -> Result<Self, GatewayConfigurationError> {
        let relay_bearer = relay_bearer.into();
        if relay_bearer.len() < 32
            || relay_bearer.len() > 512
            || !relay_bearer.iter().all(u8::is_ascii_graphic)
        {
            return Err(GatewayConfigurationError::InvalidRelayBearer);
        }
        let key = hmac::Key::new(hmac::HMAC_SHA256, &relay_bearer);
        let relay_bearer_proof = hmac::sign(&key, RELAY_BEARER_PROOF_MESSAGE)
            .as_ref()
            .try_into()
            .map_err(|_| GatewayConfigurationError::InvalidRelayBearer)?;
        Ok(Self {
            inner: Arc::new(Mutex::new(Store::default())),
            relay_bearer_proof,
            clock,
        })
    }

    fn now(&self) -> u64 {
        self.clock.now_unix()
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Store> {
        self.inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

pub fn router(state: GatewayState) -> Router {
    Router::new()
        .route("/healthz", get(health))
        .route("/v1/slots/{slot}", put(publish))
        .route("/v1/slots/{slot}/consume", post(consume))
        .route("/v1/relay/authorize", post(authorize_relay))
        .layer(DefaultBodyLimit::max(MAX_BODY_BYTES))
        .layer(middleware::from_fn(no_store))
        .with_state(state)
}

async fn no_store(request: Request<Body>, next: Next) -> Response {
    let mut response = next.run(request).await;
    response
        .headers_mut()
        .insert(CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
}

async fn health() -> &'static str {
    "ok"
}

async fn publish(
    State(state): State<GatewayState>,
    Path(slot): Path<String>,
    headers: HeaderMap,
    Json(request): Json<PublishRequest<RendezvousEnvelope>>,
) -> Result<StatusCode, ApiError> {
    validate_public_request(&state, &headers)?;
    if !is_valid_slot(&slot)
        || !is_valid_endpoint_id(&request.admission.endpoint_id)
        || request.envelope.version != 1
        || request.envelope.nonce.iter().all(|byte| *byte == 0)
        || request.envelope.ciphertext.is_empty()
        || request.envelope.ciphertext.len() > MAX_CIPHERTEXT_BYTES
    {
        return Err(ApiError::Invalid);
    }
    let now = state.now();
    let expires_at = request.envelope.expires_at_unix;
    if expires_at <= now {
        return Err(ApiError::Expired);
    }
    if expires_at - now > MAX_ADMISSION_TTL_SECS {
        return Err(ApiError::Invalid);
    }
    let capability = AdmissionCapability::decode(&request.admission.capability)
        .map_err(|_| ApiError::Invalid)?;

    let mut store = state.lock();
    cleanup(&mut store, now);
    if store.slots.contains_key(&slot) || store.consumed.contains_key(&slot) {
        return Err(ApiError::Conflict);
    }
    store
        .admissions
        .insert(request.admission.endpoint_id.clone(), expires_at);
    tracing::debug!(
        endpoint_prefix = &request.admission.endpoint_id[..10],
        "invitation owner admitted"
    );
    store.slots.insert(
        slot,
        SlotRecord {
            envelope: request.envelope,
            capability,
        },
    );
    Ok(StatusCode::CREATED)
}

async fn consume(
    State(state): State<GatewayState>,
    Path(slot): Path<String>,
    headers: HeaderMap,
    Json(request): Json<ConsumeRequest>,
) -> Result<Json<RendezvousEnvelope>, ApiError> {
    validate_public_request(&state, &headers)?;
    if !is_valid_slot(&slot) || !is_valid_endpoint_id(&request.endpoint_id) {
        return Err(ApiError::Invalid);
    }
    let now = state.now();
    let mut store = state.lock();
    cleanup(&mut store, now);
    if store.consumed.contains_key(&slot) {
        return Err(ApiError::Conflict);
    }
    let record = store.slots.get(&slot).ok_or(ApiError::NotFound)?;
    let expires_at = record.envelope.expires_at_unix;
    record
        .capability
        .verify_join_proof(&slot, &request.endpoint_id, expires_at, &request.proof)
        .map_err(|_| ApiError::Forbidden)?;

    let record = store.slots.remove(&slot).ok_or(ApiError::NotFound)?;
    store.consumed.insert(slot, expires_at);
    store
        .admissions
        .insert(request.endpoint_id.clone(), expires_at);
    Ok(Json(record.envelope))
}

async fn authorize_relay(
    State(state): State<GatewayState>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let supplied = headers
        .get(AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .ok_or(ApiError::Unauthorized)?;
    let supplied_key = hmac::Key::new(hmac::HMAC_SHA256, supplied.as_bytes());
    hmac::verify(
        &supplied_key,
        RELAY_BEARER_PROOF_MESSAGE,
        &state.relay_bearer_proof,
    )
    .map_err(|_| ApiError::Unauthorized)?;

    let endpoint_id = headers
        .get(RELAY_ENDPOINT_HEADER)
        .and_then(|value| value.to_str().ok())
        .ok_or(ApiError::Invalid)?;
    tracing::debug!(
        endpoint_prefix = endpoint_id.get(..10).unwrap_or("invalid"),
        endpoint_length = endpoint_id.len(),
        endpoint_is_valid = is_valid_endpoint_id(endpoint_id),
        "relay authorization request"
    );
    if !is_valid_endpoint_id(endpoint_id) {
        return Err(ApiError::Invalid);
    }
    let now = state.now();
    let mut store = state.lock();
    cleanup(&mut store, now);
    let admitted = store
        .admissions
        .get(endpoint_id)
        .is_some_and(|expires_at| *expires_at > now);
    Ok(Response::builder()
        .status(StatusCode::OK)
        .header("content-type", "text/plain; charset=utf-8")
        .body(Body::from(if admitted { "true" } else { "false" }))
        .expect("static relay authorization response is valid"))
}

fn validate_public_request(state: &GatewayState, headers: &HeaderMap) -> Result<(), ApiError> {
    let address = headers
        .get(FORWARDED_FOR_HEADER)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split(',').next())
        .map(str::trim)
        .and_then(|value| value.parse::<IpAddr>().ok())
        .ok_or(ApiError::Invalid)?;
    let now = state.now();
    let minute = now / 60;
    let mut store = state.lock();
    cleanup(&mut store, now);
    let window = store
        .rate_limits
        .entry(address)
        .or_insert(RateWindow { minute, count: 0 });
    if window.minute != minute {
        *window = RateWindow { minute, count: 0 };
    }
    if window.count >= MAX_PUBLIC_REQUESTS_PER_MINUTE {
        return Err(ApiError::RateLimited);
    }
    window.count += 1;
    Ok(())
}

fn cleanup(store: &mut Store, now: u64) {
    store
        .slots
        .retain(|_, record| record.envelope.expires_at_unix > now);
    store.consumed.retain(|_, expires_at| *expires_at > now);
    store.admissions.retain(|_, expires_at| *expires_at > now);
    let current_minute = now / 60;
    store
        .rate_limits
        .retain(|_, window| window.minute.saturating_add(1) >= current_minute);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ApiError {
    Invalid,
    Unauthorized,
    Forbidden,
    NotFound,
    Conflict,
    Expired,
    RateLimited,
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let status = match self {
            Self::Invalid => StatusCode::BAD_REQUEST,
            Self::Unauthorized => StatusCode::UNAUTHORIZED,
            Self::Forbidden => StatusCode::FORBIDDEN,
            Self::NotFound => StatusCode::NOT_FOUND,
            Self::Conflict => StatusCode::CONFLICT,
            Self::Expired => StatusCode::GONE,
            Self::RateLimited => StatusCode::TOO_MANY_REQUESTS,
        };
        status.into_response()
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicU64, Ordering};

    use axum::http::{Request, StatusCode};
    use dukto_admission::{AdmissionRegistration, PublishRequest};
    use http_body_util::BodyExt;
    use serde_json::json;
    use tower::ServiceExt;

    use super::*;

    const TOKEN: &str = "0123456789abcdef0123456789abcdef";
    const SLOT: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const OWNER: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
    const JOINER: &str = "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc";

    struct TestClock(AtomicU64);

    impl TestClock {
        fn new(now: u64) -> Self {
            Self(AtomicU64::new(now))
        }

        fn set(&self, now: u64) {
            self.0.store(now, Ordering::SeqCst);
        }
    }

    impl Clock for TestClock {
        fn now_unix(&self) -> u64 {
            self.0.load(Ordering::SeqCst)
        }
    }

    fn request(method: &str, uri: &str, body: serde_json::Value) -> Request<Body> {
        Request::builder()
            .method(method)
            .uri(uri)
            .header("content-type", "application/json")
            .header(FORWARDED_FOR_HEADER, "192.0.2.10")
            .body(Body::from(body.to_string()))
            .unwrap()
    }

    fn relay_request(endpoint: &str, token: &str) -> Request<Body> {
        Request::builder()
            .method("POST")
            .uri("/v1/relay/authorize")
            .header(AUTHORIZATION, format!("Bearer {token}"))
            .header(RELAY_ENDPOINT_HEADER, endpoint)
            .body(Body::empty())
            .unwrap()
    }

    fn publish_body(capability: &AdmissionCapability, expires_at: u64) -> serde_json::Value {
        serde_json::to_value(PublishRequest {
            envelope: RendezvousEnvelope {
                version: 1,
                expires_at_unix: expires_at,
                nonce: [1; 12],
                ciphertext: vec![7; 64],
            },
            admission: AdmissionRegistration {
                endpoint_id: OWNER.to_owned(),
                capability: capability.encode(),
            },
        })
        .unwrap()
    }

    #[tokio::test]
    async fn publish_consume_and_relay_authorization_are_atomic() {
        let clock = Arc::new(TestClock::new(1_800_000_000));
        let app = router(GatewayState::new(TOKEN, clock).unwrap());
        let capability = AdmissionCapability::derive(&[9; 32], "session-a").unwrap();
        let expires_at = 1_800_000_300;

        let response = app
            .clone()
            .oneshot(request(
                "PUT",
                &format!("/v1/slots/{SLOT}"),
                publish_body(&capability, expires_at),
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::CREATED);

        let owner = app
            .clone()
            .oneshot(relay_request(OWNER, TOKEN))
            .await
            .unwrap();
        assert_eq!(owner.status(), StatusCode::OK);
        assert_eq!(
            owner.into_body().collect().await.unwrap().to_bytes(),
            "true"
        );

        let proof = capability.join_proof(SLOT, JOINER, expires_at).unwrap();
        let response = app
            .clone()
            .oneshot(request(
                "POST",
                &format!("/v1/slots/{SLOT}/consume"),
                json!({ "endpoint_id": JOINER, "proof": proof }),
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let replay = app
            .clone()
            .oneshot(request(
                "POST",
                &format!("/v1/slots/{SLOT}/consume"),
                json!({ "endpoint_id": JOINER, "proof": "invalid" }),
            ))
            .await
            .unwrap();
        assert_eq!(replay.status(), StatusCode::CONFLICT);

        let joiner = app.oneshot(relay_request(JOINER, TOKEN)).await.unwrap();
        assert_eq!(
            joiner.into_body().collect().await.unwrap().to_bytes(),
            "true"
        );
    }

    #[tokio::test]
    async fn wrong_proof_and_unknown_endpoint_fail_closed() {
        let clock = Arc::new(TestClock::new(1_800_000_000));
        let app = router(GatewayState::new(TOKEN, clock).unwrap());
        let capability = AdmissionCapability::derive(&[9; 32], "session-a").unwrap();
        let expires_at = 1_800_000_300;
        app.clone()
            .oneshot(request(
                "PUT",
                &format!("/v1/slots/{SLOT}"),
                publish_body(&capability, expires_at),
            ))
            .await
            .unwrap();

        let response = app
            .clone()
            .oneshot(request(
                "POST",
                &format!("/v1/slots/{SLOT}/consume"),
                json!({ "endpoint_id": JOINER, "proof": "wrong" }),
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);

        let unknown = app.oneshot(relay_request(JOINER, TOKEN)).await.unwrap();
        assert_eq!(
            unknown.into_body().collect().await.unwrap().to_bytes(),
            "false"
        );
    }

    #[tokio::test]
    async fn concurrent_consume_succeeds_exactly_once() {
        let clock = Arc::new(TestClock::new(1_800_000_000));
        let app = router(GatewayState::new(TOKEN, clock).unwrap());
        let capability = AdmissionCapability::derive(&[9; 32], "session-a").unwrap();
        let expires_at = 1_800_000_300;
        app.clone()
            .oneshot(request(
                "PUT",
                &format!("/v1/slots/{SLOT}"),
                publish_body(&capability, expires_at),
            ))
            .await
            .unwrap();
        let proof = capability.join_proof(SLOT, JOINER, expires_at).unwrap();
        let first = app.clone().oneshot(request(
            "POST",
            &format!("/v1/slots/{SLOT}/consume"),
            json!({ "endpoint_id": JOINER, "proof": proof.clone() }),
        ));
        let second = app.clone().oneshot(request(
            "POST",
            &format!("/v1/slots/{SLOT}/consume"),
            json!({ "endpoint_id": JOINER, "proof": proof }),
        ));
        let (first, second) = tokio::join!(first, second);
        let statuses = [first.unwrap().status(), second.unwrap().status()];
        assert_eq!(
            statuses
                .iter()
                .filter(|status| **status == StatusCode::OK)
                .count(),
            1
        );
        assert_eq!(
            statuses
                .iter()
                .filter(|status| **status == StatusCode::CONFLICT)
                .count(),
            1
        );
    }

    #[tokio::test]
    async fn oversized_body_is_rejected_without_parsing() {
        let clock = Arc::new(TestClock::new(1_800_000_000));
        let app = router(GatewayState::new(TOKEN, clock).unwrap());
        let response = app
            .oneshot(
                Request::builder()
                    .method("PUT")
                    .uri(format!("/v1/slots/{SLOT}"))
                    .header("content-type", "application/json")
                    .header(FORWARDED_FOR_HEADER, "192.0.2.10")
                    .body(Body::from(vec![b'x'; MAX_BODY_BYTES + 1]))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
        assert_eq!(response.headers().get(CACHE_CONTROL).unwrap(), "no-store");
    }

    #[tokio::test]
    async fn expiry_and_m2m_authentication_fail_closed() {
        let clock = Arc::new(TestClock::new(1_800_000_000));
        let app = router(GatewayState::new(TOKEN, clock.clone()).unwrap());
        let capability = AdmissionCapability::derive(&[9; 32], "session-a").unwrap();
        app.clone()
            .oneshot(request(
                "PUT",
                &format!("/v1/slots/{SLOT}"),
                publish_body(&capability, 1_800_000_001),
            ))
            .await
            .unwrap();
        clock.set(1_800_000_002);

        let expired = app
            .clone()
            .oneshot(relay_request(OWNER, TOKEN))
            .await
            .unwrap();
        assert_eq!(
            expired.into_body().collect().await.unwrap().to_bytes(),
            "false"
        );
        let unauthorized = app.oneshot(relay_request(OWNER, "wrong")).await.unwrap();
        assert_eq!(unauthorized.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn rate_limit_is_enforced() {
        let clock = Arc::new(TestClock::new(1_800_000_000));
        let app = router(GatewayState::new(TOKEN, clock).unwrap());
        let capability = AdmissionCapability::derive(&[9; 32], "session-a").unwrap();
        for index in 0..MAX_PUBLIC_REQUESTS_PER_MINUTE {
            let slot = format!("{index:064x}");
            let response = app
                .clone()
                .oneshot(request(
                    "PUT",
                    &format!("/v1/slots/{slot}"),
                    publish_body(&capability, 1_800_000_300),
                ))
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::CREATED);
        }
        let response = app
            .oneshot(request(
                "PUT",
                &format!("/v1/slots/{SLOT}"),
                publish_body(&capability, 1_800_000_300),
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
    }
}

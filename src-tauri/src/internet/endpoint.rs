use std::{fmt, net::SocketAddr, time::Duration};

#[cfg(test)]
use iroh::tls::CaTlsConfig;
use iroh::{endpoint::presets, Endpoint, EndpointAddr, EndpointId, RelayMap, RelayMode, SecretKey};
use thiserror::Error;
use tokio::time::timeout;

use crate::internet::{channel::IrohAuthenticatedChannel, pairing::DUKTO_INTERNET_ALPN};

const DEFAULT_HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(10);
const MAX_HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(60);
const MAX_CONNECT_ATTEMPTS: usize = 8;
const MAX_CONFIGURED_RELAYS: usize = 8;
const RELAY_URLS_ENV: &str = "DUKTO_RELAY_URLS";
const FORCE_RELAY_ONLY_ENV: &str = "DUKTO_FORCE_RELAY_ONLY";

pub struct InternetEndpointConfig {
    secret_key: Option<SecretKey>,
    bind_addr: Option<SocketAddr>,
    handshake_timeout: Duration,
    relay_map: Option<RelayMap>,
    direct_transports: bool,
    #[cfg(test)]
    insecure_relay_tls: bool,
}

impl InternetEndpointConfig {
    pub fn from_environment() -> Result<Self, InternetEndpointError> {
        let relay_urls = optional_unicode_environment(RELAY_URLS_ENV)?;
        let force_relay_only = optional_unicode_environment(FORCE_RELAY_ONLY_ENV)?;
        config_from_values(relay_urls.as_deref(), force_relay_only.as_deref())
    }

    pub fn with_secret_key(mut self, secret_key: SecretKey) -> Self {
        self.secret_key = Some(secret_key);
        self
    }

    pub fn with_bind_addr(mut self, bind_addr: SocketAddr) -> Self {
        self.bind_addr = Some(bind_addr);
        self
    }

    pub fn with_handshake_timeout(mut self, handshake_timeout: Duration) -> Self {
        self.handshake_timeout = handshake_timeout;
        self
    }

    pub fn with_relay_map(mut self, relay_map: RelayMap) -> Self {
        self.relay_map = Some(relay_map);
        self
    }

    #[cfg(test)]
    pub(crate) fn without_direct_transports_for_test(mut self) -> Self {
        self.direct_transports = false;
        self
    }

    #[cfg(test)]
    pub(crate) fn with_insecure_relay_tls_for_test(mut self) -> Self {
        self.insecure_relay_tls = true;
        self
    }
}

fn optional_unicode_environment(name: &str) -> Result<Option<String>, InternetEndpointError> {
    match std::env::var(name) {
        Ok(value) => Ok(Some(value)),
        Err(std::env::VarError::NotPresent) => Ok(None),
        Err(std::env::VarError::NotUnicode(_)) => Err(InternetEndpointError::InvalidConfiguration),
    }
}

fn config_from_values(
    relay_urls: Option<&str>,
    force_relay_only: Option<&str>,
) -> Result<InternetEndpointConfig, InternetEndpointError> {
    let mut config = InternetEndpointConfig::default();
    if let Some(relay_urls) = relay_urls {
        config = config.with_relay_map(relay_map_from_csv(relay_urls)?);
    }

    let force_relay_only = match force_relay_only {
        None | Some("0" | "false") => false,
        Some("1" | "true") => true,
        Some(_) => return Err(InternetEndpointError::InvalidConfiguration),
    };
    if force_relay_only {
        if config.relay_map.is_none() {
            return Err(InternetEndpointError::InvalidConfiguration);
        }
        config.direct_transports = false;
    }
    Ok(config)
}

fn relay_map_from_csv(value: &str) -> Result<RelayMap, InternetEndpointError> {
    let relay_urls = value
        .split(',')
        .map(str::trim)
        .filter(|url| !url.is_empty())
        .collect::<Vec<_>>();
    if relay_urls.is_empty()
        || relay_urls.len() > MAX_CONFIGURED_RELAYS
        || relay_urls.iter().any(|url| !url.starts_with("https://"))
    {
        return Err(InternetEndpointError::InvalidConfiguration);
    }
    RelayMap::try_from_iter(relay_urls).map_err(|_| InternetEndpointError::InvalidConfiguration)
}

impl Default for InternetEndpointConfig {
    fn default() -> Self {
        Self {
            secret_key: None,
            bind_addr: None,
            handshake_timeout: DEFAULT_HANDSHAKE_TIMEOUT,
            relay_map: None,
            direct_transports: true,
            #[cfg(test)]
            insecure_relay_tls: false,
        }
    }
}

impl fmt::Debug for InternetEndpointConfig {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("InternetEndpointConfig")
            .field(
                "secret_key",
                &self.secret_key.as_ref().map(|_| "[REDACTED]"),
            )
            .field("bind_addr", &self.bind_addr)
            .field("handshake_timeout", &self.handshake_timeout)
            .field("has_relay_map", &self.relay_map.is_some())
            .field("direct_transports", &self.direct_transports)
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum InternetEndpointError {
    #[error("internet endpoint configuration is invalid")]
    InvalidConfiguration,
    #[error("internet endpoint bind address is invalid")]
    InvalidBindAddress,
    #[error("internet endpoint could not bind")]
    Bind,
    #[error("internet endpoint is closed")]
    Closed,
    #[error("internet endpoint handshake timed out")]
    HandshakeTimeout,
    #[error("internet endpoint did not become relay-reachable before timeout")]
    OnlineTimeout,
    #[error("internet endpoint connection failed")]
    Connect,
    #[error("internet endpoint exhausted {attempts} bounded connection attempts")]
    ConnectAttemptsExhausted { attempts: usize },
    #[error("internet endpoint accept failed")]
    Accept,
    #[error("internet endpoint identity does not match the invitation")]
    ExpectedEndpointMismatch,
}

#[derive(Clone)]
pub struct InternetEndpoint {
    endpoint: Endpoint,
    handshake_timeout: Duration,
    relay_enabled: bool,
}

pub struct VerifiedInternetConnection {
    pub channel: IrohAuthenticatedChannel,
    pub attempts: usize,
}

impl fmt::Debug for VerifiedInternetConnection {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("VerifiedInternetConnection")
            .field("attempts", &self.attempts)
            .field("channel", &"[AUTHENTICATED]")
            .finish()
    }
}

impl InternetEndpoint {
    pub async fn bind(config: InternetEndpointConfig) -> Result<Self, InternetEndpointError> {
        if config.handshake_timeout.is_zero() || config.handshake_timeout > MAX_HANDSHAKE_TIMEOUT {
            return Err(InternetEndpointError::InvalidConfiguration);
        }

        let relay_enabled = config
            .relay_map
            .as_ref()
            .is_some_and(|relay_map| !relay_map.is_empty());
        let mut builder =
            Endpoint::builder(presets::Minimal).alpns(vec![DUKTO_INTERNET_ALPN.to_vec()]);
        builder = match config.relay_map {
            Some(relay_map) => builder.relay_mode(RelayMode::Custom(relay_map)),
            None => builder.clear_relay_transports(),
        };
        if !config.direct_transports {
            builder = builder.clear_ip_transports();
        }
        #[cfg(test)]
        if config.insecure_relay_tls {
            builder = builder.ca_tls_config(CaTlsConfig::insecure_skip_verify());
        }
        if let Some(secret_key) = config.secret_key {
            builder = builder.secret_key(secret_key);
        }
        if let Some(bind_addr) = config.bind_addr {
            builder = builder
                .clear_ip_transports()
                .bind_addr(bind_addr)
                .map_err(|_| InternetEndpointError::InvalidBindAddress)?;
        }

        let endpoint = builder
            .bind()
            .await
            .map_err(|_| InternetEndpointError::Bind)?;
        Ok(Self {
            endpoint,
            handshake_timeout: config.handshake_timeout,
            relay_enabled,
        })
    }

    pub fn endpoint_id(&self) -> EndpointId {
        self.endpoint.id()
    }

    pub fn address(&self) -> EndpointAddr {
        self.endpoint.addr()
    }

    pub fn relay_enabled(&self) -> bool {
        self.relay_enabled
    }

    pub async fn connect_verified(
        &self,
        remote_address: EndpointAddr,
        expected_endpoint: EndpointId,
    ) -> Result<IrohAuthenticatedChannel, InternetEndpointError> {
        if remote_address.id != expected_endpoint {
            return Err(InternetEndpointError::ExpectedEndpointMismatch);
        }
        let connection = timeout(
            self.handshake_timeout,
            self.endpoint.connect(remote_address, DUKTO_INTERNET_ALPN),
        )
        .await
        .map_err(|_| InternetEndpointError::HandshakeTimeout)?
        .map_err(|_| InternetEndpointError::Connect)?;
        if connection.remote_id() != expected_endpoint {
            connection.close(1u32.into(), b"unexpected endpoint identity");
            return Err(InternetEndpointError::ExpectedEndpointMismatch);
        }
        Ok(IrohAuthenticatedChannel::new(connection))
    }

    pub async fn connect_verified_candidates(
        &self,
        candidates: impl IntoIterator<Item = EndpointAddr>,
        expected_endpoint: EndpointId,
        max_attempts: usize,
    ) -> Result<VerifiedInternetConnection, InternetEndpointError> {
        if max_attempts == 0 || max_attempts > MAX_CONNECT_ATTEMPTS {
            return Err(InternetEndpointError::InvalidConfiguration);
        }
        let candidates: Vec<_> = candidates.into_iter().collect();
        if candidates.is_empty()
            || candidates
                .iter()
                .any(|candidate| candidate.id != expected_endpoint)
        {
            return Err(InternetEndpointError::ExpectedEndpointMismatch);
        }

        let mut attempts = 0;
        for candidate in candidates.into_iter().take(max_attempts) {
            attempts += 1;
            match self.connect_verified(candidate, expected_endpoint).await {
                Ok(channel) => return Ok(VerifiedInternetConnection { channel, attempts }),
                Err(InternetEndpointError::ExpectedEndpointMismatch) => {
                    return Err(InternetEndpointError::ExpectedEndpointMismatch);
                }
                Err(_) => {}
            }
        }
        Err(InternetEndpointError::ConnectAttemptsExhausted { attempts })
    }

    pub async fn accept(&self) -> Result<IrohAuthenticatedChannel, InternetEndpointError> {
        let incoming = self
            .endpoint
            .accept()
            .await
            .ok_or(InternetEndpointError::Closed)?;
        let connection = timeout(self.handshake_timeout, incoming)
            .await
            .map_err(|_| InternetEndpointError::HandshakeTimeout)?
            .map_err(|_| InternetEndpointError::Accept)?;
        Ok(IrohAuthenticatedChannel::new(connection))
    }

    pub async fn wait_until_online(&self) -> Result<(), InternetEndpointError> {
        timeout(self.handshake_timeout, self.endpoint.online())
            .await
            .map_err(|_| InternetEndpointError::OnlineTimeout)?;
        Ok(())
    }

    pub async fn close(&self) {
        self.endpoint.close().await;
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use crate::transfer::channel::{AuthenticatedChannel, ChannelCloseReason, RouteKind};

    use super::{
        config_from_values, relay_map_from_csv, InternetEndpoint, InternetEndpointConfig,
        InternetEndpointError, MAX_CONFIGURED_RELAYS,
    };

    #[test]
    fn relay_configuration_requires_one_to_eight_https_urls() {
        let configured =
            relay_map_from_csv("https://relay-one.example, https://relay-two.example/custom-path")
                .unwrap();
        assert_eq!(configured.len(), 2);

        assert_eq!(
            relay_map_from_csv(""),
            Err(InternetEndpointError::InvalidConfiguration)
        );
        assert_eq!(
            relay_map_from_csv("http://relay.example"),
            Err(InternetEndpointError::InvalidConfiguration)
        );
        assert_eq!(
            relay_map_from_csv(&["https://relay.example"; MAX_CONFIGURED_RELAYS + 1].join(","),),
            Err(InternetEndpointError::InvalidConfiguration)
        );
    }

    #[test]
    fn relay_only_validation_mode_is_explicit_and_requires_a_relay() {
        let direct = config_from_values(None, None).unwrap();
        assert!(direct.direct_transports);
        assert!(direct.relay_map.is_none());

        let mixed = config_from_values(Some("https://relay.example"), Some("false")).unwrap();
        assert!(mixed.direct_transports);
        assert!(mixed.relay_map.is_some());

        let relay_only = config_from_values(Some("https://relay.example"), Some("true")).unwrap();
        assert!(!relay_only.direct_transports);
        assert!(relay_only.relay_map.is_some());

        assert_eq!(
            config_from_values(None, Some("true")).unwrap_err(),
            InternetEndpointError::InvalidConfiguration
        );
        assert_eq!(
            config_from_values(Some("https://relay.example"), Some("yes")).unwrap_err(),
            InternetEndpointError::InvalidConfiguration
        );
    }

    #[tokio::test]
    async fn production_endpoint_connects_directly_and_authenticates_both_ids() {
        let server = InternetEndpoint::bind(
            InternetEndpointConfig::default()
                .with_bind_addr("127.0.0.1:0".parse().unwrap())
                .with_handshake_timeout(Duration::from_secs(3)),
        )
        .await
        .expect("server endpoint");
        let client = InternetEndpoint::bind(
            InternetEndpointConfig::default()
                .with_bind_addr("127.0.0.1:0".parse().unwrap())
                .with_handshake_timeout(Duration::from_secs(3)),
        )
        .await
        .expect("client endpoint");

        let expected_server_id = server.endpoint_id();
        let expected_client_id = client.endpoint_id();
        let server_address = server.address();
        let accepting = tokio::spawn(async move {
            let channel = server.accept().await.expect("incoming direct channel");
            assert_eq!(channel.peer_id(), expected_client_id.to_string());
            assert_eq!(channel.route_kind(), RouteKind::Direct);
            let (mut send, mut receive) = channel.accept_bi().await.expect("incoming stream");
            let payload = receive.read_to_end(32).await.expect("request payload");
            assert_eq!(payload, b"dukto-direct");
            send.write_all(b"dukto-ack")
                .await
                .expect("response payload");
            send.finish().expect("finish response");
            send.stopped().await.expect("response acknowledged");
            channel.close(ChannelCloseReason::Completed);
            server.close().await;
        });

        let channel = client
            .connect_verified(server_address, expected_server_id)
            .await
            .expect("verified direct connection");
        assert_eq!(channel.peer_id(), expected_server_id.to_string());
        assert_eq!(channel.route_kind(), RouteKind::Direct);
        let (mut send, mut receive) = channel.open_bi().await.expect("outgoing stream");
        send.write_all(b"dukto-direct")
            .await
            .expect("request payload");
        send.finish().expect("finish request");
        assert_eq!(
            receive.read_to_end(32).await.expect("response payload"),
            b"dukto-ack"
        );

        accepting.await.expect("server task");
        client.close().await;
    }

    #[tokio::test]
    async fn expected_endpoint_mismatch_is_rejected_before_connecting() {
        let server = InternetEndpoint::bind(
            InternetEndpointConfig::default().with_bind_addr("127.0.0.1:0".parse().unwrap()),
        )
        .await
        .expect("server endpoint");
        let client = InternetEndpoint::bind(
            InternetEndpointConfig::default().with_bind_addr("127.0.0.1:0".parse().unwrap()),
        )
        .await
        .expect("client endpoint");
        let wrong_id = iroh::SecretKey::from_bytes(&[0xA5; 32]).public();

        let result = client.connect_verified(server.address(), wrong_id).await;
        assert!(matches!(
            result,
            Err(InternetEndpointError::ExpectedEndpointMismatch)
        ));

        client.close().await;
        server.close().await;
    }

    #[tokio::test]
    async fn custom_relay_fallback_reports_the_selected_relay_route() {
        let (relay_map, _relay_url, relay_server) = iroh::test_utils::run_relay_server()
            .await
            .expect("test relay");
        let server = InternetEndpoint::bind(
            InternetEndpointConfig::default()
                .with_relay_map(relay_map.clone())
                .without_direct_transports_for_test()
                .with_insecure_relay_tls_for_test(),
        )
        .await
        .expect("relay-only server endpoint");
        let client = InternetEndpoint::bind(
            InternetEndpointConfig::default()
                .with_relay_map(relay_map)
                .without_direct_transports_for_test()
                .with_insecure_relay_tls_for_test(),
        )
        .await
        .expect("relay-only client endpoint");
        server.wait_until_online().await.expect("server online");
        client.wait_until_online().await.expect("client online");

        let server_id = server.endpoint_id();
        let client_id = client.endpoint_id();
        let relay_address = server.address();
        assert!(relay_address
            .addrs
            .iter()
            .all(|address| matches!(address, iroh::TransportAddr::Relay(_))));

        let accepting = tokio::spawn(async move {
            let channel = server.accept().await.expect("relayed incoming channel");
            assert_eq!(channel.peer_id(), client_id.to_string());
            assert_eq!(channel.route_kind(), RouteKind::Relay);
            let (mut send, mut receive) = channel.accept_bi().await.expect("relayed stream");
            assert_eq!(receive.read_to_end(16).await.unwrap(), b"relay");
            send.write_all(b"ack").await.unwrap();
            send.finish().unwrap();
            send.stopped().await.unwrap();
            server.close().await;
        });

        let channel = client
            .connect_verified(relay_address, server_id)
            .await
            .expect("relayed verified connection");
        assert_eq!(channel.route_kind(), RouteKind::Relay);
        let (mut send, mut receive) = channel.open_bi().await.unwrap();
        send.write_all(b"relay").await.unwrap();
        send.finish().unwrap();
        assert_eq!(receive.read_to_end(16).await.unwrap(), b"ack");

        accepting.await.unwrap();
        client.close().await;
        drop(relay_server);
    }

    #[tokio::test]
    #[ignore = "requires an explicitly selected public HTTPS relay"]
    async fn public_https_relay_smoke_uses_production_tls_and_relay_only_mode() {
        let relay_url = std::env::var("DUKTO_PUBLIC_RELAY_SMOKE_URL")
            .expect("set DUKTO_PUBLIC_RELAY_SMOKE_URL to run the public-relay smoke test");
        let server = InternetEndpoint::bind(
            config_from_values(Some(&relay_url), Some("true"))
                .expect("valid relay-only configuration")
                .with_handshake_timeout(Duration::from_secs(30)),
        )
        .await
        .expect("relay-only server endpoint");
        let client = InternetEndpoint::bind(
            config_from_values(Some(&relay_url), Some("true"))
                .expect("valid relay-only configuration")
                .with_handshake_timeout(Duration::from_secs(30)),
        )
        .await
        .expect("relay-only client endpoint");
        server.wait_until_online().await.expect("server online");
        client.wait_until_online().await.expect("client online");

        let server_id = server.endpoint_id();
        let client_id = client.endpoint_id();
        let server_address = server.address();
        assert!(server_address
            .addrs
            .iter()
            .all(|address| matches!(address, iroh::TransportAddr::Relay(_))));

        let accepting = tokio::spawn(async move {
            let channel = server.accept().await.expect("public-relay connection");
            assert_eq!(channel.peer_id(), client_id.to_string());
            assert_eq!(channel.route_kind(), RouteKind::Relay);
            let (mut send, mut receive) = channel.accept_bi().await.expect("relayed stream");
            assert_eq!(
                receive.read_to_end(32).await.unwrap(),
                b"dukto-public-relay"
            );
            send.write_all(b"dukto-public-relay-ack").await.unwrap();
            send.finish().unwrap();
            send.stopped().await.unwrap();
            server.close().await;
        });

        let channel = client
            .connect_verified(server_address, server_id)
            .await
            .expect("verified public-relay connection");
        assert_eq!(channel.peer_id(), server_id.to_string());
        assert_eq!(channel.route_kind(), RouteKind::Relay);
        let (mut send, mut receive) = channel.open_bi().await.unwrap();
        send.write_all(b"dukto-public-relay").await.unwrap();
        send.finish().unwrap();
        assert_eq!(
            receive.read_to_end(32).await.unwrap(),
            b"dukto-public-relay-ack"
        );

        accepting.await.unwrap();
        client.close().await;
    }

    #[tokio::test]
    async fn direct_path_wins_when_direct_and_relay_are_both_available() {
        let (relay_map, _relay_url, relay_server) = iroh::test_utils::run_relay_server()
            .await
            .expect("test relay");
        let server = InternetEndpoint::bind(
            InternetEndpointConfig::default()
                .with_bind_addr("127.0.0.1:0".parse().unwrap())
                .with_relay_map(relay_map.clone())
                .with_insecure_relay_tls_for_test(),
        )
        .await
        .expect("direct-and-relay server");
        let client = InternetEndpoint::bind(
            InternetEndpointConfig::default()
                .with_bind_addr("127.0.0.1:0".parse().unwrap())
                .with_relay_map(relay_map)
                .with_insecure_relay_tls_for_test(),
        )
        .await
        .expect("direct-and-relay client");
        server.wait_until_online().await.expect("server online");
        client.wait_until_online().await.expect("client online");

        let server_id = server.endpoint_id();
        let server_address = server.address();
        assert!(server_address
            .addrs
            .iter()
            .any(|address| matches!(address, iroh::TransportAddr::Relay(_))));
        assert!(server_address
            .addrs
            .iter()
            .any(|address| matches!(address, iroh::TransportAddr::Ip(_))));

        let accepting = tokio::spawn(async move {
            let channel = server.accept().await.expect("incoming channel");
            let (_, mut receive) = channel.accept_bi().await.expect("incoming stream");
            assert_eq!(receive.read_to_end(8).await.unwrap(), b"direct");
            assert_eq!(channel.route_kind(), RouteKind::Direct);
            server.close().await;
        });
        let channel = client
            .connect_verified(server_address, server_id)
            .await
            .expect("direct-first connection");
        let (mut send, _) = channel.open_bi().await.unwrap();
        send.write_all(b"direct").await.unwrap();
        send.finish().unwrap();
        send.stopped().await.unwrap();
        assert_eq!(channel.route_kind(), RouteKind::Direct);

        accepting.await.unwrap();
        client.close().await;
        drop(relay_server);
    }

    #[tokio::test]
    async fn address_change_retries_are_bounded_and_report_attempt_count() {
        let server = InternetEndpoint::bind(
            InternetEndpointConfig::default()
                .with_bind_addr("127.0.0.1:0".parse().unwrap())
                .with_handshake_timeout(Duration::from_millis(300)),
        )
        .await
        .unwrap();
        let client = InternetEndpoint::bind(
            InternetEndpointConfig::default()
                .with_bind_addr("127.0.0.1:0".parse().unwrap())
                .with_handshake_timeout(Duration::from_millis(300)),
        )
        .await
        .unwrap();
        let server_id = server.endpoint_id();
        let stale = iroh::EndpointAddr::from_parts(
            server_id,
            [iroh::TransportAddr::Ip("127.0.0.1:9".parse().unwrap())],
        );
        let current = server.address();
        let accepting = tokio::spawn(async move {
            let channel = server.accept().await.unwrap();
            channel.close(ChannelCloseReason::Completed);
            server.close().await;
        });

        let connected = client
            .connect_verified_candidates(vec![stale.clone(), current], server_id, 2)
            .await
            .unwrap();
        assert_eq!(connected.attempts, 2);
        assert_eq!(connected.channel.route_kind(), RouteKind::Direct);
        connected.channel.close(ChannelCloseReason::Completed);
        accepting.await.unwrap();

        let error = client
            .connect_verified_candidates(vec![stale], server_id, 1)
            .await
            .unwrap_err();
        assert_eq!(
            error,
            InternetEndpointError::ConnectAttemptsExhausted { attempts: 1 }
        );
        client.close().await;
    }
}

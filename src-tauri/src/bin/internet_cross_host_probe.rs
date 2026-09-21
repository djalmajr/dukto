use std::{
    path::{Path, PathBuf},
    str::FromStr,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use clap::{Parser, Subcommand};
use dukto_lib::{
    internet::{
        endpoint::{InternetEndpoint, InternetEndpointConfig},
        invite::InternetInvite,
        rendezvous::{
            HttpRendezvousTransport, RendezvousClient, RendezvousPayload, RendezvousSlot,
        },
    },
    transfer::channel::{AuthenticatedChannel, ChannelCloseReason, RouteKind},
};
use iroh::{EndpointAddr, EndpointId, RelayUrl, TransportAddr};
use tokio::time::timeout;

const INVITE_TTL_SECS: u64 = 10 * 60;
const MAX_ENVELOPE_BYTES: usize = 16 * 1024;
const FIXTURE_BYTES: usize = 128 * 1024;
const OPERATION_TIMEOUT: Duration = Duration::from_secs(90);
const DIRECT_SETTLE_TIMEOUT: Duration = Duration::from_secs(20);

#[derive(Debug, Parser)]
#[command(
    name = "dukto-internet-cross-host-probe",
    about = "Cross-host Dukto internet transport verification"
)]
struct Arguments {
    #[command(subcommand)]
    role: Role,
}

#[derive(Debug, Subcommand)]
enum Role {
    Owner { exchange_path: PathBuf },
    Joiner { exchange_path: PathBuf },
}

struct SensitiveExchangeFile(PathBuf);

impl Drop for SensitiveExchangeFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

#[tokio::main]
async fn main() -> Result<(), String> {
    let _ = rustls::crypto::ring::default_provider().install_default();
    match Arguments::parse().role {
        Role::Owner { exchange_path } => run_owner(exchange_path).await,
        Role::Joiner { exchange_path } => run_joiner(exchange_path).await,
    }
}

async fn run_owner(exchange_path: PathBuf) -> Result<(), String> {
    let endpoint = bind_endpoint().await?;
    let now = unix_now()?;
    let slot = RendezvousSlot::generate().map_err(display_error)?;
    let invite = InternetInvite::new(
        endpoint.endpoint_id().to_string(),
        Vec::new(),
        Some(slot.expose().to_owned()),
        now,
        INVITE_TTL_SECS,
    )
    .map_err(display_error)?;
    let transport = HttpRendezvousTransport::configured_from_environment()
        .map_err(display_error)?
        .ok_or_else(|| "operated rendezvous is disabled".to_owned())?;
    let client = RendezvousClient::new(&transport, MAX_ENVELOPE_BYTES).map_err(display_error)?;
    let address = endpoint.address();
    let payload = RendezvousPayload::generate(
        address.id.to_string(),
        address.addrs.iter().map(ToString::to_string).collect(),
    )
    .map_err(display_error)?;
    client
        .publish(&invite, &payload, now)
        .await
        .map_err(display_error)?;
    write_sensitive(
        &exchange_path,
        invite.to_canonical_json().map_err(display_error)?,
    )?;
    let _exchange_guard = SensitiveExchangeFile(exchange_path);
    println!("PROBE owner invite_ready");

    endpoint.wait_until_online().await.map_err(display_error)?;
    let channel = timeout(OPERATION_TIMEOUT, endpoint.accept())
        .await
        .map_err(|_| "timed out waiting for the joining endpoint".to_owned())?
        .map_err(display_error)?;

    for sequence in 1..=2_u8 {
        let (mut send, mut receive) = timeout(OPERATION_TIMEOUT, channel.accept_bi())
            .await
            .map_err(|_| "timed out waiting for a transfer stream".to_owned())?
            .map_err(display_error)?;
        let received = timeout(OPERATION_TIMEOUT, receive.read_to_end(FIXTURE_BYTES + 1))
            .await
            .map_err(|_| "timed out reading the transfer fixture".to_owned())?
            .map_err(display_error)?;
        let expected = fixture(sequence);
        if received != expected {
            return Err(format!("fixture {sequence} did not match"));
        }
        send.write_all(b"verified").await.map_err(display_error)?;
        send.finish().map_err(display_error)?;
        println!(
            "PROBE owner transfer={} bytes={} sha256={}",
            sequence,
            received.len(),
            sha256_hex(&received)
        );
        if sequence == 1 {
            require_direct(&channel).await?;
        }
    }

    timeout(OPERATION_TIMEOUT, channel.closed())
        .await
        .map_err(|_| "joining endpoint did not close the connection".to_owned())?;
    println!("PROBE owner remote_close_observed=true");
    endpoint.close().await;
    Ok(())
}

async fn run_joiner(exchange_path: PathBuf) -> Result<(), String> {
    let encoded = std::fs::read_to_string(&exchange_path).map_err(display_error)?;
    let _exchange_guard = SensitiveExchangeFile(exchange_path);
    let now = unix_now()?;
    let invite = InternetInvite::from_canonical_json(encoded.trim(), now).map_err(display_error)?;
    let endpoint = bind_endpoint().await?;
    let transport = HttpRendezvousTransport::configured_from_environment()
        .map_err(display_error)?
        .ok_or_else(|| "operated rendezvous is disabled".to_owned())?;
    let client = RendezvousClient::new(&transport, MAX_ENVELOPE_BYTES).map_err(display_error)?;
    let payload = client
        .consume(&invite, &endpoint.endpoint_id().to_string(), now)
        .await
        .map_err(display_error)?;
    if payload.endpoint_id() != invite.endpoint_id() {
        return Err("rendezvous endpoint did not match the invitation".to_owned());
    }
    endpoint.wait_until_online().await.map_err(display_error)?;
    let expected_endpoint = EndpointId::from_str(payload.endpoint_id()).map_err(display_error)?;
    let address = endpoint_address(expected_endpoint, payload.addresses())?;
    let channel = timeout(
        OPERATION_TIMEOUT,
        endpoint.connect_verified(address, expected_endpoint),
    )
    .await
    .map_err(|_| "timed out connecting to the invitation owner".to_owned())?
    .map_err(display_error)?;

    for sequence in 1..=2_u8 {
        let payload = fixture(sequence);
        let (mut send, mut receive) = timeout(OPERATION_TIMEOUT, channel.open_bi())
            .await
            .map_err(|_| "timed out opening a transfer stream".to_owned())?
            .map_err(display_error)?;
        send.write_all(&payload).await.map_err(display_error)?;
        send.finish().map_err(display_error)?;
        let acknowledgement = timeout(OPERATION_TIMEOUT, receive.read_to_end(32))
            .await
            .map_err(|_| "timed out waiting for transfer acknowledgement".to_owned())?
            .map_err(display_error)?;
        if acknowledgement != b"verified" {
            return Err(format!("fixture {sequence} was not acknowledged"));
        }
        println!(
            "PROBE joiner transfer={} bytes={} sha256={}",
            sequence,
            payload.len(),
            sha256_hex(&payload)
        );
        if sequence == 1 {
            require_direct(&channel).await?;
        }
    }

    channel.close(ChannelCloseReason::Completed);
    println!("PROBE joiner local_close_sent=true");
    endpoint.close().await;
    Ok(())
}

async fn bind_endpoint() -> Result<InternetEndpoint, String> {
    let config = InternetEndpointConfig::from_environment().map_err(display_error)?;
    InternetEndpoint::bind(config).await.map_err(display_error)
}

fn endpoint_address(endpoint_id: EndpointId, addresses: &[String]) -> Result<EndpointAddr, String> {
    let mut parsed = Vec::with_capacity(addresses.len());
    for address in addresses {
        let transport = if let Some(address) = address.strip_prefix("ip:") {
            TransportAddr::Ip(address.parse().map_err(display_error)?)
        } else if let Some(address) = address.strip_prefix("relay:") {
            TransportAddr::Relay(RelayUrl::from_str(address).map_err(display_error)?)
        } else {
            return Err("invitation contains an unsupported transport address".to_owned());
        };
        parsed.push(transport);
    }
    if parsed.is_empty() {
        return Err("invitation contains no transport addresses".to_owned());
    }
    Ok(EndpointAddr::from_parts(endpoint_id, parsed))
}

async fn require_direct<C: AuthenticatedChannel>(channel: &C) -> Result<(), String> {
    let started = tokio::time::Instant::now();
    loop {
        let route = channel.route_kind();
        if route == RouteKind::Direct {
            println!("PROBE route=direct");
            return Ok(());
        }
        if started.elapsed() >= DIRECT_SETTLE_TIMEOUT {
            return Err("connection remained on relay instead of becoming direct".to_owned());
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
}

fn fixture(sequence: u8) -> Vec<u8> {
    (0..FIXTURE_BYTES)
        .map(|index| ((index + usize::from(sequence)) % 251) as u8)
        .collect()
}

fn sha256_hex(bytes: &[u8]) -> String {
    ring::digest::digest(&ring::digest::SHA256, bytes)
        .as_ref()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn unix_now() -> Result<u64, String> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .map_err(display_error)
}

fn write_sensitive(path: &Path, contents: String) -> Result<(), String> {
    std::fs::write(path, contents).map_err(display_error)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
            .map_err(display_error)?;
    }
    Ok(())
}

fn display_error(error: impl std::fmt::Display) -> String {
    error.to_string()
}

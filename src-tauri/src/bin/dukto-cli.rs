//! Headless LAN client. Wire format and filesystem handling are shared with the desktop app.
use clap::{Parser, Subcommand};
use dukto_lib::{
    crypto::noise::handshake_initiator,
    discovery::types::{PeerInfo, PROTOCOL_VERSION},
    state::device::DeviceIdentity,
    transfer::{quic::create_endpoint, sender::send_transfer},
};
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    io::Write,
    net::SocketAddr,
    path::PathBuf,
    process::ExitCode,
    time::{Duration, Instant},
};

#[path = "../cli/receive.rs"]
mod receive;

type Error = Box<dyn std::error::Error + Send + Sync>;

#[derive(Parser)]
#[command(
    name = "dukto",
    version,
    about = "Encrypted LAN file and folder transfers, compatible with Dukto desktop"
)]
struct Args {
    /// Emit newline-delimited JSON events on stdout; diagnostics go to stderr.
    #[arg(long, global = true)]
    json: bool,
    /// Isolated CLI identity directory (defaults to the user's local Dukto CLI data directory).
    #[arg(long, global = true)]
    data_dir: Option<PathBuf>,
    /// Device display name advertised through mDNS.
    #[arg(long, global = true)]
    name: Option<String>,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Discover nearby receivers using mDNS.
    Peers {
        #[arg(long, default_value_t = 5, value_parser = clap::value_parser!(u64).range(1..))]
        timeout: u64,
    },
    /// Send files and/or folders. Use -- before paths that start with a dash.
    Send {
        /// Device ID printed by peers.
        #[arg(long, required_unless_present = "address", conflicts_with = "address")]
        peer: Option<String>,
        /// Explicit QUIC endpoint, e.g. 192.168.0.16:4242, when discovery is unavailable.
        #[arg(long)]
        address: Option<SocketAddr>,
        /// Overall discovery/connection/transfer deadline in seconds.
        #[arg(long, default_value_t = 300, value_parser = clap::value_parser!(u64).range(1..))]
        timeout: u64,
        #[arg(required = true, num_args = 1..)]
        paths: Vec<PathBuf>,
    },
    /// Advertise a receiver and wait for transfers. Ctrl+C stops it.
    Receive {
        #[arg(long, short = 'd')]
        destination: PathBuf,
        /// Use another port when the desktop app is running on this computer.
        #[arg(long, default_value_t = 4242)]
        port: u16,
        /// Explicitly accept incoming transfers for this invocation without prompting.
        #[arg(long)]
        accept: bool,
        /// Exit after one attempted transfer; return nonzero if it fails or is rejected.
        #[arg(long)]
        once: bool,
        /// Per-connection deadline; time spent idle listening is unlimited.
        #[arg(long, default_value_t = 300, value_parser = clap::value_parser!(u64).range(1..))]
        timeout: u64,
    },
}

fn emit(as_json: bool, event: &str, detail: Value) {
    if as_json {
        println!("{}", json!({"event":event,"data":detail}));
    } else {
        println!("{event}: {detail}");
    }
    let _ = std::io::stdout().flush();
}

#[tokio::main]
async fn main() -> ExitCode {
    let args = Args::parse();
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "warn".into()),
        )
        .init();
    let result = tokio::select! {
        result = run(&args) => result,
        _ = tokio::signal::ctrl_c() => Err("Interrupted".into()),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            if args.json {
                emit(true, "error", json!({"message":error.to_string()}));
            } else {
                eprintln!("Error: {error}");
            }
            ExitCode::FAILURE
        }
    }
}

async fn run(args: &Args) -> Result<(), Error> {
    let data_dir = args
        .data_dir
        .clone()
        .or_else(|| dirs::data_local_dir().map(|p| p.join("com.dukto.cli")))
        .ok_or("Cannot locate CLI data directory; supply --data-dir")?;
    let mut device = DeviceIdentity::load_or_create(&data_dir).map_err(|e| e.to_string())?;
    device.display_name = args
        .name
        .clone()
        .unwrap_or_else(|| format!("{} (CLI)", device.display_name));
    match &args.command {
        Command::Peers { timeout } => {
            let peers = discover(Duration::from_secs(*timeout), None).await?;
            emit(args.json, "peers", json!(peers));
        }
        Command::Send {
            peer,
            address,
            timeout,
            paths,
        } => {
            // Validate before contacting another device or asking it to accept.
            for path in paths {
                std::fs::metadata(path).map_err(|e| format!("{}: {e}", path.display()))?;
            }
            tokio::time::timeout(Duration::from_secs(*timeout), async {
                let addresses = if let Some(address) = address {
                    vec![*address]
                } else {
                    let peer_id = peer.as_deref().ok_or("Missing peer")?;
                    let peers = discover(Duration::from_secs(10), Some(peer_id)).await?;
                    let target = peers
                        .into_iter()
                        .find(|p| p.device_id == peer_id)
                        .ok_or("Peer not found")?;
                    if target.protocol_version != PROTOCOL_VERSION {
                        return Err(format!(
                            "Peer uses protocol {}; requires {}. Update both apps.",
                            target.protocol_version, PROTOCOL_VERSION
                        )
                        .into());
                    }
                    let addresses: Vec<_> = target
                        .addresses
                        .into_iter()
                        .filter(|ip| ip.is_ipv4())
                        .map(|ip| SocketAddr::new(ip, target.port))
                        .collect();
                    addresses
                };
                let endpoint = create_endpoint("0.0.0.0:0".parse()?).map_err(|e| e.to_string())?;
                let mut connected = None;
                let mut last_error = "Peer has no usable IPv4 endpoint".to_string();
                for addr in addresses {
                    let attempt = endpoint
                        .connect(addr, "localhost")
                        .map_err(|e| e.to_string())?;
                    match tokio::time::timeout(Duration::from_secs(5), attempt).await {
                        Ok(Ok(conn)) => {
                            connected = Some(conn);
                            break;
                        }
                        Ok(Err(e)) => last_error = e.to_string(),
                        Err(_) => last_error = format!("Connection to {addr} timed out"),
                    }
                }
                let conn = connected.ok_or(last_error)?;
                let transfer_id = uuid::Uuid::new_v4().to_string();
                emit(
                    args.json,
                    "connecting",
                    json!({"transfer_id":transfer_id,"address":conn.remote_address().to_string()}),
                );
                let (mut send, mut recv) = conn.open_bi().await?;
                let mut noise = handshake_initiator(&mut send, &mut recv).await?;
                let mut last_progress = Instant::now();
                let bytes = send_transfer(
                    &mut send,
                    &mut recv,
                    &mut noise,
                    paths,
                    &transfer_id,
                    &device,
                    |p| {
                        if last_progress.elapsed() >= Duration::from_secs(1) {
                            emit(args.json, "progress", json!(p));
                            last_progress = Instant::now();
                        }
                    },
                )
                .await?;
                conn.close(0u32.into(), b"done");
                endpoint.wait_idle().await;
                emit(
                    args.json,
                    "sent",
                    json!({"transfer_id":transfer_id,"bytes_sent":bytes,"acknowledged":true}),
                );
                Ok::<(), Error>(())
            })
            .await
            .map_err(|_| "Send timed out")??;
        }
        Command::Receive {
            destination,
            port,
            accept,
            once,
            timeout,
        } => {
            receive::run(
                receive::ReceiveOptions {
                    accept: *accept,
                    destination: destination.clone(),
                    json: args.json,
                    once: *once,
                    port: *port,
                    timeout: Duration::from_secs(*timeout),
                },
                &device,
            )
            .await?;
        }
    }
    Ok(())
}

/// Browse without advertising a sender as a receiver.
async fn discover(duration: Duration, wanted: Option<&str>) -> Result<Vec<PeerInfo>, Error> {
    use dukto_lib::discovery::types::*;
    let daemon = mdns_sd::ServiceDaemon::new()?;
    let events = daemon.browse(SERVICE_TYPE)?;
    let deadline = Instant::now() + duration;
    let mut peers = BTreeMap::new();
    while Instant::now() < deadline {
        while let Ok(event) = events.try_recv() {
            if let mdns_sd::ServiceEvent::ServiceResolved(info) = event {
                let props = info
                    .get_properties()
                    .iter()
                    .map(|p| (p.key().to_string(), p.val_str().to_string()))
                    .collect();
                if let Some(peer) = PeerInfo::from_txt_records(
                    &props,
                    info.get_addresses().iter().copied().collect(),
                    info.get_port(),
                ) {
                    // mDNS can resolve IPv6 before the IPv4 record arrives. Our
                    // QUIC endpoint is IPv4, so keep browsing until it is usable.
                    let found = wanted == Some(peer.device_id.as_str())
                        && peer.addresses.iter().any(|address| address.is_ipv4());
                    peers.insert(peer.device_id.clone(), peer);
                    if found {
                        daemon.shutdown()?.recv_timeout(Duration::from_secs(2))?;
                        return Ok(peers.into_values().collect());
                    }
                }
            }
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    daemon.shutdown()?.recv_timeout(Duration::from_secs(2))?;
    Ok(peers.into_values().collect())
}

use std::io::{BufRead, IsTerminal, Write};
use std::path::PathBuf;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use dukto_lib::crypto::noise::handshake_responder;
use dukto_lib::discovery::mdns::{DiscoveryHandle, MdnsDiscovery};
use dukto_lib::discovery::types::PROTOCOL_VERSION;
use dukto_lib::protocol::types::TransferHeader;
use dukto_lib::state::device::DeviceIdentity;
use dukto_lib::transfer::quic::create_endpoint;
use dukto_lib::transfer::receiver::receive_transfer_with_cancellation;
use serde_json::json;
use tokio::sync::{oneshot, watch};
use tokio::task::JoinSet;

use super::{emit, Error, Shutdown};

pub struct ReceiveOptions {
    pub accept: bool,
    pub destination: PathBuf,
    pub json: bool,
    pub once: bool,
    pub port: u16,
    pub timeout: Duration,
}

struct ApprovalRequest {
    header: TransferHeader,
    response: oneshot::Sender<bool>,
}

/// A single terminal reader keeps concurrent approvals associated with their printed request.
fn approval_queue() -> mpsc::Sender<ApprovalRequest> {
    let (sender, receiver) = mpsc::channel::<ApprovalRequest>();
    std::thread::spawn(move || {
        read_approvals(receiver, std::io::stdin().lock(), std::io::stderr());
    });
    sender
}

fn read_approvals(
    receiver: mpsc::Receiver<ApprovalRequest>,
    mut input: impl BufRead,
    mut output: impl Write,
) {
    for request in receiver {
        if request.response.is_closed() {
            continue;
        }
        let _ = write!(
            output,
            "Accept transfer {}: {} items ({} bytes) from {}? [y/N] ",
            request.header.transfer_id,
            request.header.item_count,
            request.header.total_size,
            request
                .header
                .sender
                .as_ref()
                .map(|sender| format!("{}@{}", sender.display_name, sender.hostname))
                .unwrap_or_else(|| request.header.sender_device_id.clone())
        );
        let _ = output.flush();
        let mut line = String::new();
        let approved = input.read_line(&mut line).is_ok()
            && matches!(line.trim().to_ascii_lowercase().as_str(), "y" | "yes");
        if request.response.send(approved).is_err() {
            let _ = writeln!(
                output,
                "That transfer is no longer pending; response ignored."
            );
        }
    }
}

#[derive(Clone)]
struct Receiver {
    approval: Option<mpsc::Sender<ApprovalRequest>>,
    destination: PathBuf,
    json: bool,
    timeout: Duration,
    cancellation: watch::Receiver<bool>,
}

impl Receiver {
    async fn receive(&self, incoming: quinn::Incoming) -> Result<(), Error> {
        let address = incoming.remote_address();
        let mut transfer_id = None;
        let result = tokio::time::timeout(self.timeout, async {
            let conn = incoming.await?;
            let (mut send, mut recv) = conn.accept_bi().await?;
            let mut noise = handshake_responder(&mut send, &mut recv).await?;
            let mut last_progress = Instant::now();
            let result = receive_transfer_with_cancellation(
                &mut send,
                &mut recv,
                &mut noise,
                &self.destination,
                |header, peer_cancelled| {
                    transfer_id = Some(header.transfer_id.clone());
                    emit(self.json, "incoming", json!(header));
                    let approval = self.approval.clone();
                    let approval_connection = conn.clone();
                    async move {
                        let Some(queue) = approval else {
                            return true;
                        };
                        let (response, decision) = oneshot::channel();
                        if queue.send(ApprovalRequest { header, response }).is_err() {
                            return false;
                        }
                        tokio::select! {
                            _ = super::wait_for_cancel(peer_cancelled) => false,
                            accepted = decision => accepted.unwrap_or(false),
                            _ = approval_connection.closed() => false,
                        }
                    }
                },
                |progress| {
                    if last_progress.elapsed() >= Duration::from_secs(1) {
                        emit(self.json, "progress", json!(progress));
                        last_progress = Instant::now();
                    }
                },
                super::wait_for_cancel(self.cancellation.clone()),
            )
            .await;
            conn.close(0u32.into(), b"done");
            result
        })
        .await
        .unwrap_or_else(|_| Err("Receive timed out".into()));
        match result {
            Ok(receipt) => {
                emit(self.json, "received", json!(receipt));
                Ok(())
            }
            Err(error) => {
                emit(
                    self.json,
                    "receive_error",
                    json!({
                        "transfer_id": transfer_id,
                        "address": address.to_string(),
                        "message": error.to_string(),
                    }),
                );
                Err(error)
            }
        }
    }
}

pub async fn run(
    options: ReceiveOptions,
    device: &DeviceIdentity,
    shutdown: &Shutdown,
) -> Result<(), Error> {
    if !options.accept && !std::io::stdin().is_terminal() {
        return Err("Interactive approval requires a terminal; use --accept explicitly for automated receiving".into());
    }
    std::fs::create_dir_all(&options.destination)?;
    let destination = std::fs::canonicalize(&options.destination)?;
    let endpoint =
        create_endpoint(format!("0.0.0.0:{}", options.port).parse()?).map_err(|e| e.to_string())?;
    shutdown.register(&endpoint);
    let (discovery, _) =
        MdnsDiscovery::new(device, endpoint.local_addr()?.port()).map_err(|e| e.to_string())?;
    let _discovery = DiscoveryHandle::new(discovery);
    emit(
        options.json,
        "listening",
        json!({
            "device": device,
            "port": endpoint.local_addr()?.port(),
            "destination": destination,
            "protocol_version": PROTOCOL_VERSION,
        }),
    );
    let receiver = Receiver {
        approval: if options.accept {
            None
        } else {
            Some(approval_queue())
        },
        destination,
        json: options.json,
        timeout: options.timeout,
        cancellation: shutdown.cancellation_receiver(),
    };
    let mut transfers = JoinSet::new();
    let cancellation = shutdown.cancellation_receiver();
    loop {
        tokio::select! {
            biased;
            _ = super::wait_for_cancel(cancellation.clone()) => {
                let drain_deadline = tokio::time::Instant::now() + Duration::from_secs(5);
                while !transfers.is_empty() {
                    tokio::select! {
                        result = transfers.join_next() => {
                            if let Some(Err(error)) = result {
                                emit(options.json, "receive_error", json!({"message": error.to_string()}));
                            }
                        }
                        _ = tokio::time::sleep_until(drain_deadline) => break,
                    }
                }
                endpoint.close(
                    dukto_lib::protocol::types::TRANSFER_CANCEL_CLOSE_CODE.into(),
                    dukto_lib::protocol::types::TRANSFER_CANCEL_CLOSE_REASON,
                );
                let _ = tokio::time::timeout(Duration::from_secs(1), endpoint.wait_idle()).await;
                return Err("Transfer cancelled".into());
            }
            incoming = endpoint.accept() => {
                let incoming = incoming.ok_or("Listener closed")?;
                if options.once {
                    let result = receiver.receive(incoming).await;
                    endpoint.close(0u32.into(), b"shutdown");
                    endpoint.wait_idle().await;
                    return result;
                }
                let receiver = receiver.clone();
                transfers.spawn(async move { receiver.receive(incoming).await });
            }
            Some(result) = transfers.join_next(), if !transfers.is_empty() => {
                if let Err(error) = result {
                    emit(options.json, "receive_error", json!({"message": error.to_string()}));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn queued_approvals_keep_responses_with_their_transfer_and_skip_expired_requests() {
        let (sender, receiver) = mpsc::channel();
        let mut decisions = Vec::new();
        for id in ["expired", "first", "second"] {
            let (response, decision) = oneshot::channel();
            sender
                .send(ApprovalRequest {
                    header: TransferHeader {
                        transfer_id: id.into(),
                        sender_device_id: "peer".into(),
                        sender: None,
                        item_count: 1,
                        total_size: 512,
                    },
                    response,
                })
                .unwrap();
            if id != "expired" {
                decisions.push(decision);
            }
        }
        drop(sender);
        let mut output = Vec::new();
        read_approvals(receiver, std::io::Cursor::new(b"yes\nno\n"), &mut output);
        let mut responses = decisions.into_iter();
        assert!(responses.next().unwrap().blocking_recv().unwrap());
        assert!(!responses.next().unwrap().blocking_recv().unwrap());
        let prompts = String::from_utf8(output).unwrap();
        assert!(!prompts.contains("expired"));
        assert!(prompts.find("first").unwrap() < prompts.find("second").unwrap());
    }
}

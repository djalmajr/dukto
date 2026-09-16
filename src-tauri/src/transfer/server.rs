use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::{mpsc, watch, Mutex};

use crate::crypto::noise::{
    handshake_responder, is_remote_transfer_cancellation, RemoteTransferCancelled,
};
use crate::state::app_state::AppState;
use crate::state::device::DeviceIdentity;
use crate::transfer::cancellation::TransferRegistry;
use crate::transfer::quic::create_endpoint;
use crate::transfer::receiver::receive_transfer_with_cancellation;

/// Event emitted to the frontend when an incoming transfer request arrives.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IncomingTransferRequest {
    pub transfer_id: String,
    pub sender_device_id: String,
    pub sender: Option<DeviceIdentity>,
    pub item_count: u32,
    pub total_size: u64,
}

/// Accept/reject decision channel per transfer.
type AcceptDecision = bool;

/// Active pending incoming transfer.
struct PendingIncoming {
    tx: mpsc::Sender<AcceptDecision>,
}

#[derive(Serialize)]
struct IncomingTransferError {
    transfer_id: String,
    error: String,
}

/// Desktop adapter for the same receiving pipeline used by the CLI.
async fn handle_incoming(
    app_handle: AppHandle,
    pending: Arc<Mutex<std::collections::HashMap<String, PendingIncoming>>>,
    registry: Arc<TransferRegistry>,
    incoming: quinn::Incoming,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let conn = incoming.await?;
    let transfer_connection = conn.clone();
    let (mut send, mut recv) = conn.accept_bi().await?;
    let mut noise = handshake_responder(&mut send, &mut recv).await?;
    let destination = PathBuf::from(
        app_handle
            .state::<AppState>()
            .get_settings()
            .destination_dir,
    );
    let approval_handle = app_handle.clone();
    let progress_handle = app_handle.clone();
    let active_transfer = Arc::new(Mutex::new(None));
    let (cancellation_sender, cancellation_receiver) =
        watch::channel(None::<Arc<crate::transfer::cancellation::TransferCancellation>>);
    let was_rejected = Arc::new(AtomicBool::new(false));
    let was_remote_cancelled = Arc::new(AtomicBool::new(false));
    let active_transfer_for_approval = active_transfer.clone();
    let was_rejected_for_approval = was_rejected.clone();
    let was_remote_cancelled_for_approval = was_remote_cancelled.clone();
    let registry_for_approval = registry.clone();
    let pending_for_approval = pending.clone();
    let cancellation_sender_for_approval = cancellation_sender.clone();
    let cancellation_wait = async move {
        let mut receiver = cancellation_receiver;
        loop {
            let control = { receiver.borrow().clone() };
            if let Some(control) = control {
                control.cancelled().await;
                return;
            }
            if receiver.changed().await.is_err() {
                return;
            }
        }
    };
    let result = receive_transfer_with_cancellation(
        &mut send,
        &mut recv,
        &mut noise,
        &destination,
        move |header, peer_cancelled| async move {
            let transfer_id = header.transfer_id.clone();
            let approval_connection = transfer_connection.clone();
            let control = match registry_for_approval.register(transfer_id.clone()).await {
                Ok(control) => control,
                Err(error) => {
                    tracing::warn!(transfer_id, %error, "Duplicate incoming transfer ID");
                    return false;
                }
            };
            if !control.attach_connection(transfer_connection).await {
                registry_for_approval.remove(&transfer_id, &control).await;
                return false;
            }
            *active_transfer_for_approval.lock().await =
                Some((transfer_id.clone(), control.clone()));
            cancellation_sender_for_approval.send_replace(Some(control.clone()));

            let (tx, mut rx) = mpsc::channel(1);
            // Register the decision before notifying the UI to avoid losing fast responses.
            let inserted = {
                let mut pending = pending_for_approval.lock().await;
                if pending.contains_key(&transfer_id) {
                    false
                } else {
                    pending.insert(transfer_id.clone(), PendingIncoming { tx: tx.clone() });
                    true
                }
            };
            if !inserted {
                registry_for_approval.remove(&transfer_id, &control).await;
                *active_transfer_for_approval.lock().await = None;
                return false;
            }
            let request = IncomingTransferRequest {
                transfer_id: header.transfer_id.clone(),
                sender_device_id: header.sender_device_id,
                sender: header.sender,
                item_count: header.item_count,
                total_size: header.total_size,
            };
            let _ = approval_handle.emit("transfer:incoming", &request);
            let mut peer_cancelled = peer_cancelled;
            let peer_cancel_wait = async {
                loop {
                    let was_cancelled = *peer_cancelled.borrow();
                    if was_cancelled {
                        break;
                    }
                    if peer_cancelled.changed().await.is_err() {
                        break;
                    }
                }
            };
            let decision = tokio::select! {
                biased;
                _ = control.cancelled() => None,
                _ = peer_cancel_wait => {
                    was_remote_cancelled_for_approval.store(true, Ordering::Release);
                    None
                },
                reason = approval_connection.closed() => {
                    if is_remote_transfer_cancellation(&reason) {
                        was_remote_cancelled_for_approval.store(true, Ordering::Release);
                    }
                    None
                },
                decision = tokio::time::timeout(std::time::Duration::from_secs(60), rx.recv()) => {
                    decision.ok().flatten()
                }
            };
            {
                let mut pending = pending_for_approval.lock().await;
                if pending
                    .get(&transfer_id)
                    .is_some_and(|current| current.tx.same_channel(&tx))
                {
                    pending.remove(&transfer_id);
                }
            }
            let accepted = decision.unwrap_or(false);
            if !accepted
                && !control.is_cancelled()
                && !was_remote_cancelled_for_approval.load(Ordering::Acquire)
            {
                was_rejected_for_approval.store(true, Ordering::Release);
                let _ = approval_handle.emit("transfer:rejected", &transfer_id);
            }
            accepted
        },
        move |progress| {
            let _ = progress_handle.emit("transfer:progress", progress);
        },
        cancellation_wait,
    )
    .await;
    let result = if was_remote_cancelled.load(Ordering::Acquire) {
        Err(Box::new(RemoteTransferCancelled) as Box<dyn std::error::Error + Send + Sync>)
    } else {
        result
    };
    let active = active_transfer.lock().await.take();
    let transfer_context = if let Some((transfer_id, control)) = active {
        pending.lock().await.remove(&transfer_id);
        registry.remove(&transfer_id, &control).await;
        control.close_connection().await;
        Some((transfer_id, control))
    } else {
        None
    };

    let was_cancelled = transfer_context
        .as_ref()
        .is_some_and(|(_, control)| control.is_cancelled());
    if !was_cancelled {
        match &result {
            Ok(receipt) => {
                let _ = app_handle.emit("transfer:complete", receipt);
            }
            Err(error) => {
                tracing::error!(%error, "Incoming transfer failed");
                if let Some((transfer_id, _)) = &transfer_context {
                    if !was_rejected.load(Ordering::Acquire) {
                        let _ = app_handle.emit(
                            "transfer:receive-error",
                            &IncomingTransferError {
                                transfer_id: transfer_id.clone(),
                                error: error.to_string(),
                            },
                        );
                    }
                }
            }
        }
    }
    conn.close(0u32.into(), b"done");
    result.map(|_| ())
}

/// Manages the QUIC listener and pending incoming transfers.
pub struct TransferServer {
    pending: Arc<Mutex<std::collections::HashMap<String, PendingIncoming>>>,
    registry: Arc<TransferRegistry>,
}

impl TransferServer {
    /// Start the QUIC listener on the given port (or fallback to ephemeral port if occupied)
    /// and process incoming connections. Must be called from a context where tauri::async_runtime is available.
    pub fn start(
        app_handle: AppHandle,
        port: u16,
        registry: Arc<TransferRegistry>,
    ) -> Result<(Arc<Self>, u16), Box<dyn std::error::Error>> {
        let (endpoint, bound_port) = tauri::async_runtime::block_on(async {
            let bind_addr: SocketAddr = format!("0.0.0.0:{}", port).parse().unwrap();
            let endpoint = match create_endpoint(bind_addr) {
                Ok(ep) => ep,
                Err(e) => {
                    tracing::warn!(
                        port,
                        %e,
                        "Failed to bind preferred QUIC port, falling back to ephemeral port"
                    );
                    let fallback_addr: SocketAddr = "0.0.0.0:0".parse().unwrap();
                    create_endpoint(fallback_addr)?
                }
            };
            let bound_port = endpoint.local_addr()?.port();
            Ok::<_, Box<dyn std::error::Error>>((endpoint, bound_port))
        })?;
        tracing::info!(bound_port, "QUIC listener started");

        let server = Arc::new(Self {
            pending: Arc::new(Mutex::new(std::collections::HashMap::new())),
            registry,
        });

        let server_ref = server.clone();
        tauri::async_runtime::spawn(async move {
            loop {
                match endpoint.accept().await {
                    Some(incoming) => {
                        let handle = app_handle.clone();
                        let pending = server_ref.pending.clone();
                        let registry = server_ref.registry.clone();
                        tauri::async_runtime::spawn(async move {
                            if let Err(e) =
                                handle_incoming(handle, pending, registry, incoming).await
                            {
                                tracing::error!("Incoming transfer failed: {}", e);
                            }
                        });
                    }
                    None => {
                        tracing::info!("QUIC endpoint closed");
                        break;
                    }
                }
            }
        });

        Ok((server, bound_port))
    }

    /// Respond to a pending incoming transfer.
    pub async fn respond(&self, transfer_id: &str, accepted: bool) {
        let mut pending = self.pending.lock().await;
        if let Some(p) = pending.remove(transfer_id) {
            let _ = p.tx.send(accepted).await;
        }
    }
}

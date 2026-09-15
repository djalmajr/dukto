use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::{watch, Mutex};

/// Owns cancellation state for active outgoing and incoming transfers.
#[derive(Default)]
pub struct TransferRegistry {
    state: Mutex<RegistryState>,
}

#[derive(Default)]
struct RegistryState {
    active: HashMap<String, Arc<TransferCancellation>>,
    installing_update: bool,
}

/// Per-transfer cancellation signal and its QUIC connection, when established.
pub struct TransferCancellation {
    cancelled: watch::Sender<bool>,
    connection: Mutex<Option<quinn::Connection>>,
}

impl TransferRegistry {
    /// Reserve an idle app for installation atomically with transfer registration.
    pub async fn begin_update(&self) -> Result<(), String> {
        let mut state = self.state.lock().await;
        if state.installing_update {
            return Err("An update is already being installed.".into());
        }
        if !state.active.is_empty() {
            return Err(
                "Wait for active transfers and incoming requests to finish before updating.".into(),
            );
        }
        state.installing_update = true;
        Ok(())
    }

    pub async fn end_update(&self) {
        self.state.lock().await.installing_update = false;
    }

    /// Reserve an ID before work starts so cancellation cannot race task startup.
    pub async fn register(&self, transfer_id: String) -> Result<Arc<TransferCancellation>, String> {
        let mut state = self.state.lock().await;
        if state.installing_update {
            return Err("Dukto is installing an update. Try again after restart.".into());
        }
        if state.active.contains_key(&transfer_id) {
            return Err(format!("Transfer {transfer_id} is already active"));
        }

        let (cancelled, _) = watch::channel(false);
        let control = Arc::new(TransferCancellation {
            cancelled,
            connection: Mutex::new(None),
        });
        state.active.insert(transfer_id, control.clone());
        Ok(control)
    }

    /// Cancel only the transfer identified by `transfer_id`.
    pub async fn cancel(&self, transfer_id: &str) -> bool {
        let state = self.state.lock().await;
        match state.active.get(transfer_id) {
            Some(control) => control.cancel().await,
            None => false,
        }
    }

    /// Release a reservation only when it still belongs to this task.
    pub async fn remove(&self, transfer_id: &str, control: &Arc<TransferCancellation>) {
        let mut state = self.state.lock().await;
        if state
            .active
            .get(transfer_id)
            .is_some_and(|current| Arc::ptr_eq(current, control))
        {
            state.active.remove(transfer_id);
        }
    }
}

impl TransferCancellation {
    pub fn is_cancelled(&self) -> bool {
        *self.cancelled.borrow()
    }

    /// Wait until this transfer alone is cancelled.
    pub async fn cancelled(&self) {
        let mut receiver = self.cancelled.subscribe();
        while !*receiver.borrow() {
            if receiver.changed().await.is_err() {
                break;
            }
        }
    }

    /// Attach the QUIC connection, closing it if cancellation already won.
    pub async fn attach_connection(&self, connection: quinn::Connection) -> bool {
        let mut current = self.connection.lock().await;
        if self.is_cancelled() {
            connection.close(0u32.into(), b"transfer cancelled");
            false
        } else {
            *current = Some(connection);
            true
        }
    }

    /// Signal cancellation and interrupt any active network operation.
    pub async fn cancel(&self) -> bool {
        if self.cancelled.send_replace(true) {
            return false;
        }
        if let Some(connection) = self.connection.lock().await.as_ref() {
            connection.close(0u32.into(), b"transfer cancelled");
        }
        true
    }

    /// Close and release a connection after normal completion or failure.
    pub async fn close_connection(&self) {
        if let Some(connection) = self.connection.lock().await.take() {
            connection.close(0u32.into(), b"transfer finished");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transfer::quic::create_endpoint;
    use std::net::SocketAddr;

    #[tokio::test]
    async fn update_waits_for_transfers_then_blocks_new_work_until_released() {
        // Verified mutation: removing the installation guard from register fails this test.
        let registry = TransferRegistry::default();
        let transfer = registry.register("incoming-approval".into()).await.unwrap();
        assert!(registry.begin_update().await.is_err());
        assert!(!transfer.is_cancelled());
        registry.remove("incoming-approval", &transfer).await;
        registry.begin_update().await.unwrap();
        assert!(registry.begin_update().await.is_err());
        assert!(registry.register("outgoing".into()).await.is_err());
        registry.end_update().await;
        assert!(registry.register("outgoing".into()).await.is_ok());
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn update_and_transfer_registration_cannot_both_win() {
        // Verified mutation: allowing register during installation lets both competitors win.
        for _ in 0..64 {
            let registry = Arc::new(TransferRegistry::default());
            let barrier = Arc::new(tokio::sync::Barrier::new(2));
            let updating = tokio::spawn({
                let registry = registry.clone();
                let barrier = barrier.clone();
                async move {
                    barrier.wait().await;
                    registry.begin_update().await
                }
            });
            barrier.wait().await;
            let transfer = registry.register("new-transfer".into()).await;
            let update = updating.await.unwrap();
            assert_ne!(update.is_ok(), transfer.is_ok());
        }
    }

    #[tokio::test]
    async fn cancellation_is_scoped_and_wakes_only_the_named_transfer() {
        // Mutation captured: ignoring the transfer ID would cancel both concurrent transfers.
        let registry = Arc::new(TransferRegistry::default());
        let first = registry.register("send-a".into()).await.unwrap();
        let second = registry.register("receive-b".into()).await.unwrap();
        let second_waiter = tokio::spawn({
            let second = second.clone();
            async move { second.cancelled().await }
        });

        assert!(registry.cancel("send-a").await);
        assert!(first.is_cancelled());
        assert!(!second.is_cancelled());
        assert!(!registry.cancel("send-a").await);
        assert!(!second_waiter.is_finished());

        assert!(registry.cancel("receive-b").await);
        tokio::time::timeout(std::time::Duration::from_secs(1), second_waiter)
            .await
            .unwrap()
            .unwrap();
        assert!(second.is_cancelled());

        registry.remove("send-a", &first).await;
        assert!(!registry.cancel("send-a").await);
        assert!(registry.register("send-a".into()).await.is_ok());
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn cancelling_one_quic_transfer_keeps_the_other_connection_usable() {
        // Mutation captured: closing a shared endpoint or cancelling all IDs breaks the second stream.
        let server_endpoint =
            create_endpoint("127.0.0.1:0".parse::<SocketAddr>().unwrap()).unwrap();
        let server_address = server_endpoint.local_addr().unwrap();
        let accepting = tokio::spawn(async move {
            let mut connections = Vec::new();
            for _ in 0..2 {
                let incoming = server_endpoint.accept().await.unwrap();
                connections.push(incoming.await.unwrap());
            }
            connections
        });
        let client_endpoint =
            create_endpoint("127.0.0.1:0".parse::<SocketAddr>().unwrap()).unwrap();
        let first_connection = client_endpoint
            .connect(server_address, "localhost")
            .unwrap()
            .await
            .unwrap();
        let second_connection = client_endpoint
            .connect(server_address, "localhost")
            .unwrap()
            .await
            .unwrap();
        let peer_connections = accepting.await.unwrap();

        let registry = TransferRegistry::default();
        let first = registry.register("send-a".into()).await.unwrap();
        let second = registry.register("send-b".into()).await.unwrap();
        assert!(first.attach_connection(first_connection.clone()).await);
        assert!(second.attach_connection(second_connection.clone()).await);

        assert!(registry.cancel("send-a").await);
        tokio::time::timeout(
            std::time::Duration::from_secs(3),
            peer_connections[0].closed(),
        )
        .await
        .expect("the canceled peer connection should close");
        assert!(!second.is_cancelled());

        let (mut sender, _) = second_connection.open_bi().await.unwrap();
        let (_, mut receiver) = peer_connections[1].accept_bi().await.unwrap();
        sender.write_all(b"still active").await.unwrap();
        sender.finish().unwrap();
        let received = receiver.read_to_end(32).await.unwrap();
        assert_eq!(received, b"still active");

        registry.remove("send-a", &first).await;
        registry.remove("send-b", &second).await;
        client_endpoint.close(0u32.into(), b"test finished");
    }
}

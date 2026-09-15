use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::{watch, Mutex};

/// Owns cancellation state for active outgoing and incoming transfers.
#[derive(Default)]
pub struct TransferRegistry {
    active: Mutex<HashMap<String, Arc<TransferCancellation>>>,
}

/// Per-transfer cancellation signal and its QUIC connection, when established.
pub struct TransferCancellation {
    cancelled: watch::Sender<bool>,
    connection: Mutex<Option<quinn::Connection>>,
}

impl TransferRegistry {
    /// Reserve an ID before work starts so cancellation cannot race task startup.
    pub async fn register(&self, transfer_id: String) -> Result<Arc<TransferCancellation>, String> {
        let mut active = self.active.lock().await;
        if active.contains_key(&transfer_id) {
            return Err(format!("Transfer {transfer_id} is already active"));
        }

        let (cancelled, _) = watch::channel(false);
        let control = Arc::new(TransferCancellation {
            cancelled,
            connection: Mutex::new(None),
        });
        active.insert(transfer_id, control.clone());
        Ok(control)
    }

    /// Cancel only the transfer identified by `transfer_id`.
    pub async fn cancel(&self, transfer_id: &str) -> bool {
        let active = self.active.lock().await;
        match active.get(transfer_id) {
            Some(control) => control.cancel().await,
            None => false,
        }
    }

    /// Release a reservation only when it still belongs to this task.
    pub async fn remove(&self, transfer_id: &str, control: &Arc<TransferCancellation>) {
        let mut active = self.active.lock().await;
        if active
            .get(transfer_id)
            .is_some_and(|current| Arc::ptr_eq(current, control))
        {
            active.remove(transfer_id);
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

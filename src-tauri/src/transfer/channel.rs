use std::{future::Future, pin::Pin};

use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::io::{AsyncRead, AsyncWrite};

const AUTHENTICATION_FAILED_CLOSE_CODE: u32 = 0xD0171;
const PROTOCOL_ERROR_CLOSE_CODE: u32 = 0xD0172;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RouteKind {
    Direct,
    Relay,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChannelCloseReason {
    Completed,
    Cancelled,
    AuthenticationFailed,
    ProtocolError,
}

impl ChannelCloseReason {
    pub(crate) fn code(self) -> u32 {
        match self {
            Self::Completed => 0,
            Self::Cancelled => crate::protocol::types::TRANSFER_CANCEL_CLOSE_CODE,
            Self::AuthenticationFailed => AUTHENTICATION_FAILED_CLOSE_CODE,
            Self::ProtocolError => PROTOCOL_ERROR_CLOSE_CODE,
        }
    }

    pub(crate) fn reason(self) -> &'static [u8] {
        match self {
            Self::Completed => b"dukto:complete:v1",
            Self::Cancelled => crate::protocol::types::TRANSFER_CANCEL_CLOSE_REASON,
            Self::AuthenticationFailed => b"dukto:authentication-failed:v1",
            Self::ProtocolError => b"dukto:protocol-error:v1",
        }
    }
}

#[derive(Debug, Error)]
pub enum ChannelError {
    #[error("authenticated peer ID is invalid")]
    InvalidPeerId,
    #[error("failed to open an authenticated channel stream over {transport}")]
    OpenStream { transport: &'static str },
    #[error("failed to accept an authenticated channel stream over {transport}")]
    AcceptStream { transport: &'static str },
}

pub type TransferStreamError = Box<dyn std::error::Error + Send + Sync>;

pub trait TransferSendStream: AsyncWrite + Unpin + Send {
    fn finish_transfer(&mut self) -> Result<(), TransferStreamError>;
    fn stopped_transfer(
        &self,
    ) -> Pin<Box<dyn Future<Output = Result<Option<u64>, TransferStreamError>> + Send>>;
    fn reset_transfer(&mut self, code: u32);
}

pub trait TransferReceiveStream: AsyncRead + Unpin + Send {
    fn received_reset_transfer(
        &mut self,
    ) -> impl Future<Output = Result<Option<u64>, TransferStreamError>> + Send;
    fn stop_transfer(&mut self, code: u32);
}

impl TransferSendStream for quinn::SendStream {
    fn finish_transfer(&mut self) -> Result<(), TransferStreamError> {
        self.finish().map_err(Into::into)
    }

    fn stopped_transfer(
        &self,
    ) -> Pin<Box<dyn Future<Output = Result<Option<u64>, TransferStreamError>> + Send>> {
        let stopped = self.stopped();
        Box::pin(async move {
            stopped
                .await
                .map(|code| code.map(quinn::VarInt::into_inner))
                .map_err(Into::into)
        })
    }

    fn reset_transfer(&mut self, code: u32) {
        let _ = self.reset(code.into());
    }
}

impl TransferReceiveStream for quinn::RecvStream {
    async fn received_reset_transfer(&mut self) -> Result<Option<u64>, TransferStreamError> {
        self.received_reset()
            .await
            .map(|code| code.map(quinn::VarInt::into_inner))
            .map_err(Into::into)
    }

    fn stop_transfer(&mut self, code: u32) {
        let _ = self.stop(code.into());
    }
}

impl TransferSendStream for iroh::endpoint::SendStream {
    fn finish_transfer(&mut self) -> Result<(), TransferStreamError> {
        self.finish().map_err(Into::into)
    }

    fn stopped_transfer(
        &self,
    ) -> Pin<Box<dyn Future<Output = Result<Option<u64>, TransferStreamError>> + Send>> {
        let stopped = self.stopped();
        Box::pin(async move {
            stopped
                .await
                .map(|code| code.map(iroh::endpoint::VarInt::into_inner))
                .map_err(Into::into)
        })
    }

    fn reset_transfer(&mut self, code: u32) {
        let _ = self.reset(code.into());
    }
}

impl TransferReceiveStream for iroh::endpoint::RecvStream {
    async fn received_reset_transfer(&mut self) -> Result<Option<u64>, TransferStreamError> {
        self.received_reset()
            .await
            .map(|code| code.map(iroh::endpoint::VarInt::into_inner))
            .map_err(Into::into)
    }

    fn stop_transfer(&mut self, code: u32) {
        let _ = self.stop(code.into());
    }
}

pub trait AuthenticatedChannel: Send + Sync {
    type SendStream: TransferSendStream + 'static;
    type ReceiveStream: TransferReceiveStream + 'static;

    fn peer_id(&self) -> &str;
    fn route_kind(&self) -> RouteKind;

    fn open_bi(
        &self,
    ) -> impl Future<Output = Result<(Self::SendStream, Self::ReceiveStream), ChannelError>> + Send;

    fn accept_bi(
        &self,
    ) -> impl Future<Output = Result<(Self::SendStream, Self::ReceiveStream), ChannelError>> + Send;

    fn close(&self, reason: ChannelCloseReason);
}

/// Adapter for a QUIC connection whose remote identity was verified by the
/// caller's transport or pairing layer before construction.
#[derive(Clone)]
pub struct QuinnAuthenticatedChannel {
    connection: quinn::Connection,
    peer_id: String,
}

impl QuinnAuthenticatedChannel {
    pub fn new_direct(
        verified_connection: quinn::Connection,
        authenticated_peer_id: impl Into<String>,
    ) -> Result<Self, ChannelError> {
        let peer_id = authenticated_peer_id.into();
        if peer_id.is_empty()
            || peer_id.len() > 256
            || peer_id.trim() != peer_id
            || peer_id.chars().any(|character| {
                !character.is_ascii() || character.is_ascii_control() || character.is_whitespace()
            })
        {
            return Err(ChannelError::InvalidPeerId);
        }
        Ok(Self {
            connection: verified_connection,
            peer_id,
        })
    }
}

impl AuthenticatedChannel for QuinnAuthenticatedChannel {
    type SendStream = quinn::SendStream;
    type ReceiveStream = quinn::RecvStream;

    fn peer_id(&self) -> &str {
        &self.peer_id
    }

    fn route_kind(&self) -> RouteKind {
        RouteKind::Direct
    }

    fn open_bi(
        &self,
    ) -> impl Future<Output = Result<(Self::SendStream, Self::ReceiveStream), ChannelError>> + Send
    {
        let connection = self.connection.clone();
        async move {
            connection
                .open_bi()
                .await
                .map_err(|_| ChannelError::OpenStream { transport: "quinn" })
        }
    }

    fn accept_bi(
        &self,
    ) -> impl Future<Output = Result<(Self::SendStream, Self::ReceiveStream), ChannelError>> + Send
    {
        let connection = self.connection.clone();
        async move {
            connection
                .accept_bi()
                .await
                .map_err(|_| ChannelError::AcceptStream { transport: "quinn" })
        }
    }

    fn close(&self, reason: ChannelCloseReason) {
        self.connection.close(reason.code().into(), reason.reason());
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::{
        AuthenticatedChannel, ChannelCloseReason, ChannelError, QuinnAuthenticatedChannel,
    };
    use crate::transfer::quic::create_endpoint;

    #[tokio::test]
    async fn quinn_adapter_rejects_unverified_empty_peer_identity() {
        let endpoint = create_endpoint("127.0.0.1:0".parse().unwrap()).unwrap();
        let address = endpoint.local_addr().unwrap();
        let peer_endpoint = create_endpoint("127.0.0.1:0".parse().unwrap()).unwrap();
        let accepting =
            tokio::spawn(async move { endpoint.accept().await.unwrap().await.unwrap() });
        let connection = peer_endpoint
            .connect(address, "localhost")
            .unwrap()
            .await
            .unwrap();
        let _accepted = accepting.await.unwrap();

        assert!(matches!(
            QuinnAuthenticatedChannel::new_direct(connection, " "),
            Err(ChannelError::InvalidPeerId)
        ));
        peer_endpoint.close(0u32.into(), b"test complete");
    }

    #[tokio::test]
    async fn typed_cancellation_close_reaches_the_peer() {
        let server_endpoint = create_endpoint("127.0.0.1:0".parse().unwrap()).unwrap();
        let address = server_endpoint.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let connection = server_endpoint.accept().await.unwrap().await.unwrap();
            connection.closed().await
        });
        let client_endpoint = create_endpoint("127.0.0.1:0".parse().unwrap()).unwrap();
        let connection = client_endpoint
            .connect(address, "localhost")
            .unwrap()
            .await
            .unwrap();
        let channel = QuinnAuthenticatedChannel::new_direct(connection, "receiver").unwrap();

        channel.close(ChannelCloseReason::Cancelled);

        let remote_error = tokio::time::timeout(Duration::from_secs(3), server)
            .await
            .expect("typed close should arrive")
            .expect("server task should not panic");
        match remote_error {
            quinn::ConnectionError::ApplicationClosed(close) => {
                assert_eq!(
                    close.error_code,
                    crate::protocol::types::TRANSFER_CANCEL_CLOSE_CODE.into()
                );
                assert_eq!(
                    close.reason.as_ref(),
                    crate::protocol::types::TRANSFER_CANCEL_CLOSE_REASON
                );
            }
            other => panic!("expected typed application close, got {other:?}"),
        }
        client_endpoint.close(0u32.into(), b"test complete");
    }
}

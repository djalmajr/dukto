use std::future::Future;

use iroh::endpoint::{Connection, RecvStream, SendStream};

use crate::transfer::channel::{AuthenticatedChannel, ChannelCloseReason, ChannelError, RouteKind};

#[derive(Clone)]
pub struct IrohAuthenticatedChannel {
    connection: Connection,
    peer_id: String,
}

impl IrohAuthenticatedChannel {
    pub(crate) fn new(connection: Connection) -> Self {
        Self {
            peer_id: connection.remote_id().to_string(),
            connection,
        }
    }

    pub async fn closed(&self) {
        self.connection.closed().await;
    }
}

impl AuthenticatedChannel for IrohAuthenticatedChannel {
    type SendStream = SendStream;
    type ReceiveStream = RecvStream;

    fn peer_id(&self) -> &str {
        &self.peer_id
    }

    fn route_kind(&self) -> RouteKind {
        if self
            .connection
            .paths()
            .iter()
            .any(|path| path.is_selected() && path.is_relay())
        {
            RouteKind::Relay
        } else {
            RouteKind::Direct
        }
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
                .map_err(|_| ChannelError::OpenStream { transport: "iroh" })
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
                .map_err(|_| ChannelError::AcceptStream { transport: "iroh" })
        }
    }

    fn close(&self, reason: ChannelCloseReason) {
        self.connection.close(reason.code().into(), reason.reason());
    }
}

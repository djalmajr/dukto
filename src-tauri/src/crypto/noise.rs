use snow::{Builder, TransportState};
use std::error::Error;
use std::time::Duration;
// tokio::io::AsyncWriteExt used indirectly via quinn streams

const NOISE_PATTERN: &str = "Noise_XX_25519_ChaChaPoly_BLAKE2s";
const MAX_MSG_LEN: usize = 65535;

#[derive(Debug, thiserror::Error)]
#[error("Transfer cancelled by the remote peer")]
pub struct RemoteTransferCancelled;

#[derive(Debug, thiserror::Error)]
#[error("Transfer cancelled")]
pub struct TransferCancelled;

/// Close a QUIC connection with a stable application marker for transfer cancellation.
pub fn close_for_transfer_cancellation(connection: &quinn::Connection) {
    connection.close(
        crate::protocol::types::TRANSFER_CANCEL_CLOSE_CODE.into(),
        crate::protocol::types::TRANSFER_CANCEL_CLOSE_REASON,
    );
}

/// Abort the send half of a transfer stream with the shared cancellation code.
/// QUIC retransmits RESET_STREAM until acknowledged, unlike CONNECTION_CLOSE.
pub fn reset_transfer_send_stream(send: &mut quinn::SendStream) {
    let _ = send.reset(crate::protocol::types::TRANSFER_CANCEL_CLOSE_CODE.into());
}

/// Stop receiving the data half of a transfer stream with the shared cancellation code.
pub fn stop_transfer_receive_stream(recv: &mut quinn::RecvStream) {
    let _ = recv.stop(crate::protocol::types::TRANSFER_CANCEL_CLOSE_CODE.into());
}

/// Wait briefly for the other side to acknowledge a cancellation on the reverse stream.
pub async fn wait_for_transfer_cancel_ack(recv: &mut quinn::RecvStream) -> bool {
    tokio::time::timeout(Duration::from_secs(3), async {
        let mut byte = [0u8; 1];
        loop {
            match recv.read(&mut byte).await {
                Err(quinn::ReadError::Reset(code))
                    if code == crate::protocol::types::TRANSFER_CANCEL_CLOSE_CODE.into() =>
                {
                    return true;
                }
                Err(_) | Ok(None) => return false,
                Ok(Some(_)) => continue,
            }
        }
    })
    .await
    .unwrap_or(false)
}

/// Identify the reserved connection-close or stream-abort marker for cancellation.
pub fn is_remote_transfer_cancellation(error: &(dyn Error + 'static)) -> bool {
    let mut current = Some(error);
    while let Some(error) = current {
        if error.is::<RemoteTransferCancelled>() {
            return true;
        }
        if let Some(connection_error) = error.downcast_ref::<quinn::ConnectionError>() {
            if is_cancellation_close(connection_error) {
                return true;
            }
        }
        if let Some(quinn::ReadError::ConnectionLost(connection_error)) =
            error.downcast_ref::<quinn::ReadError>()
        {
            if is_cancellation_close(connection_error) {
                return true;
            }
        }
        if matches!(
            error.downcast_ref::<quinn::ReadError>(),
            Some(quinn::ReadError::Reset(code))
                if *code == crate::protocol::types::TRANSFER_CANCEL_CLOSE_CODE.into()
        ) {
            return true;
        }
        if matches!(
            error.downcast_ref::<quinn::ReadExactError>(),
            Some(quinn::ReadExactError::ReadError(quinn::ReadError::Reset(code)))
                if *code == crate::protocol::types::TRANSFER_CANCEL_CLOSE_CODE.into()
        ) || matches!(
            error.downcast_ref::<quinn::ReadToEndError>(),
            Some(quinn::ReadToEndError::Read(quinn::ReadError::Reset(code)))
                if *code == crate::protocol::types::TRANSFER_CANCEL_CLOSE_CODE.into()
        ) {
            return true;
        }
        if let Some(quinn::WriteError::ConnectionLost(connection_error)) =
            error.downcast_ref::<quinn::WriteError>()
        {
            if is_cancellation_close(connection_error) {
                return true;
            }
        }
        if matches!(
            error.downcast_ref::<quinn::WriteError>(),
            Some(quinn::WriteError::Stopped(code))
                if *code == crate::protocol::types::TRANSFER_CANCEL_CLOSE_CODE.into()
        ) {
            return true;
        }
        if let Some(io_error) = error.downcast_ref::<std::io::Error>() {
            if io_error
                .get_ref()
                .is_some_and(|source| is_remote_transfer_cancellation(source))
            {
                return true;
            }
        }
        current = error.source();
    }
    false
}

fn is_cancellation_close(error: &quinn::ConnectionError) -> bool {
    matches!(
        error,
        quinn::ConnectionError::ApplicationClosed(close)
            if close.error_code == crate::protocol::types::TRANSFER_CANCEL_CLOSE_CODE.into()
                && close.reason.as_ref() == crate::protocol::types::TRANSFER_CANCEL_CLOSE_REASON
    )
}

fn normalize_transport_error<E>(error: E) -> Box<dyn Error + Send + Sync>
where
    E: Error + Send + Sync + 'static,
{
    if is_remote_transfer_cancellation(&error) {
        Box::new(RemoteTransferCancelled)
    } else {
        Box::new(error)
    }
}

/// Perform Noise_XX handshake as the initiator over a QUIC stream.
/// Returns a TransportState ready for encrypted communication.
pub async fn handshake_initiator(
    send: &mut quinn::SendStream,
    recv: &mut quinn::RecvStream,
) -> Result<TransportState, Box<dyn std::error::Error + Send + Sync>> {
    let builder = Builder::new(NOISE_PATTERN.parse()?);
    let keypair = builder.generate_keypair()?;
    let mut noise = builder
        .local_private_key(&keypair.private)
        .build_initiator()?;

    let mut buf = vec![0u8; MAX_MSG_LEN];

    // -> e
    let len = noise.write_message(&[], &mut buf)?;
    send_framed(send, &buf[..len]).await?;

    // <- e, ee, s, es
    let msg = recv_framed(recv).await?;
    noise.read_message(&msg, &mut buf)?;

    // -> s, se
    let len = noise.write_message(&[], &mut buf)?;
    send_framed(send, &buf[..len]).await?;

    Ok(noise.into_transport_mode()?)
}

/// Perform Noise_XX handshake as the responder over a QUIC stream.
/// Returns a TransportState ready for encrypted communication.
pub async fn handshake_responder(
    send: &mut quinn::SendStream,
    recv: &mut quinn::RecvStream,
) -> Result<TransportState, Box<dyn std::error::Error + Send + Sync>> {
    let builder = Builder::new(NOISE_PATTERN.parse()?);
    let keypair = builder.generate_keypair()?;
    let mut noise = builder
        .local_private_key(&keypair.private)
        .build_responder()?;

    let mut buf = vec![0u8; MAX_MSG_LEN];

    // <- e
    let msg = recv_framed(recv).await?;
    noise.read_message(&msg, &mut buf)?;

    // -> e, ee, s, es
    let len = noise.write_message(&[], &mut buf)?;
    send_framed(send, &buf[..len]).await?;

    // <- s, se
    let msg = recv_framed(recv).await?;
    noise.read_message(&msg, &mut buf)?;

    Ok(noise.into_transport_mode()?)
}

/// Send a length-prefixed frame.
pub async fn send_framed(
    send: &mut quinn::SendStream,
    data: &[u8],
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let len = (data.len() as u32).to_le_bytes();
    send.write_all(&len)
        .await
        .map_err(normalize_transport_error)?;
    send.write_all(data)
        .await
        .map_err(normalize_transport_error)?;
    Ok(())
}

/// Receive a length-prefixed frame.
pub async fn recv_framed(
    recv: &mut quinn::RecvStream,
) -> Result<Vec<u8>, Box<dyn std::error::Error + Send + Sync>> {
    let mut len_buf = [0u8; 4];
    recv.read_exact(&mut len_buf)
        .await
        .map_err(normalize_transport_error)?;
    let len = u32::from_le_bytes(len_buf) as usize;

    if len > MAX_MSG_LEN {
        return Err(format!("Frame too large: {} bytes", len).into());
    }

    let mut buf = vec![0u8; len];
    recv.read_exact(&mut buf)
        .await
        .map_err(normalize_transport_error)?;
    Ok(buf)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transfer::quic::create_endpoint;
    use std::net::SocketAddr;

    #[tokio::test]
    async fn noise_handshake_completes_over_quic() {
        let server_addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
        let server_ep = create_endpoint(server_addr).expect("server endpoint");
        let server_addr = server_ep.local_addr().unwrap();

        let client_ep = create_endpoint("127.0.0.1:0".parse().unwrap()).expect("client endpoint");

        // Server: accept connection, do responder handshake
        let server_handle = tokio::spawn(async move {
            let incoming = server_ep.accept().await.expect("no incoming");
            let conn = incoming.await.expect("connection failed");
            let (mut send, mut recv) = conn.accept_bi().await.expect("no bi stream");

            let mut transport = handshake_responder(&mut send, &mut recv)
                .await
                .expect("responder handshake failed");

            // Receive an encrypted message to verify transport works
            let encrypted = recv_framed(&mut recv).await.expect("recv failed");
            let mut plaintext = vec![0u8; MAX_MSG_LEN];
            let len = transport
                .read_message(&encrypted, &mut plaintext)
                .expect("decrypt failed");
            let msg = String::from_utf8_lossy(&plaintext[..len]);
            assert_eq!(msg, "hello from initiator");

            conn.close(0u32.into(), b"done");
            server_ep.close(0u32.into(), b"shutdown");
        });

        // Client: connect, do initiator handshake
        let conn = client_ep
            .connect(server_addr, "localhost")
            .unwrap()
            .await
            .expect("connection failed");

        let (mut send, mut recv) = conn.open_bi().await.expect("open bi failed");

        let mut transport = handshake_initiator(&mut send, &mut recv)
            .await
            .expect("initiator handshake failed");

        // Send an encrypted message to verify transport works
        let mut ciphertext = vec![0u8; MAX_MSG_LEN];
        let len = transport
            .write_message(b"hello from initiator", &mut ciphertext)
            .expect("encrypt failed");
        send_framed(&mut send, &ciphertext[..len])
            .await
            .expect("send failed");

        // Wait for server to finish before closing
        server_handle.await.unwrap();

        conn.close(0u32.into(), b"done");
        client_ep.close(0u32.into(), b"shutdown");
    }

    #[tokio::test]
    async fn framed_write_reports_remote_cancellation_separately_from_connection_loss() {
        // Mutation captured: omitting the cancellation close marker makes this an ordinary disconnect.
        let server_endpoint =
            create_endpoint("127.0.0.1:0".parse::<SocketAddr>().unwrap()).unwrap();
        let address = server_endpoint.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let connection = server_endpoint.accept().await.unwrap().await.unwrap();
            let _ = connection.accept_bi().await.unwrap();
            close_for_transfer_cancellation(&connection);
            server_endpoint.wait_idle().await;
        });

        let client_endpoint =
            create_endpoint("127.0.0.1:0".parse::<SocketAddr>().unwrap()).unwrap();
        let connection = client_endpoint
            .connect(address, "localhost")
            .unwrap()
            .await
            .unwrap();
        let (mut send, _) = connection.open_bi().await.unwrap();
        let close_reason =
            tokio::time::timeout(std::time::Duration::from_secs(3), connection.closed())
                .await
                .unwrap();
        assert!(is_remote_transfer_cancellation(&close_reason));

        let error = send_framed(&mut send, b"still sending").await.unwrap_err();
        assert!(error
            .to_string()
            .contains("Transfer cancelled by the remote peer"));
        server.await.unwrap();
        client_endpoint.close(0u32.into(), b"test complete");
    }
}

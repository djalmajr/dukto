use snow::{Builder, TransportState};
// tokio::io::AsyncWriteExt used indirectly via quinn streams

const NOISE_PATTERN: &str = "Noise_XX_25519_ChaChaPoly_BLAKE2s";
const MAX_MSG_LEN: usize = 65535;

/// Perform Noise_XX handshake as the initiator over a QUIC stream.
/// Returns a TransportState ready for encrypted communication.
pub async fn handshake_initiator(
    send: &mut quinn::SendStream,
    recv: &mut quinn::RecvStream,
) -> Result<TransportState, Box<dyn std::error::Error + Send + Sync>> {
    let builder = Builder::new(NOISE_PATTERN.parse()?);
    let keypair = builder.generate_keypair()?;
    let mut noise = builder.local_private_key(&keypair.private).build_initiator()?;

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
    let mut noise = builder.local_private_key(&keypair.private).build_responder()?;

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
    send.write_all(&len).await?;
    send.write_all(data).await?;
    Ok(())
}

/// Receive a length-prefixed frame.
pub async fn recv_framed(
    recv: &mut quinn::RecvStream,
) -> Result<Vec<u8>, Box<dyn std::error::Error + Send + Sync>> {
    let mut len_buf = [0u8; 4];
    recv.read_exact(&mut len_buf).await?;
    let len = u32::from_le_bytes(len_buf) as usize;

    if len > MAX_MSG_LEN {
        return Err(format!("Frame too large: {} bytes", len).into());
    }

    let mut buf = vec![0u8; len];
    recv.read_exact(&mut buf).await?;
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
}

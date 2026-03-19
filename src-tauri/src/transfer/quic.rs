use std::net::SocketAddr;
use std::sync::Arc;

use crate::crypto::cert::generate_self_signed_config;

/// Create a QUIC endpoint configured as both server and client.
pub fn create_endpoint(
    bind_addr: SocketAddr,
) -> Result<quinn::Endpoint, Box<dyn std::error::Error>> {
    let (server_tls, client_tls) = generate_self_signed_config()?;

    let server_config = quinn::ServerConfig::with_crypto(Arc::new(
        quinn::crypto::rustls::QuicServerConfig::try_from(server_tls)?,
    ));

    let client_config = quinn::ClientConfig::new(Arc::new(
        quinn::crypto::rustls::QuicClientConfig::try_from(client_tls)?,
    ));

    let mut endpoint = quinn::Endpoint::server(server_config, bind_addr)?;
    endpoint.set_default_client_config(client_config);

    Ok(endpoint)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn quic_client_connects_to_server() {
        let server_addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
        let server_endpoint = create_endpoint(server_addr).expect("server endpoint");
        let server_addr = server_endpoint.local_addr().unwrap();

        let client_addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
        let client_endpoint = create_endpoint(client_addr).expect("client endpoint");

        // Server accepts in background
        let server_handle = tokio::spawn(async move {
            let incoming = server_endpoint.accept().await.expect("no incoming");
            let conn = incoming.await.expect("connection failed");
            assert!(conn.remote_address().port() > 0);
            conn.close(0u32.into(), b"done");
            server_endpoint.close(0u32.into(), b"shutdown");
        });

        // Client connects
        let conn = client_endpoint
            .connect(server_addr, "localhost")
            .expect("connect call failed")
            .await
            .expect("connection failed");

        assert!(conn.remote_address().port() > 0);

        conn.close(0u32.into(), b"done");
        client_endpoint.close(0u32.into(), b"shutdown");

        server_handle.await.unwrap();
    }
}

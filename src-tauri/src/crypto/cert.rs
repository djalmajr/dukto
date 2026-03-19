use std::sync::Arc;

/// Generate a self-signed TLS certificate for QUIC transport.
/// Returns (rustls ServerConfig, rustls ClientConfig).
///
/// The client config is configured to skip server certificate verification
/// because both sides use ephemeral self-signed certs. Real authentication
/// happens at the Noise layer, not at TLS.
pub fn generate_self_signed_config(
) -> Result<(rustls::ServerConfig, rustls::ClientConfig), Box<dyn std::error::Error>> {
    let cert = rcgen::generate_simple_self_signed(vec!["localhost".into()])?;

    let cert_der = rustls::pki_types::CertificateDer::from(cert.cert);
    let key_der = rustls::pki_types::PrivateKeyDer::try_from(cert.key_pair.serialize_der())
        .map_err(|e| format!("Failed to parse private key: {}", e))?;

    // Server config
    let server_config = rustls::ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(vec![cert_der], key_der)?;

    // Client config — skip verification (we authenticate via Noise)
    let client_config = rustls::ClientConfig::builder()
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(SkipServerVerification))
        .with_no_client_auth();

    Ok((server_config, client_config))
}

/// Skips TLS server certificate verification.
/// Safety: authentication is handled by the Noise Protocol layer.
#[derive(Debug)]
struct SkipServerVerification;

impl rustls::client::danger::ServerCertVerifier for SkipServerVerification {
    fn verify_server_cert(
        &self,
        _end_entity: &rustls::pki_types::CertificateDer<'_>,
        _intermediates: &[rustls::pki_types::CertificateDer<'_>],
        _server_name: &rustls::pki_types::ServerName<'_>,
        _ocsp_response: &[u8],
        _now: rustls::pki_types::UnixTime,
    ) -> Result<rustls::client::danger::ServerCertVerified, rustls::Error> {
        Ok(rustls::client::danger::ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        _message: &[u8],
        _cert: &rustls::pki_types::CertificateDer<'_>,
        _dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
    }

    fn verify_tls13_signature(
        &self,
        _message: &[u8],
        _cert: &rustls::pki_types::CertificateDer<'_>,
        _dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
    }

    fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
        rustls::crypto::ring::default_provider()
            .signature_verification_algorithms
            .supported_schemes()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generates_valid_server_and_client_config() {
        let (server_config, client_config) =
            generate_self_signed_config().expect("Should generate self-signed config");

        // Server config should be usable by quinn
        let _ = quinn::ServerConfig::with_crypto(Arc::new(
            quinn::crypto::rustls::QuicServerConfig::try_from(server_config).unwrap(),
        ));

        // Client config should be usable by quinn
        let _ = quinn::ClientConfig::new(Arc::new(
            quinn::crypto::rustls::QuicClientConfig::try_from(client_config).unwrap(),
        ));
    }
}

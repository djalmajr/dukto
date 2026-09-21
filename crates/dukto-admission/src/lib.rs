use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use ring::{hkdf, hmac};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use zeroize::{Zeroize, ZeroizeOnDrop};

pub const ADMISSION_VERSION: u8 = 1;
pub const CAPABILITY_BYTES: usize = 32;
pub const MAX_ADMISSION_TTL_SECS: u64 = 20 * 60;

const CAPABILITY_LABEL: &[u8] = b"dukto/internet/v1/relay-admission";
const JOIN_PROOF_LABEL: &[u8] = b"dukto/internet/v1/relay-admission/join";

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum AdmissionError {
    #[error("admission input is invalid")]
    InvalidInput,
    #[error("admission capability is invalid")]
    InvalidCapability,
    #[error("admission proof is invalid")]
    InvalidProof,
    #[error("admission derivation failed")]
    Derivation,
}

#[derive(Clone, Zeroize, ZeroizeOnDrop)]
pub struct AdmissionCapability([u8; CAPABILITY_BYTES]);

impl std::fmt::Debug for AdmissionCapability {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("AdmissionCapability([REDACTED])")
    }
}

impl AdmissionCapability {
    pub fn derive(invitation_secret: &[u8], session_id: &str) -> Result<Self, AdmissionError> {
        if invitation_secret.len() < 16 || !is_safe_session_id(session_id) {
            return Err(AdmissionError::InvalidInput);
        }
        let salt = hkdf::Salt::new(hkdf::HKDF_SHA256, session_id.as_bytes());
        let prk = salt.extract(invitation_secret);
        let labels = [CAPABILITY_LABEL];
        let okm = prk
            .expand(&labels, hkdf::HKDF_SHA256)
            .map_err(|_| AdmissionError::Derivation)?;
        let mut output = [0_u8; CAPABILITY_BYTES];
        okm.fill(&mut output)
            .map_err(|_| AdmissionError::Derivation)?;
        Ok(Self(output))
    }

    pub fn decode(value: &str) -> Result<Self, AdmissionError> {
        let decoded = URL_SAFE_NO_PAD
            .decode(value)
            .map_err(|_| AdmissionError::InvalidCapability)?;
        let bytes: [u8; CAPABILITY_BYTES] = decoded
            .try_into()
            .map_err(|_| AdmissionError::InvalidCapability)?;
        Ok(Self(bytes))
    }

    pub fn encode(&self) -> String {
        URL_SAFE_NO_PAD.encode(self.0)
    }

    pub fn join_proof(
        &self,
        slot: &str,
        endpoint_id: &str,
        expires_at_unix: u64,
    ) -> Result<String, AdmissionError> {
        validate_admission_fields(slot, endpoint_id, expires_at_unix)?;
        let key = hmac::Key::new(hmac::HMAC_SHA256, &self.0);
        Ok(URL_SAFE_NO_PAD.encode(hmac::sign(
            &key,
            &proof_message(slot, endpoint_id, expires_at_unix),
        )))
    }

    pub fn verify_join_proof(
        &self,
        slot: &str,
        endpoint_id: &str,
        expires_at_unix: u64,
        proof: &str,
    ) -> Result<(), AdmissionError> {
        validate_admission_fields(slot, endpoint_id, expires_at_unix)?;
        let proof = URL_SAFE_NO_PAD
            .decode(proof)
            .map_err(|_| AdmissionError::InvalidProof)?;
        let key = hmac::Key::new(hmac::HMAC_SHA256, &self.0);
        hmac::verify(
            &key,
            &proof_message(slot, endpoint_id, expires_at_unix),
            &proof,
        )
        .map_err(|_| AdmissionError::InvalidProof)
    }
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize, Zeroize, ZeroizeOnDrop)]
#[serde(deny_unknown_fields)]
pub struct AdmissionRegistration {
    pub endpoint_id: String,
    pub capability: String,
}

impl std::fmt::Debug for AdmissionRegistration {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("AdmissionRegistration")
            .field("endpoint_id", &self.endpoint_id)
            .field("capability", &"[REDACTED]")
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PublishRequest<E> {
    pub envelope: E,
    pub admission: AdmissionRegistration,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize, Zeroize, ZeroizeOnDrop)]
#[serde(deny_unknown_fields)]
pub struct ConsumeRequest {
    pub endpoint_id: String,
    pub proof: String,
}

impl std::fmt::Debug for ConsumeRequest {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ConsumeRequest")
            .field("endpoint_id", &self.endpoint_id)
            .field("proof", &"[REDACTED]")
            .finish()
    }
}

pub fn is_valid_slot(slot: &str) -> bool {
    slot.len() == 64
        && slot.bytes().all(|byte| byte.is_ascii_hexdigit())
        && !slot.bytes().any(|byte| byte.is_ascii_uppercase())
}

pub fn is_valid_endpoint_id(endpoint_id: &str) -> bool {
    endpoint_id.len() == 64
        && endpoint_id.bytes().all(|byte| byte.is_ascii_hexdigit())
        && !endpoint_id.bytes().any(|byte| byte.is_ascii_uppercase())
}

fn validate_admission_fields(
    slot: &str,
    endpoint_id: &str,
    expires_at_unix: u64,
) -> Result<(), AdmissionError> {
    if !is_valid_slot(slot) || !is_valid_endpoint_id(endpoint_id) || expires_at_unix == 0 {
        return Err(AdmissionError::InvalidInput);
    }
    Ok(())
}

fn is_safe_session_id(session_id: &str) -> bool {
    !session_id.is_empty()
        && session_id.len() <= 128
        && session_id
            .bytes()
            .all(|byte| byte.is_ascii_graphic() && !byte.is_ascii_whitespace())
}

fn proof_message(slot: &str, endpoint_id: &str, expires_at_unix: u64) -> Vec<u8> {
    let mut output = Vec::with_capacity(
        JOIN_PROOF_LABEL.len() + 1 + slot.len() + endpoint_id.len() + std::mem::size_of::<u64>(),
    );
    output.extend_from_slice(JOIN_PROOF_LABEL);
    output.push(ADMISSION_VERSION);
    output.extend_from_slice(slot.as_bytes());
    output.extend_from_slice(endpoint_id.as_bytes());
    output.extend_from_slice(&expires_at_unix.to_be_bytes());
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    const SLOT: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const ENDPOINT: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

    #[test]
    fn capability_is_scoped_and_redacted() {
        let first = AdmissionCapability::derive(&[7; 32], "session-a").unwrap();
        let second = AdmissionCapability::derive(&[7; 32], "session-b").unwrap();
        assert_ne!(first.encode(), second.encode());
        assert_eq!(format!("{first:?}"), "AdmissionCapability([REDACTED])");
        assert_eq!(
            AdmissionCapability::decode(&first.encode())
                .unwrap()
                .encode(),
            first.encode()
        );
    }

    #[test]
    fn proof_binds_slot_endpoint_and_expiry() {
        let capability = AdmissionCapability::derive(&[9; 32], "session-a").unwrap();
        let proof = capability
            .join_proof(SLOT, ENDPOINT, 1_800_000_900)
            .unwrap();
        capability
            .verify_join_proof(SLOT, ENDPOINT, 1_800_000_900, &proof)
            .unwrap();

        let other_endpoint = "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc";
        assert_eq!(
            capability.verify_join_proof(SLOT, other_endpoint, 1_800_000_900, &proof),
            Err(AdmissionError::InvalidProof)
        );
        assert_eq!(
            capability.verify_join_proof(SLOT, ENDPOINT, 1_800_000_901, &proof),
            Err(AdmissionError::InvalidProof)
        );
    }

    #[test]
    fn malformed_values_fail_closed() {
        assert!(AdmissionCapability::decode("not-base64!").is_err());
        assert!(!is_valid_slot("short"));
        assert!(!is_valid_endpoint_id(&"A".repeat(64)));
    }

    #[test]
    fn wire_secrets_are_redacted_from_debug() {
        let registration = AdmissionRegistration {
            endpoint_id: ENDPOINT.to_owned(),
            capability: "capability-secret".to_owned(),
        };
        let consume = ConsumeRequest {
            endpoint_id: ENDPOINT.to_owned(),
            proof: "proof-secret".to_owned(),
        };
        assert!(!format!("{registration:?}").contains("capability-secret"));
        assert!(!format!("{consume:?}").contains("proof-secret"));
    }
}

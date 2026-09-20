use std::fmt;

use ring::rand::{SecureRandom, SystemRandom};
use spake2::{Ed25519Group, Identity, Password, Spake2};
use thiserror::Error;
use uuid::Uuid;

use crate::crypto::{hkdf_sha256, sha256};

pub const PAIRING_TRANSCRIPT_VERSION: u8 = 1;
pub const PAIRING_NONCE_BYTES: usize = 32;
pub const PAIRING_KEY_BYTES: usize = 32;
pub const DUKTO_INTERNET_ALPN: &[u8] = b"dukto/internet/1";
const MAX_PAIRING_LIFETIME_SECS: u64 = 15 * 60;
const MAX_TOKEN_LEN: usize = 256;
const TRANSCRIPT_DOMAIN: &[u8] = b"dukto-pairing-transcript\0";
const INITIATOR_CONFIRMATION_LABEL: &[u8] = b"dukto/internet/v1/initiator-confirmation";
const RESPONDER_CONFIRMATION_LABEL: &[u8] = b"dukto/internet/v1/responder-confirmation";
const NOISE_BINDING_LABEL: &[u8] = b"dukto/internet/v1/noise-binding";
const MANUAL_CODE_DIGITS: usize = 8;
const MANUAL_CODE_RANGE: u32 = 100_000_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PairingRole {
    Initiator,
    Responder,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PairingOrigin {
    InviteLink,
    ManualCode,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum PairingError {
    #[error("the pairing transcript is invalid")]
    InvalidTranscript,
    #[error("the pairing transcript version is not supported")]
    UnsupportedVersion,
    #[error("the pairing transcript has expired")]
    Expired,
    #[error("pairing key derivation failed")]
    KeyDerivation,
    #[error("the manual pairing code is invalid")]
    InvalidCode,
    #[error("the manual pairing exchange has already finished")]
    AlreadyFinished,
    #[error("the peer sent an invalid pairing message")]
    InvalidPeerMessage,
    #[error("secure manual pairing code generation failed")]
    Randomness,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PairingTranscript {
    version: u8,
    origin: PairingOrigin,
    session_id: String,
    initiator_endpoint_id: String,
    responder_endpoint_id: String,
    initiator_nonce: [u8; PAIRING_NONCE_BYTES],
    responder_nonce: [u8; PAIRING_NONCE_BYTES],
    alpn: Vec<u8>,
    expires_at_unix: u64,
}

impl PairingTranscript {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        origin: PairingOrigin,
        session_id: impl Into<String>,
        initiator_endpoint_id: impl Into<String>,
        responder_endpoint_id: impl Into<String>,
        initiator_nonce: [u8; PAIRING_NONCE_BYTES],
        responder_nonce: [u8; PAIRING_NONCE_BYTES],
        expires_at_unix: u64,
        now_unix: u64,
    ) -> Result<Self, PairingError> {
        if expires_at_unix <= now_unix {
            return Err(PairingError::Expired);
        }
        if expires_at_unix - now_unix > MAX_PAIRING_LIFETIME_SECS {
            return Err(PairingError::InvalidTranscript);
        }

        let transcript = Self {
            version: PAIRING_TRANSCRIPT_VERSION,
            origin,
            session_id: session_id.into(),
            initiator_endpoint_id: initiator_endpoint_id.into(),
            responder_endpoint_id: responder_endpoint_id.into(),
            initiator_nonce,
            responder_nonce,
            alpn: DUKTO_INTERNET_ALPN.to_vec(),
            expires_at_unix,
        };
        transcript.validate_shape()?;
        Ok(transcript)
    }

    pub fn canonical_bytes(&self) -> Result<Vec<u8>, PairingError> {
        self.validate_shape()?;

        let mut output = Vec::with_capacity(256);
        output.extend_from_slice(TRANSCRIPT_DOMAIN);
        output.push(self.version);
        output.push(self.origin.code());
        append_field(&mut output, b"session", self.session_id.as_bytes());
        append_field(
            &mut output,
            b"initiator-endpoint",
            self.initiator_endpoint_id.as_bytes(),
        );
        append_field(
            &mut output,
            b"responder-endpoint",
            self.responder_endpoint_id.as_bytes(),
        );
        append_field(&mut output, b"initiator-nonce", &self.initiator_nonce);
        append_field(&mut output, b"responder-nonce", &self.responder_nonce);
        append_field(&mut output, b"alpn", &self.alpn);
        append_field(
            &mut output,
            b"expires-at-unix",
            &self.expires_at_unix.to_be_bytes(),
        );
        Ok(output)
    }

    pub fn derive_keys(&self, shared_secret: &[u8]) -> Result<PairingKeys, PairingError> {
        if shared_secret.len() < 16 {
            return Err(PairingError::KeyDerivation);
        }
        let transcript_hash = sha256(&self.canonical_bytes()?);
        Ok(PairingKeys {
            initiator_confirmation: hkdf_sha256(
                shared_secret,
                &transcript_hash,
                INITIATOR_CONFIRMATION_LABEL,
            )
            .map_err(|_| PairingError::KeyDerivation)?,
            responder_confirmation: hkdf_sha256(
                shared_secret,
                &transcript_hash,
                RESPONDER_CONFIRMATION_LABEL,
            )
            .map_err(|_| PairingError::KeyDerivation)?,
            noise_binding: hkdf_sha256(shared_secret, &transcript_hash, NOISE_BINDING_LABEL)
                .map_err(|_| PairingError::KeyDerivation)?,
        })
    }

    fn validate_shape(&self) -> Result<(), PairingError> {
        if self.version != PAIRING_TRANSCRIPT_VERSION {
            return Err(PairingError::UnsupportedVersion);
        }
        if Uuid::parse_str(&self.session_id).is_err()
            || !is_safe_token(&self.initiator_endpoint_id)
            || !is_safe_token(&self.responder_endpoint_id)
            || self.initiator_endpoint_id == self.responder_endpoint_id
            || self.initiator_nonce.iter().all(|byte| *byte == 0)
            || self.responder_nonce.iter().all(|byte| *byte == 0)
            || self.alpn.is_empty()
            || self.alpn.len() > u8::MAX as usize
            || self.expires_at_unix == 0
        {
            return Err(PairingError::InvalidTranscript);
        }
        Ok(())
    }
}

impl PairingOrigin {
    const fn code(self) -> u8 {
        match self {
            Self::InviteLink => 1,
            Self::ManualCode => 2,
        }
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct ManualPairingCode(String);

impl ManualPairingCode {
    pub fn parse(code: impl Into<String>) -> Result<Self, PairingError> {
        let code = code.into();
        if code.len() != MANUAL_CODE_DIGITS
            || !code.bytes().all(|character| character.is_ascii_digit())
        {
            return Err(PairingError::InvalidCode);
        }
        Ok(Self(code))
    }

    pub fn generate() -> Result<Self, PairingError> {
        let random = SystemRandom::new();
        let unbiased_limit = u32::MAX - (u32::MAX % MANUAL_CODE_RANGE);
        loop {
            let mut bytes = [0_u8; std::mem::size_of::<u32>()];
            random
                .fill(&mut bytes)
                .map_err(|_| PairingError::Randomness)?;
            let value = u32::from_be_bytes(bytes);
            if value < unbiased_limit {
                return Ok(Self(format!(
                    "{:0width$}",
                    value % MANUAL_CODE_RANGE,
                    width = MANUAL_CODE_DIGITS
                )));
            }
        }
    }

    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for ManualPairingCode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("ManualPairingCode([REDACTED])")
    }
}

pub struct ManualCodePairing {
    exchange: Option<Spake2<Ed25519Group>>,
    transcript: PairingTranscript,
}

impl ManualCodePairing {
    pub fn start(
        role: PairingRole,
        code: &ManualPairingCode,
        transcript: &PairingTranscript,
    ) -> Result<(Self, Vec<u8>), PairingError> {
        if transcript.origin != PairingOrigin::ManualCode {
            return Err(PairingError::InvalidTranscript);
        }
        let transcript_hash = sha256(&transcript.canonical_bytes()?);
        let initiator_identity = pairing_identity(b"initiator", &transcript_hash);
        let responder_identity = pairing_identity(b"responder", &transcript_hash);
        let password = Password::new(code.expose());
        let (exchange, message) = match role {
            PairingRole::Initiator => {
                Spake2::<Ed25519Group>::start_a(&password, &initiator_identity, &responder_identity)
            }
            PairingRole::Responder => {
                Spake2::<Ed25519Group>::start_b(&password, &initiator_identity, &responder_identity)
            }
        };
        Ok((
            Self {
                exchange: Some(exchange),
                transcript: transcript.clone(),
            },
            message,
        ))
    }

    pub fn finish(&mut self, peer_message: &[u8]) -> Result<PairingKeys, PairingError> {
        let exchange = self.exchange.take().ok_or(PairingError::AlreadyFinished)?;
        let shared_secret = exchange
            .finish(peer_message)
            .map_err(|_| PairingError::InvalidPeerMessage)?;
        self.transcript.derive_keys(&shared_secret)
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct PairingKeys {
    initiator_confirmation: [u8; PAIRING_KEY_BYTES],
    responder_confirmation: [u8; PAIRING_KEY_BYTES],
    noise_binding: [u8; PAIRING_KEY_BYTES],
}

impl PairingKeys {
    pub fn confirmation(&self, role: PairingRole) -> &[u8; PAIRING_KEY_BYTES] {
        match role {
            PairingRole::Initiator => &self.initiator_confirmation,
            PairingRole::Responder => &self.responder_confirmation,
        }
    }

    pub fn noise_binding(&self) -> &[u8; PAIRING_KEY_BYTES] {
        &self.noise_binding
    }
}

impl fmt::Debug for PairingKeys {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PairingKeys")
            .field("initiator_confirmation", &"[REDACTED]")
            .field("responder_confirmation", &"[REDACTED]")
            .field("noise_binding", &"[REDACTED]")
            .finish()
    }
}

fn is_safe_token(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_TOKEN_LEN
        && value.trim() == value
        && value.chars().all(|character| {
            character.is_ascii() && !character.is_ascii_control() && !character.is_whitespace()
        })
}

fn append_field(output: &mut Vec<u8>, label: &[u8], value: &[u8]) {
    output.extend_from_slice(&(label.len() as u32).to_be_bytes());
    output.extend_from_slice(label);
    output.extend_from_slice(&(value.len() as u32).to_be_bytes());
    output.extend_from_slice(value);
}

fn pairing_identity(role: &[u8], transcript_hash: &[u8; PAIRING_KEY_BYTES]) -> Identity {
    let mut identity = Vec::with_capacity(role.len() + transcript_hash.len() + 1);
    identity.extend_from_slice(b"dukto-manual-pairing");
    identity.push(0);
    identity.extend_from_slice(role);
    identity.extend_from_slice(transcript_hash);
    Identity::new(&identity)
}

#[cfg(test)]
mod tests {
    use uuid::Uuid;

    use super::{
        ManualCodePairing, ManualPairingCode, PairingError, PairingKeys, PairingOrigin,
        PairingRole, PairingTranscript, DUKTO_INTERNET_ALPN, PAIRING_KEY_BYTES,
        PAIRING_TRANSCRIPT_VERSION,
    };

    const NOW: u64 = 1_800_000_000;
    const SECRET: [u8; 32] = [0x42; 32];

    fn transcript() -> PairingTranscript {
        PairingTranscript::new(
            PairingOrigin::InviteLink,
            "018f1f3a-7730-7a2f-92d8-314278523b67",
            "initiator-endpoint",
            "responder-endpoint",
            [0x11; 32],
            [0x22; 32],
            NOW + 300,
            NOW,
        )
        .expect("valid transcript")
    }

    fn manual_transcript() -> PairingTranscript {
        let mut transcript = transcript();
        transcript.origin = PairingOrigin::ManualCode;
        transcript
    }

    #[test]
    fn both_peers_derive_matching_keys_with_independent_role_labels() {
        let first = transcript().derive_keys(&SECRET).expect("pairing keys");
        let second = transcript().derive_keys(&SECRET).expect("pairing keys");

        assert_eq!(first, second);
        assert_ne!(
            first.confirmation(PairingRole::Initiator),
            first.confirmation(PairingRole::Responder)
        );
        assert_ne!(
            first.confirmation(PairingRole::Initiator),
            first.noise_binding()
        );
        assert_ne!(
            first.confirmation(PairingRole::Responder),
            first.noise_binding()
        );
        assert_eq!(first.noise_binding().len(), PAIRING_KEY_BYTES);
    }

    #[test]
    fn every_transcript_field_changes_or_invalidates_confirmation() {
        let baseline = transcript()
            .derive_keys(&SECRET)
            .expect("baseline keys")
            .confirmation(PairingRole::Initiator)
            .to_owned();

        let mut variants = Vec::new();

        let mut changed = transcript();
        changed.origin = PairingOrigin::ManualCode;
        variants.push(changed);

        let mut changed = transcript();
        changed.session_id = Uuid::new_v4().to_string();
        variants.push(changed);

        let mut changed = transcript();
        std::mem::swap(
            &mut changed.initiator_endpoint_id,
            &mut changed.responder_endpoint_id,
        );
        variants.push(changed);

        let mut changed = transcript();
        changed.initiator_nonce[0] ^= 1;
        variants.push(changed);

        let mut changed = transcript();
        changed.responder_nonce[0] ^= 1;
        variants.push(changed);

        let mut changed = transcript();
        changed.alpn = b"dukto/internet/2".to_vec();
        variants.push(changed);

        let mut changed = transcript();
        changed.expires_at_unix += 1;
        variants.push(changed);

        for changed in variants {
            let changed_confirmation = changed
                .derive_keys(&SECRET)
                .expect("changed transcript remains structurally valid")
                .confirmation(PairingRole::Initiator)
                .to_owned();
            assert_ne!(changed_confirmation, baseline);
        }

        let mut unsupported = transcript();
        unsupported.version = PAIRING_TRANSCRIPT_VERSION + 1;
        assert_eq!(
            unsupported.derive_keys(&SECRET),
            Err(PairingError::UnsupportedVersion)
        );
    }

    #[test]
    fn canonical_transcript_is_stable_and_contains_the_versioned_context() {
        let first = transcript()
            .canonical_bytes()
            .expect("canonical transcript");
        let second = transcript()
            .canonical_bytes()
            .expect("canonical transcript");

        assert_eq!(first, second);
        assert!(first.starts_with(b"dukto-pairing-transcript\0"));
        assert!(first
            .windows(DUKTO_INTERNET_ALPN.len())
            .any(|window| window == DUKTO_INTERNET_ALPN));
    }

    #[test]
    fn rejects_expired_ambiguous_or_secretless_pairing() {
        assert_eq!(
            PairingTranscript::new(
                PairingOrigin::InviteLink,
                "018f1f3a-7730-7a2f-92d8-314278523b67",
                "same-endpoint",
                "same-endpoint",
                [0x11; 32],
                [0x22; 32],
                NOW,
                NOW,
            ),
            Err(PairingError::Expired)
        );

        let mut ambiguous = transcript();
        ambiguous.responder_endpoint_id = ambiguous.initiator_endpoint_id.clone();
        assert_eq!(
            ambiguous.derive_keys(&SECRET),
            Err(PairingError::InvalidTranscript)
        );
        assert_eq!(
            transcript().derive_keys(&[]),
            Err(PairingError::KeyDerivation)
        );
    }

    #[test]
    fn pairing_key_debug_output_is_redacted() {
        let keys = transcript().derive_keys(&SECRET).expect("pairing keys");
        let debug = format!("{keys:?}");

        assert!(debug.matches("[REDACTED]").count() == 3);
        assert!(!debug.contains(&hex(keys.confirmation(PairingRole::Initiator))));
        assert!(!debug.contains(&hex(keys.confirmation(PairingRole::Responder))));
        assert!(!debug.contains(&hex(keys.noise_binding())));
    }

    #[test]
    fn matching_manual_codes_complete_pake_and_confirm_the_same_keys() {
        let code = ManualPairingCode::parse("31415926").expect("valid code");
        let transcript = manual_transcript();
        let (mut initiator, initiator_message) =
            ManualCodePairing::start(PairingRole::Initiator, &code, &transcript)
                .expect("initiator exchange");
        let (mut responder, responder_message) =
            ManualCodePairing::start(PairingRole::Responder, &code, &transcript)
                .expect("responder exchange");

        let initiator_keys = initiator
            .finish(&responder_message)
            .expect("initiator keys");
        let responder_keys = responder
            .finish(&initiator_message)
            .expect("responder keys");

        assert_eq!(initiator_keys, responder_keys);
        assert_eq!(
            initiator_keys.confirmation(PairingRole::Initiator),
            responder_keys.confirmation(PairingRole::Initiator)
        );
        assert_eq!(
            initiator_keys.noise_binding(),
            responder_keys.noise_binding()
        );
    }

    #[test]
    fn wrong_code_or_changed_transcript_cannot_confirm() {
        let transcript = manual_transcript();
        let correct = ManualPairingCode::parse("31415926").expect("valid code");
        let wrong = ManualPairingCode::parse("27182818").expect("valid code");

        let (mut initiator, initiator_message) =
            ManualCodePairing::start(PairingRole::Initiator, &correct, &transcript)
                .expect("initiator exchange");
        let (mut responder, responder_message) =
            ManualCodePairing::start(PairingRole::Responder, &wrong, &transcript)
                .expect("responder exchange");
        let initiator_keys = initiator.finish(&responder_message).expect("SPAKE2 output");
        let responder_keys = responder.finish(&initiator_message).expect("SPAKE2 output");
        assert_ne!(
            initiator_keys.confirmation(PairingRole::Initiator),
            responder_keys.confirmation(PairingRole::Initiator)
        );

        let mut changed_transcript = transcript.clone();
        changed_transcript.expires_at_unix += 1;
        let (mut initiator, initiator_message) =
            ManualCodePairing::start(PairingRole::Initiator, &correct, &transcript)
                .expect("initiator exchange");
        let (mut responder, responder_message) =
            ManualCodePairing::start(PairingRole::Responder, &correct, &changed_transcript)
                .expect("responder exchange");
        let initiator_keys = initiator.finish(&responder_message).expect("SPAKE2 output");
        let responder_keys = responder.finish(&initiator_message).expect("SPAKE2 output");
        assert_ne!(
            initiator_keys.confirmation(PairingRole::Responder),
            responder_keys.confirmation(PairingRole::Responder)
        );
    }

    #[test]
    fn manual_pairing_state_is_single_use_and_rejects_replay() {
        let code = ManualPairingCode::parse("31415926").expect("valid code");
        let transcript = manual_transcript();
        let (mut initiator, _) =
            ManualCodePairing::start(PairingRole::Initiator, &code, &transcript)
                .expect("initiator exchange");
        let (_, responder_message) =
            ManualCodePairing::start(PairingRole::Responder, &code, &transcript)
                .expect("responder exchange");

        initiator
            .finish(&responder_message)
            .expect("first completion succeeds");
        assert_eq!(
            initiator.finish(&responder_message),
            Err(PairingError::AlreadyFinished)
        );
    }

    #[test]
    fn public_pake_messages_do_not_reveal_the_manual_code() {
        let code = ManualPairingCode::parse("31415926").expect("valid code");
        let generated = ManualPairingCode::generate().expect("generated code");
        let transcript = manual_transcript();
        let (_, first_message) =
            ManualCodePairing::start(PairingRole::Initiator, &code, &transcript)
                .expect("first exchange");
        let (_, second_message) =
            ManualCodePairing::start(PairingRole::Initiator, &code, &transcript)
                .expect("second exchange");

        assert_ne!(first_message, second_message);
        assert!(!first_message
            .windows(code.expose().len())
            .any(|window| window == code.expose().as_bytes()));
        assert!(format!("{code:?}").contains("[REDACTED]"));
        assert!(!format!("{code:?}").contains(code.expose()));
        assert_eq!(generated.expose().len(), 8);
        assert!(generated.expose().bytes().all(|byte| byte.is_ascii_digit()));
        assert!(ManualPairingCode::parse("1234").is_err());
        assert!(ManualPairingCode::parse("abcdefgh").is_err());
    }

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|byte| format!("{byte:02x}")).collect()
    }

    const _: fn() = || {
        let _ = std::mem::size_of::<PairingKeys>();
    };
}

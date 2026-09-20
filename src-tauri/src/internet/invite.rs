use std::fmt;

use serde::{de::Error as DeserializeError, Deserialize, Deserializer, Serialize, Serializer};
use thiserror::Error;
use uuid::Uuid;

pub const INTERNET_INVITE_VERSION: u8 = 1;
pub const INVITE_SECRET_BYTES: usize = 32;
pub const MAX_INVITE_TTL_SECS: u64 = 15 * 60;
pub const MAX_CANONICAL_INVITE_BYTES: usize = 8 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum InviteError {
    #[error("the internet invitation is malformed")]
    Malformed,
    #[error("the internet invitation version is not supported")]
    UnsupportedVersion,
    #[error("the internet invitation is not in canonical form")]
    NonCanonical,
    #[error("the internet invitation has invalid endpoint data")]
    InvalidEndpoint,
    #[error("the internet invitation has invalid routing data")]
    InvalidRouting,
    #[error("the internet invitation lifetime is invalid")]
    InvalidLifetime,
    #[error("the internet invitation has expired")]
    Expired,
}

const MAX_ENDPOINT_ID_LEN: usize = 256;
const MAX_ROUTE_COUNT: usize = 16;
const MAX_ROUTE_LEN: usize = 512;
const MAX_RENDEZVOUS_SLOT_LEN: usize = 256;

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InternetInvite {
    version: u8,
    session_id: String,
    endpoint_id: String,
    addresses: Vec<String>,
    rendezvous_slot: Option<String>,
    issued_at_unix: u64,
    expires_at_unix: u64,
    #[serde(
        serialize_with = "serialize_secret",
        deserialize_with = "deserialize_secret"
    )]
    secret: [u8; INVITE_SECRET_BYTES],
}

impl InternetInvite {
    pub fn new(
        endpoint_id: impl Into<String>,
        mut addresses: Vec<String>,
        rendezvous_slot: Option<String>,
        issued_at_unix: u64,
        ttl_secs: u64,
    ) -> Result<Self, InviteError> {
        if ttl_secs == 0 || ttl_secs > MAX_INVITE_TTL_SECS {
            return Err(InviteError::InvalidLifetime);
        }
        let expires_at_unix = issued_at_unix
            .checked_add(ttl_secs)
            .ok_or(InviteError::InvalidLifetime)?;

        addresses.sort_unstable();
        addresses.dedup();

        let mut secret = [0_u8; INVITE_SECRET_BYTES];
        secret[..16].copy_from_slice(Uuid::new_v4().as_bytes());
        secret[16..].copy_from_slice(Uuid::new_v4().as_bytes());

        let invite = Self {
            version: INTERNET_INVITE_VERSION,
            session_id: Uuid::new_v4().to_string(),
            endpoint_id: endpoint_id.into(),
            addresses,
            rendezvous_slot,
            issued_at_unix,
            expires_at_unix,
            secret,
        };
        invite.validate_shape()?;
        Ok(invite)
    }

    pub fn from_canonical_json(encoded: &str, now_unix: u64) -> Result<Self, InviteError> {
        if encoded.len() > MAX_CANONICAL_INVITE_BYTES {
            return Err(InviteError::Malformed);
        }
        let invite: Self = serde_json::from_str(encoded).map_err(|_| InviteError::Malformed)?;
        invite.validate_shape()?;
        invite.validate_at(now_unix)?;
        if invite.to_canonical_json()? != encoded {
            return Err(InviteError::NonCanonical);
        }
        Ok(invite)
    }

    pub fn to_canonical_json(&self) -> Result<String, InviteError> {
        self.validate_shape()?;
        serde_json::to_string(self).map_err(|_| InviteError::Malformed)
    }

    pub fn endpoint_id(&self) -> &str {
        &self.endpoint_id
    }

    pub fn expires_at_unix(&self) -> u64 {
        self.expires_at_unix
    }

    pub fn session_id(&self) -> &str {
        &self.session_id
    }

    pub fn addresses(&self) -> &[String] {
        &self.addresses
    }

    #[cfg(feature = "app-common")]
    pub(crate) fn secret_copy_for_registry(&self) -> [u8; INVITE_SECRET_BYTES] {
        self.secret
    }

    pub(super) fn rendezvous_slot(&self) -> Option<&str> {
        self.rendezvous_slot.as_deref()
    }

    pub(super) fn secret_bytes(&self) -> &[u8; INVITE_SECRET_BYTES] {
        &self.secret
    }

    fn validate_shape(&self) -> Result<(), InviteError> {
        if self.version != INTERNET_INVITE_VERSION {
            return Err(InviteError::UnsupportedVersion);
        }
        if Uuid::parse_str(&self.session_id).is_err()
            || !is_safe_token(&self.endpoint_id, MAX_ENDPOINT_ID_LEN)
        {
            return Err(InviteError::InvalidEndpoint);
        }
        if self.addresses.len() > MAX_ROUTE_COUNT
            || self
                .addresses
                .iter()
                .any(|address| !is_safe_token(address, MAX_ROUTE_LEN))
            || self
                .addresses
                .windows(2)
                .any(|pair| pair[0].as_str() >= pair[1].as_str())
        {
            return Err(InviteError::InvalidRouting);
        }
        if let Some(slot) = &self.rendezvous_slot {
            if !is_safe_token(slot, MAX_RENDEZVOUS_SLOT_LEN)
                || slot.len() != 64
                || !slot.bytes().all(|byte| byte.is_ascii_hexdigit())
                || slot.bytes().any(|byte| byte.is_ascii_uppercase())
            {
                return Err(InviteError::InvalidRouting);
            }
        }
        if self.addresses.is_empty() && self.rendezvous_slot.is_none() {
            return Err(InviteError::InvalidRouting);
        }

        let lifetime = self
            .expires_at_unix
            .checked_sub(self.issued_at_unix)
            .ok_or(InviteError::InvalidLifetime)?;
        if lifetime == 0 || lifetime > MAX_INVITE_TTL_SECS {
            return Err(InviteError::InvalidLifetime);
        }
        Ok(())
    }

    pub(super) fn validate_at(&self, now_unix: u64) -> Result<(), InviteError> {
        if now_unix >= self.expires_at_unix {
            return Err(InviteError::Expired);
        }
        Ok(())
    }
}

impl fmt::Debug for InternetInvite {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("InternetInvite")
            .field("version", &self.version)
            .field("session_id", &self.session_id)
            .field("endpoint_id", &self.endpoint_id)
            .field("addresses", &self.addresses)
            .field("rendezvous_slot", &self.rendezvous_slot)
            .field("issued_at_unix", &self.issued_at_unix)
            .field("expires_at_unix", &self.expires_at_unix)
            .field("secret", &"[REDACTED]")
            .finish()
    }
}

fn is_safe_token(value: &str, max_len: usize) -> bool {
    !value.is_empty()
        && value.len() <= max_len
        && value.trim() == value
        && value.chars().all(|character| {
            character.is_ascii() && !character.is_ascii_control() && !character.is_whitespace()
        })
}

fn serialize_secret<S>(secret: &[u8; INVITE_SECRET_BYTES], serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    let mut encoded = String::with_capacity(INVITE_SECRET_BYTES * 2);
    for byte in secret {
        use std::fmt::Write;
        write!(&mut encoded, "{byte:02x}").expect("writing to a string cannot fail");
    }
    serializer.serialize_str(&encoded)
}

fn deserialize_secret<'de, D>(deserializer: D) -> Result<[u8; INVITE_SECRET_BYTES], D::Error>
where
    D: Deserializer<'de>,
{
    let encoded = String::deserialize(deserializer)?;
    if encoded.len() != INVITE_SECRET_BYTES * 2 || !encoded.is_ascii() {
        return Err(D::Error::custom("invalid invitation secret"));
    }

    let mut secret = [0_u8; INVITE_SECRET_BYTES];
    for (index, output) in secret.iter_mut().enumerate() {
        let offset = index * 2;
        *output = u8::from_str_radix(&encoded[offset..offset + 2], 16)
            .map_err(|_| D::Error::custom("invalid invitation secret"))?;
    }
    Ok(secret)
}

#[cfg(test)]
mod tests {
    use super::{
        InternetInvite, InviteError, INTERNET_INVITE_VERSION, INVITE_SECRET_BYTES,
        MAX_CANONICAL_INVITE_BYTES, MAX_INVITE_TTL_SECS,
    };

    const NOW: u64 = 1_800_000_000;

    fn invite() -> InternetInvite {
        InternetInvite::new(
            "endpoint-public-key",
            vec![
                "relay:https://relay.dukto.test".to_owned(),
                "ip:203.0.113.8:4242".to_owned(),
            ],
            Some("ab".repeat(32)),
            NOW,
            300,
        )
        .expect("valid invitation")
    }

    #[test]
    fn canonical_round_trip_preserves_public_fields_and_secret() {
        let original = invite();
        let encoded = original.to_canonical_json().expect("canonical JSON");
        let decoded =
            InternetInvite::from_canonical_json(&encoded, NOW + 1).expect("valid round trip");

        assert_eq!(decoded.endpoint_id(), "endpoint-public-key");
        assert_eq!(decoded.expires_at_unix(), NOW + 300);
        assert_eq!(decoded.secret_bytes(), original.secret_bytes());
        assert_eq!(
            decoded.to_canonical_json().expect("canonical JSON"),
            encoded
        );
        assert!(!encoded.contains(char::is_whitespace));
    }

    #[test]
    fn generated_secrets_are_high_entropy_length_and_unique() {
        let first = invite();
        let second = invite();

        assert_eq!(first.secret_bytes().len(), INVITE_SECRET_BYTES);
        assert_ne!(first.secret_bytes(), second.secret_bytes());
    }

    #[test]
    fn rejects_expired_unsupported_noncanonical_and_invalid_lifetimes() {
        let original = invite();
        let encoded = original.to_canonical_json().expect("canonical JSON");

        assert_eq!(
            InternetInvite::from_canonical_json(&encoded, NOW + 300),
            Err(InviteError::Expired)
        );
        assert_eq!(
            InternetInvite::from_canonical_json(&format!(" {encoded}"), NOW + 1),
            Err(InviteError::NonCanonical)
        );

        let unsupported = encoded.replacen(
            &format!("\"version\":{INTERNET_INVITE_VERSION}"),
            "\"version\":99",
            1,
        );
        assert_eq!(
            InternetInvite::from_canonical_json(&unsupported, NOW + 1),
            Err(InviteError::UnsupportedVersion)
        );

        assert_eq!(
            InternetInvite::new("endpoint", vec!["ip:127.0.0.1:1".to_owned()], None, NOW, 0),
            Err(InviteError::InvalidLifetime)
        );
        assert_eq!(
            InternetInvite::new(
                "endpoint",
                vec!["ip:127.0.0.1:1".to_owned()],
                None,
                NOW,
                MAX_INVITE_TTL_SECS + 1,
            ),
            Err(InviteError::InvalidLifetime)
        );
    }

    #[test]
    fn rejects_missing_routes_unknown_fields_and_malformed_secrets() {
        assert_eq!(
            InternetInvite::new("endpoint", Vec::new(), None, NOW, 60),
            Err(InviteError::InvalidRouting)
        );

        let original = invite();
        let encoded = original.to_canonical_json().expect("canonical JSON");
        let with_unknown_field = encoded.replacen("{", "{\"extra\":true,", 1);
        assert_eq!(
            InternetInvite::from_canonical_json(&with_unknown_field, NOW + 1),
            Err(InviteError::Malformed)
        );

        let secret_hex = secret_hex(&original);
        let malformed_secret = encoded.replace(&secret_hex, "00");
        assert_eq!(
            InternetInvite::from_canonical_json(&malformed_secret, NOW + 1),
            Err(InviteError::Malformed)
        );

        let multibyte_secret = format!("{}€", "a".repeat(61));
        let malformed_multibyte = encoded.replace(&secret_hex, &multibyte_secret);
        assert_eq!(malformed_multibyte.len(), encoded.len());
        assert_eq!(
            InternetInvite::from_canonical_json(&malformed_multibyte, NOW + 1),
            Err(InviteError::Malformed)
        );
    }

    #[test]
    fn rejects_oversized_canonical_input_before_parsing() {
        let oversized = "x".repeat(MAX_CANONICAL_INVITE_BYTES + 1);
        assert_eq!(
            InternetInvite::from_canonical_json(&oversized, NOW),
            Err(InviteError::Malformed)
        );
    }

    #[test]
    fn debug_and_errors_never_expose_the_invitation_secret() {
        let original = invite();
        let secret_hex = secret_hex(&original);
        let debug = format!("{original:?}");
        assert!(debug.contains("[REDACTED]"));
        assert!(!debug.contains(&secret_hex));

        let encoded = original.to_canonical_json().expect("canonical JSON");
        let malformed = encoded.replace(&secret_hex, "not-a-secret");
        let error = InternetInvite::from_canonical_json(&malformed, NOW + 1)
            .expect_err("malformed secret must fail");
        assert!(!error.to_string().contains(&secret_hex));
        assert!(!format!("{error:?}").contains(&secret_hex));
    }

    fn secret_hex(invite: &InternetInvite) -> String {
        invite
            .secret_bytes()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect()
    }
}

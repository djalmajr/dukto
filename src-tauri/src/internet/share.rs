use std::fmt;

use base64::{
    engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD},
    Engine as _,
};
use qrcode::{render::svg, QrCode};
use thiserror::Error;
use zeroize::Zeroizing;

use crate::internet::{
    invite::InternetInvite,
    pairing::{ManualPairingCode, PairingError},
};

const DEEP_LINK_PREFIX: &str = "dukto://connect#v1.";
const MAX_DEEP_LINK_BYTES: usize = 8 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum InviteShareError {
    #[error("internet invitation could not be shared")]
    Encoding,
    #[error("internet invitation link is invalid")]
    InvalidLink,
    #[error("internet invitation link is too large")]
    LinkTooLarge,
    #[error("internet invitation code could not be generated")]
    ManualCode,
}

pub struct InviteShareArtifacts {
    pub qr_svg_data_url: String,
    pub manual_code: String,
    #[cfg(any(feature = "app-common", test))]
    pub(crate) deep_link: Zeroizing<String>,
}

impl InviteShareArtifacts {
    #[cfg(test)]
    pub(crate) fn deep_link_for_test(&self) -> &str {
        self.deep_link.as_str()
    }
}

impl fmt::Debug for InviteShareArtifacts {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("InviteShareArtifacts")
            .field("qr_svg_data_url_bytes", &self.qr_svg_data_url.len())
            .field("manual_code", &"[REDACTED]")
            .field("deep_link", &"[REDACTED]")
            .finish()
    }
}

pub fn create_share_artifacts(
    invite: &InternetInvite,
) -> Result<InviteShareArtifacts, InviteShareError> {
    let canonical = Zeroizing::new(
        invite
            .to_canonical_json()
            .map_err(|_| InviteShareError::Encoding)?,
    );
    let mut link = String::with_capacity(DEEP_LINK_PREFIX.len() + canonical.len() * 2);
    link.push_str(DEEP_LINK_PREFIX);
    URL_SAFE_NO_PAD.encode_string(canonical.as_bytes(), &mut link);
    if link.len() > MAX_DEEP_LINK_BYTES {
        return Err(InviteShareError::LinkTooLarge);
    }
    let deep_link = Zeroizing::new(link);
    let qr_svg = QrCode::new(deep_link.as_bytes())
        .map_err(|_| InviteShareError::Encoding)?
        .render::<svg::Color>()
        .min_dimensions(256, 256)
        .dark_color(svg::Color("#111827"))
        .light_color(svg::Color("#ffffff"))
        .build();
    let mut qr_svg_data_url = String::from("data:image/svg+xml;base64,");
    STANDARD.encode_string(qr_svg.as_bytes(), &mut qr_svg_data_url);
    let manual_code = ManualPairingCode::generate()
        .map_err(|error| match error {
            PairingError::Randomness => InviteShareError::ManualCode,
            _ => InviteShareError::Encoding,
        })?
        .expose()
        .to_owned();
    Ok(InviteShareArtifacts {
        qr_svg_data_url,
        manual_code,
        #[cfg(any(feature = "app-common", test))]
        deep_link,
    })
}

pub fn parse_deep_link(link: &str, now_unix: u64) -> Result<InternetInvite, InviteShareError> {
    if link.len() > MAX_DEEP_LINK_BYTES {
        return Err(InviteShareError::LinkTooLarge);
    }
    let encoded = link
        .strip_prefix(DEEP_LINK_PREFIX)
        .filter(|encoded| !encoded.is_empty())
        .ok_or(InviteShareError::InvalidLink)?;
    let decoded = Zeroizing::new(
        URL_SAFE_NO_PAD
            .decode(encoded.as_bytes())
            .map_err(|_| InviteShareError::InvalidLink)?,
    );
    let canonical = std::str::from_utf8(&decoded).map_err(|_| InviteShareError::InvalidLink)?;
    InternetInvite::from_canonical_json(canonical, now_unix)
        .map_err(|_| InviteShareError::InvalidLink)
}

#[cfg(test)]
mod tests {
    use base64::{engine::general_purpose::STANDARD, Engine as _};

    use crate::internet::invite::InternetInvite;
    use crate::internet::rendezvous::RendezvousSlot;

    use super::{create_share_artifacts, parse_deep_link};

    const NOW: u64 = 1_800_000_000;

    fn invite() -> InternetInvite {
        InternetInvite::new(
            "creator-endpoint",
            vec!["ip:203.0.113.10:4242".to_owned()],
            Some(RendezvousSlot::generate().unwrap().expose().to_owned()),
            NOW,
            300,
        )
        .unwrap()
    }

    #[test]
    fn qr_encodes_a_fragment_deep_link_without_plaintext_secret_in_the_svg() {
        let invite = invite();
        let canonical = invite.to_canonical_json().unwrap();
        let artifacts = create_share_artifacts(&invite).unwrap();

        assert!(artifacts
            .qr_svg_data_url
            .starts_with("data:image/svg+xml;base64,"));
        assert!(!artifacts.qr_svg_data_url.contains(&canonical));
        let encoded_svg = artifacts
            .qr_svg_data_url
            .strip_prefix("data:image/svg+xml;base64,")
            .unwrap();
        let decoded_svg = STANDARD.decode(encoded_svg).unwrap();
        let decoded_svg = String::from_utf8(decoded_svg).unwrap();
        assert!(decoded_svg.contains("<svg"));
        assert!(!decoded_svg.contains(&canonical));
        assert_eq!(artifacts.manual_code.len(), 8);
        assert!(artifacts
            .manual_code
            .bytes()
            .all(|byte| byte.is_ascii_digit()));
        assert!(artifacts
            .deep_link_for_test()
            .starts_with("dukto://connect#v1."));
    }

    #[test]
    fn deep_link_round_trip_is_strict_and_expiring() {
        let invite = invite();
        let artifacts = create_share_artifacts(&invite).unwrap();
        let parsed = parse_deep_link(artifacts.deep_link_for_test(), NOW + 1).unwrap();
        assert_eq!(
            parsed.to_canonical_json().unwrap(),
            invite.to_canonical_json().unwrap()
        );

        assert!(parse_deep_link("dukto://other#v1.invalid", NOW + 1).is_err());
        assert!(parse_deep_link(artifacts.deep_link_for_test(), NOW + 301).is_err());
    }
}

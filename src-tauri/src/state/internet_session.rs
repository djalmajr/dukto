use std::{
    collections::HashMap,
    fmt,
    sync::{
        atomic::{AtomicBool, Ordering},
        Mutex,
    },
};

use serde::{Deserialize, Serialize};
use thiserror::Error;
use zeroize::{Zeroize, Zeroizing};

use crate::{
    internet::{
        pairing::ManualPairingCode,
        session::{EstablishedInternetSession, InternetSessionCredentials, InternetSessionRole},
    },
    state::device::DeviceIdentity,
    transfer::channel::RouteKind,
};

const MAX_REMOTE_SESSION_LIFETIME_SECS: u64 = 15 * 60;
const MAX_ACTIVE_REMOTE_SESSIONS: usize = 64;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "state")]
pub enum RemoteSessionStatus {
    Invited,
    Pairing,
    Ready { route: RouteKind },
    Transferring { route: RouteKind },
    Completed,
    Cancelled,
    Expired,
    Failed,
}

impl RemoteSessionStatus {
    fn is_terminal(&self) -> bool {
        matches!(
            self,
            Self::Completed | Self::Cancelled | Self::Expired | Self::Failed
        )
    }

    fn expires_with_invitation(&self) -> bool {
        matches!(self, Self::Invited | Self::Pairing)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct RemoteSessionView {
    pub can_send: bool,
    pub session_id: String,
    pub peer_id: String,
    pub peer: Option<DeviceIdentity>,
    pub expires_at_unix: u64,
    pub status: RemoteSessionStatus,
}

pub struct RemoteSessionSecret([u8; 32]);

impl RemoteSessionSecret {
    pub fn new(secret: [u8; 32]) -> Self {
        Self(secret)
    }

    pub fn clear(&mut self) {
        self.0.zeroize();
    }

    #[cfg(test)]
    fn is_cleared(&self) -> bool {
        self.0.iter().all(|byte| *byte == 0)
    }
}

impl fmt::Debug for RemoteSessionSecret {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("RemoteSessionSecret([REDACTED])")
    }
}

impl Drop for RemoteSessionSecret {
    fn drop(&mut self) {
        self.clear();
    }
}

struct RemoteSession {
    view: RemoteSessionView,
    secret: Option<RemoteSessionSecret>,
    manual_code: Option<Zeroizing<String>>,
    share_link: Option<Zeroizing<String>>,
    role: InternetSessionRole,
    addresses: Vec<String>,
    established: Option<EstablishedInternetSession>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum RemoteSessionError {
    #[error("remote session data is invalid")]
    InvalidSession,
    #[error("remote session already exists")]
    DuplicateSession,
    #[error("remote session capacity has been reached")]
    CapacityReached,
    #[error("remote session was not found")]
    NotFound,
    #[error("remote session has expired")]
    Expired,
    #[error("remote session transition is invalid")]
    InvalidTransition,
}

#[derive(Default)]
pub struct RemoteSessionRegistry {
    sessions: Mutex<HashMap<String, RemoteSession>>,
    accept_loop_started: AtomicBool,
}

impl RemoteSessionRegistry {
    pub fn insert(
        &self,
        session_id: impl Into<String>,
        peer_id: impl Into<String>,
        expires_at_unix: u64,
        secret: RemoteSessionSecret,
        now_unix: u64,
    ) -> Result<(), RemoteSessionError> {
        self.insert_invitation(
            session_id,
            peer_id,
            Vec::new(),
            expires_at_unix,
            secret,
            InternetSessionRole::InvitationJoiner,
            now_unix,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn insert_invitation(
        &self,
        session_id: impl Into<String>,
        peer_id: impl Into<String>,
        addresses: Vec<String>,
        expires_at_unix: u64,
        secret: RemoteSessionSecret,
        role: InternetSessionRole,
        now_unix: u64,
    ) -> Result<(), RemoteSessionError> {
        let session_id = session_id.into();
        let peer_id = peer_id.into();
        if !is_safe_id(&session_id)
            || !is_safe_id(&peer_id)
            || addresses.len() > 16
            || addresses
                .iter()
                .any(|address| !is_safe_id(address) || address.len() > 512)
            || expires_at_unix <= now_unix
            || expires_at_unix - now_unix > MAX_REMOTE_SESSION_LIFETIME_SECS
        {
            return Err(RemoteSessionError::InvalidSession);
        }
        let mut sessions = self.sessions.lock().unwrap();
        sessions.retain(|_, session| {
            !session.view.status.expires_with_invitation()
                || now_unix < session.view.expires_at_unix
        });
        if sessions.contains_key(&session_id) {
            return Err(RemoteSessionError::DuplicateSession);
        }
        if sessions.len() >= MAX_ACTIVE_REMOTE_SESSIONS {
            return Err(RemoteSessionError::CapacityReached);
        }
        sessions.insert(
            session_id.clone(),
            RemoteSession {
                view: RemoteSessionView {
                    can_send: role == InternetSessionRole::InvitationJoiner,
                    session_id,
                    peer_id,
                    peer: None,
                    expires_at_unix,
                    status: RemoteSessionStatus::Invited,
                },
                secret: Some(secret),
                manual_code: None,
                share_link: None,
                role,
                addresses,
                established: None,
            },
        );
        Ok(())
    }

    pub fn begin_pairing(&self, session_id: &str, now_unix: u64) -> Result<(), RemoteSessionError> {
        let mut sessions = self.sessions.lock().unwrap();
        let session = sessions
            .get_mut(session_id)
            .ok_or(RemoteSessionError::NotFound)?;
        if now_unix >= session.view.expires_at_unix {
            sessions.remove(session_id);
            return Err(RemoteSessionError::Expired);
        }
        match session.view.status {
            RemoteSessionStatus::Invited => {
                session.view.status = RemoteSessionStatus::Pairing;
                Ok(())
            }
            RemoteSessionStatus::Pairing => Ok(()),
            _ => Err(RemoteSessionError::InvalidTransition),
        }
    }

    pub fn credentials(
        &self,
        session_id: &str,
        expected_role: InternetSessionRole,
        now_unix: u64,
    ) -> Result<InternetSessionCredentials, RemoteSessionError> {
        let mut sessions = self.sessions.lock().unwrap();
        let session = sessions
            .get(session_id)
            .ok_or(RemoteSessionError::NotFound)?;
        if now_unix >= session.view.expires_at_unix {
            sessions.remove(session_id);
            return Err(RemoteSessionError::Expired);
        }
        if session.role != expected_role {
            return Err(RemoteSessionError::InvalidTransition);
        }
        let manual_code = session
            .manual_code
            .as_ref()
            .map(|code| ManualPairingCode::parse(code.to_string()))
            .transpose()
            .map_err(|_| RemoteSessionError::InvalidSession)?;
        Ok(InternetSessionCredentials {
            session_id: session.view.session_id.clone(),
            advertised_endpoint_id: session.view.peer_id.clone(),
            addresses: session.addresses.clone(),
            expires_at_unix: session.view.expires_at_unix,
            role: session.role,
            secret: Zeroizing::new(
                session
                    .secret
                    .as_ref()
                    .ok_or(RemoteSessionError::InvalidSession)?
                    .0,
            ),
            manual_code,
        })
    }

    pub fn mark_ready(
        &self,
        session_id: &str,
        established: EstablishedInternetSession,
        now_unix: u64,
    ) -> Result<RemoteSessionView, RemoteSessionError> {
        let peer_id = established.peer_id().to_owned();
        let peer = established.peer_device().cloned();
        let route = established.route_kind();
        let mut sessions = self.sessions.lock().unwrap();
        let session = sessions
            .get_mut(session_id)
            .ok_or(RemoteSessionError::NotFound)?;
        if now_unix >= session.view.expires_at_unix {
            sessions.remove(session_id);
            return Err(RemoteSessionError::Expired);
        }
        if session.view.status != RemoteSessionStatus::Pairing {
            return Err(RemoteSessionError::InvalidTransition);
        }
        session.view.peer_id = peer_id;
        session.view.peer = peer;
        session.view.status = RemoteSessionStatus::Ready { route };
        session.secret = None;
        session.manual_code = None;
        session.share_link = None;
        session.addresses.clear();
        session.established = Some(established);
        Ok(session.view.clone())
    }

    pub fn established(
        &self,
        session_id: &str,
    ) -> Result<EstablishedInternetSession, RemoteSessionError> {
        self.sessions
            .lock()
            .unwrap()
            .get(session_id)
            .and_then(|session| session.established.clone())
            .ok_or(RemoteSessionError::NotFound)
    }

    pub fn start_accept_loop(&self) -> bool {
        self.accept_loop_started
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
    }

    pub fn transition(
        &self,
        session_id: &str,
        next: RemoteSessionStatus,
        now_unix: u64,
    ) -> Result<(), RemoteSessionError> {
        let mut sessions = self.sessions.lock().unwrap();
        let current = sessions
            .get(session_id)
            .ok_or(RemoteSessionError::NotFound)?;
        if current.view.status.expires_with_invitation() && now_unix >= current.view.expires_at_unix
        {
            sessions.remove(session_id);
            return Err(RemoteSessionError::Expired);
        }
        if !is_valid_transition(&current.view.status, &next) {
            return Err(RemoteSessionError::InvalidTransition);
        }
        if next.is_terminal() {
            sessions.remove(session_id);
        } else if let Some(session) = sessions.get_mut(session_id) {
            if matches!(next, RemoteSessionStatus::Ready { .. }) {
                session.share_link = None;
            }
            session.view.status = next;
        }
        Ok(())
    }

    pub fn cancel(&self, session_id: &str) -> bool {
        self.sessions
            .lock()
            .unwrap()
            .remove(session_id)
            .map(|session| {
                if let Some(established) = session.established {
                    established.close(crate::transfer::channel::ChannelCloseReason::Cancelled);
                }
            })
            .is_some()
    }

    pub fn expire(&self, now_unix: u64) -> usize {
        let mut sessions = self.sessions.lock().unwrap();
        let previous_len = sessions.len();
        sessions.retain(|_, session| {
            !session.view.status.expires_with_invitation()
                || now_unix < session.view.expires_at_unix
        });
        previous_len - sessions.len()
    }

    pub fn view(&self, session_id: &str) -> Option<RemoteSessionView> {
        self.sessions
            .lock()
            .unwrap()
            .get(session_id)
            .map(|session| session.view.clone())
    }

    pub fn with_secret<R>(
        &self,
        session_id: &str,
        derive: impl FnOnce(&[u8; 32]) -> R,
    ) -> Option<R> {
        self.sessions
            .lock()
            .unwrap()
            .get(session_id)
            .and_then(|session| session.secret.as_ref())
            .map(|secret| derive(&secret.0))
    }

    pub fn set_manual_code(
        &self,
        session_id: &str,
        code: String,
    ) -> Result<(), RemoteSessionError> {
        if code.len() != 8 || !code.bytes().all(|byte| byte.is_ascii_digit()) {
            return Err(RemoteSessionError::InvalidSession);
        }
        let mut sessions = self.sessions.lock().unwrap();
        let session = sessions
            .get_mut(session_id)
            .ok_or(RemoteSessionError::NotFound)?;
        session.manual_code = Some(Zeroizing::new(code));
        Ok(())
    }

    pub fn set_share_link(
        &self,
        session_id: &str,
        link: Zeroizing<String>,
    ) -> Result<(), RemoteSessionError> {
        let mut sessions = self.sessions.lock().unwrap();
        let session = sessions
            .get_mut(session_id)
            .ok_or(RemoteSessionError::NotFound)?;
        if session.role != InternetSessionRole::InvitationOwner
            || !session.view.status.expires_with_invitation()
            || !link.starts_with("dukto://connect#v1.")
            || link.len() > 8 * 1024
        {
            return Err(RemoteSessionError::InvalidSession);
        }
        session.share_link = Some(link);
        Ok(())
    }

    pub fn with_share_link<R>(
        &self,
        session_id: &str,
        now_unix: u64,
        use_link: impl FnOnce(&str) -> R,
    ) -> Result<R, RemoteSessionError> {
        let mut sessions = self.sessions.lock().unwrap();
        let session = sessions
            .get(session_id)
            .ok_or(RemoteSessionError::NotFound)?;
        if now_unix >= session.view.expires_at_unix {
            sessions.remove(session_id);
            return Err(RemoteSessionError::Expired);
        }
        if session.role != InternetSessionRole::InvitationOwner
            || !session.view.status.expires_with_invitation()
        {
            return Err(RemoteSessionError::InvalidTransition);
        }
        session
            .share_link
            .as_ref()
            .map(|link| use_link(link.as_str()))
            .ok_or(RemoteSessionError::NotFound)
    }

    pub fn with_manual_code<R>(
        &self,
        session_id: &str,
        use_code: impl FnOnce(&str) -> R,
    ) -> Option<R> {
        self.sessions
            .lock()
            .unwrap()
            .get(session_id)
            .and_then(|session| {
                session
                    .manual_code
                    .as_ref()
                    .map(|code| use_code(code.as_str()))
            })
    }
}

fn is_valid_transition(current: &RemoteSessionStatus, next: &RemoteSessionStatus) -> bool {
    matches!(
        (current, next),
        (RemoteSessionStatus::Invited, RemoteSessionStatus::Pairing)
            | (
                RemoteSessionStatus::Pairing,
                RemoteSessionStatus::Ready { .. }
            )
            | (
                RemoteSessionStatus::Ready { .. },
                RemoteSessionStatus::Transferring { .. } | RemoteSessionStatus::Completed
            )
            | (
                RemoteSessionStatus::Transferring { .. },
                RemoteSessionStatus::Ready { .. } | RemoteSessionStatus::Completed
            )
    ) || (!current.is_terminal()
        && matches!(
            next,
            RemoteSessionStatus::Cancelled
                | RemoteSessionStatus::Expired
                | RemoteSessionStatus::Failed
        ))
}

fn is_safe_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 256
        && value.trim() == value
        && value.chars().all(|character| {
            character.is_ascii() && !character.is_ascii_control() && !character.is_whitespace()
        })
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use zeroize::Zeroizing;

    use crate::internet::session::InternetSessionRole;
    use crate::transfer::channel::RouteKind;

    use super::{
        RemoteSessionError, RemoteSessionRegistry, RemoteSessionSecret, RemoteSessionStatus,
        MAX_ACTIVE_REMOTE_SESSIONS,
    };

    const NOW: u64 = 1_800_000_000;

    #[test]
    fn completed_transfer_returns_authenticated_session_to_ready_until_disconnect() {
        // Mutation captured: treating transfer completion as a terminal session state removes the peer.
        let registry = RemoteSessionRegistry::default();
        registry
            .insert(
                "session-1",
                "peer-1",
                NOW + 300,
                RemoteSessionSecret::new([0xA5; 32]),
                NOW,
            )
            .unwrap();
        assert_eq!(
            registry.with_secret("session-1", |secret| secret[0]),
            Some(0xA5)
        );

        registry
            .transition("session-1", RemoteSessionStatus::Pairing, NOW + 1)
            .unwrap();
        registry
            .transition(
                "session-1",
                RemoteSessionStatus::Ready {
                    route: RouteKind::Direct,
                },
                NOW + 2,
            )
            .unwrap();
        assert_eq!(
            registry.view("session-1").unwrap().status,
            RemoteSessionStatus::Ready {
                route: RouteKind::Direct
            }
        );

        registry
            .transition(
                "session-1",
                RemoteSessionStatus::Transferring {
                    route: RouteKind::Direct,
                },
                NOW + 3,
            )
            .unwrap();
        registry
            .transition(
                "session-1",
                RemoteSessionStatus::Ready {
                    route: RouteKind::Direct,
                },
                NOW + 4,
            )
            .unwrap();
        assert_eq!(
            registry.view("session-1").unwrap().status,
            RemoteSessionStatus::Ready {
                route: RouteKind::Direct
            }
        );
        assert!(registry.cancel("session-1"));
        assert!(registry.view("session-1").is_none());
        assert!(registry
            .with_secret("session-1", |secret| secret[0])
            .is_none());
    }

    #[test]
    fn cancellation_expiration_and_invalid_transitions_are_deterministic() {
        let registry = RemoteSessionRegistry::default();
        registry
            .insert(
                "cancelled",
                "peer",
                NOW + 30,
                RemoteSessionSecret::new([1; 32]),
                NOW,
            )
            .unwrap();
        assert!(registry.cancel("cancelled"));
        assert!(!registry.cancel("cancelled"));

        registry
            .insert(
                "expired",
                "peer",
                NOW + 1,
                RemoteSessionSecret::new([2; 32]),
                NOW,
            )
            .unwrap();
        assert_eq!(registry.expire(NOW + 1), 1);
        assert!(registry.view("expired").is_none());

        registry
            .insert(
                "invalid",
                "peer",
                NOW + 30,
                RemoteSessionSecret::new([3; 32]),
                NOW,
            )
            .unwrap();
        assert_eq!(
            registry.transition("invalid", RemoteSessionStatus::Completed, NOW + 1),
            Err(RemoteSessionError::InvalidTransition)
        );
    }

    #[test]
    fn authenticated_sessions_remain_ready_after_the_invitation_and_transfer_finish() {
        // Mutation captured: removing ready sessions at the invitation deadline or after one transfer loses the peer.
        let registry = RemoteSessionRegistry::default();
        registry
            .insert(
                "ready",
                "peer",
                NOW + 2,
                RemoteSessionSecret::new([4; 32]),
                NOW,
            )
            .unwrap();
        registry
            .transition("ready", RemoteSessionStatus::Pairing, NOW + 1)
            .unwrap();
        registry
            .transition(
                "ready",
                RemoteSessionStatus::Ready {
                    route: RouteKind::Direct,
                },
                NOW + 1,
            )
            .unwrap();

        assert_eq!(registry.expire(NOW + 3), 0);
        assert!(registry.view("ready").is_some());
        registry
            .transition(
                "ready",
                RemoteSessionStatus::Transferring {
                    route: RouteKind::Direct,
                },
                NOW + 3,
            )
            .unwrap();
        registry
            .transition(
                "ready",
                RemoteSessionStatus::Ready {
                    route: RouteKind::Direct,
                },
                NOW + 4,
            )
            .unwrap();
        assert_eq!(
            registry.view("ready").unwrap().status,
            RemoteSessionStatus::Ready {
                route: RouteKind::Direct
            }
        );
    }

    #[test]
    fn admission_prunes_expired_sessions_before_enforcing_capacity() {
        let registry = RemoteSessionRegistry::default();
        for index in 0..MAX_ACTIVE_REMOTE_SESSIONS {
            registry
                .insert(
                    format!("expired-{index}"),
                    "peer",
                    NOW + 1,
                    RemoteSessionSecret::new([index as u8; 32]),
                    NOW,
                )
                .unwrap();
        }

        registry
            .insert(
                "fresh",
                "peer",
                NOW + 60,
                RemoteSessionSecret::new([0xA5; 32]),
                NOW + 1,
            )
            .unwrap();

        assert!(registry.view("expired-0").is_none());
        assert!(registry.view("fresh").is_some());
    }

    #[test]
    fn admission_is_bounded_without_evicting_live_or_duplicate_sessions() {
        let registry = RemoteSessionRegistry::default();
        for index in 0..MAX_ACTIVE_REMOTE_SESSIONS {
            registry
                .insert(
                    format!("live-{index}"),
                    "peer",
                    NOW + 60,
                    RemoteSessionSecret::new([index as u8; 32]),
                    NOW,
                )
                .unwrap();
        }

        assert_eq!(
            registry.insert(
                "overflow",
                "peer",
                NOW + 60,
                RemoteSessionSecret::new([0xFE; 32]),
                NOW,
            ),
            Err(RemoteSessionError::CapacityReached)
        );
        assert_eq!(
            registry.insert(
                "live-0",
                "peer",
                NOW + 60,
                RemoteSessionSecret::new([0xFF; 32]),
                NOW,
            ),
            Err(RemoteSessionError::DuplicateSession)
        );
        assert_eq!(registry.with_secret("live-0", |secret| secret[0]), Some(0));
        assert!(registry.view("overflow").is_none());
    }

    #[test]
    fn concurrent_admission_never_exceeds_capacity() {
        let registry = Arc::new(RemoteSessionRegistry::default());
        let attempts = MAX_ACTIVE_REMOTE_SESSIONS * 2;
        let handles = (0..attempts)
            .map(|index| {
                let registry = Arc::clone(&registry);
                std::thread::spawn(move || {
                    registry.insert(
                        format!("concurrent-{index}"),
                        "peer",
                        NOW + 60,
                        RemoteSessionSecret::new([index as u8; 32]),
                        NOW,
                    )
                })
            })
            .collect::<Vec<_>>();

        let results = handles
            .into_iter()
            .map(|handle| handle.join().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(
            results.iter().filter(|result| result.is_ok()).count(),
            MAX_ACTIVE_REMOTE_SESSIONS
        );
        assert_eq!(
            results
                .iter()
                .filter(|result| **result == Err(RemoteSessionError::CapacityReached))
                .count(),
            MAX_ACTIVE_REMOTE_SESSIONS
        );
    }

    #[test]
    fn secret_clear_and_debug_are_redacted() {
        let mut secret = RemoteSessionSecret::new([0xAB; 32]);
        assert!(!format!("{secret:?}").contains("ab"));
        secret.clear();
        assert!(secret.is_cleared());
    }

    #[test]
    fn invitation_share_link_is_owner_only_expiring_and_pairing_scoped() {
        // Mutation captured: dropping the InvitationOwner check lets a joiner expose a bearer link.
        let owner = RemoteSessionRegistry::default();
        owner
            .insert_invitation(
                "owner-session",
                "owner-peer",
                Vec::new(),
                NOW + 30,
                RemoteSessionSecret::new([0xAB; 32]),
                InternetSessionRole::InvitationOwner,
                NOW,
            )
            .unwrap();
        owner
            .set_share_link(
                "owner-session",
                Zeroizing::new("dukto://connect#v1.owner".to_owned()),
            )
            .unwrap();
        assert_eq!(
            owner.with_share_link("owner-session", NOW + 1, str::to_owned),
            Ok("dukto://connect#v1.owner".to_owned())
        );
        owner
            .transition("owner-session", RemoteSessionStatus::Pairing, NOW + 2)
            .unwrap();
        assert!(owner
            .with_share_link("owner-session", NOW + 2, str::to_owned)
            .is_ok());
        owner
            .transition(
                "owner-session",
                RemoteSessionStatus::Ready {
                    route: RouteKind::Direct,
                },
                NOW + 3,
            )
            .unwrap();
        assert_eq!(
            owner.with_share_link("owner-session", NOW + 3, str::to_owned),
            Err(RemoteSessionError::InvalidTransition)
        );

        let joiner = RemoteSessionRegistry::default();
        joiner
            .insert(
                "joiner-session",
                "owner-peer",
                NOW + 30,
                RemoteSessionSecret::new([0xCD; 32]),
                NOW,
            )
            .unwrap();
        assert_eq!(
            joiner.set_share_link(
                "joiner-session",
                Zeroizing::new("dukto://connect#v1.joiner".to_owned()),
            ),
            Err(RemoteSessionError::InvalidSession)
        );

        let expired = RemoteSessionRegistry::default();
        expired
            .insert_invitation(
                "expired-session",
                "owner-peer",
                Vec::new(),
                NOW + 1,
                RemoteSessionSecret::new([0xEF; 32]),
                InternetSessionRole::InvitationOwner,
                NOW,
            )
            .unwrap();
        expired
            .set_share_link(
                "expired-session",
                Zeroizing::new("dukto://connect#v1.expired".to_owned()),
            )
            .unwrap();
        assert_eq!(
            expired.with_share_link("expired-session", NOW + 1, str::to_owned),
            Err(RemoteSessionError::Expired)
        );
        assert!(expired.view("expired-session").is_none());
    }
}

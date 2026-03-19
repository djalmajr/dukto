use serde::{Deserialize, Serialize};

/// Status of a transfer session.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum SessionStatus {
    Connecting,
    Handshaking,
    Ready,
    Transferring,
    Completed,
    Failed(String),
}

/// Represents a transfer session between two peers.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionInfo {
    pub session_id: String,
    pub peer_device_id: String,
    pub status: SessionStatus,
}

impl SessionInfo {
    pub fn new(peer_device_id: String) -> Self {
        Self {
            session_id: uuid::Uuid::new_v4().to_string(),
            peer_device_id,
            status: SessionStatus::Connecting,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_info_starts_as_connecting() {
        let info = SessionInfo::new("peer-123".into());
        assert_eq!(info.status, SessionStatus::Connecting);
        assert_eq!(info.peer_device_id, "peer-123");
        assert!(!info.session_id.is_empty());
    }

    #[test]
    fn session_id_is_unique() {
        let a = SessionInfo::new("peer".into());
        let b = SessionInfo::new("peer".into());
        assert_ne!(a.session_id, b.session_id);
    }
}

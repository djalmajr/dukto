use serde::{Deserialize, Serialize};

use crate::state::device::DeviceIdentity;

/// Application close code and marker for a deliberate transfer cancellation.
pub const TRANSFER_CANCEL_CLOSE_CODE: u32 = 0xD0170;
pub const TRANSFER_CANCEL_CLOSE_REASON: &[u8] = b"dukto:transfer-cancelled:v1";

/// Packet type identifiers for the transfer protocol.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum PacketType {
    TransferHeader = 0,
    ItemMetadata = 1,
    Binary = 2,
    Success = 3,
    TransferError = 4,
    AcceptReject = 5,
}

impl TryFrom<u8> for PacketType {
    type Error = String;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::TransferHeader),
            1 => Ok(Self::ItemMetadata),
            2 => Ok(Self::Binary),
            3 => Ok(Self::Success),
            4 => Ok(Self::TransferError),
            5 => Ok(Self::AcceptReject),
            _ => Err(format!("Unknown packet type: {}", value)),
        }
    }
}

/// Header sent at the start of a transfer session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransferHeader {
    pub transfer_id: String,
    pub sender_device_id: String,
    /// Full sender identity for direct-address transfers that bypass mDNS.
    /// Optional so receivers can continue accepting headers from older 0.2 peers.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sender: Option<DeviceIdentity>,
    pub item_count: u32,
    pub total_size: u64,
}

#[cfg(test)]
mod tests {
    use super::TransferHeader;

    #[test]
    fn older_transfer_headers_without_sender_identity_remain_compatible() {
        let header: TransferHeader = serde_json::from_str(
            r#"{"transfer_id":"legacy","sender_device_id":"peer","item_count":1,"total_size":4}"#,
        )
        .unwrap();

        assert_eq!(header.sender_device_id, "peer");
        assert!(header.sender.is_none());
    }
}

/// Receiver acknowledgement, emitted only after every advertised item is written.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransferReceipt {
    pub transfer_id: String,
    pub items_received: u32,
    pub bytes_received: u64,
}

/// Metadata for a single item in the transfer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ItemMetadata {
    pub item_id: String,
    pub kind: ItemKind,
    pub name: String,
    pub relative_path: Option<String>,
    pub size_bytes: u64,
    pub modified_at: Option<u64>,
}

/// Type of item being transferred.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ItemKind {
    File,
    Directory,
}

/// Accept or reject response from the receiver.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AcceptRejectResponse {
    pub accepted: bool,
    pub destination_dir: Option<String>,
}

/// Progress information emitted during transfer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransferProgress {
    pub transfer_id: String,
    pub bytes_sent: u64,
    pub bytes_total: u64,
    pub speed_bps: u64,
    pub percent: f64,
}

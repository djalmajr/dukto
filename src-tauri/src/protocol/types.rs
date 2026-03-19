use serde::{Deserialize, Serialize};

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
    pub item_count: u32,
    pub total_size: u64,
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

use super::types::PacketType;

/// Encode a packet: [4 bytes size (LE)][1 byte type][payload]
pub fn encode_packet(packet_type: PacketType, payload: &[u8]) -> Vec<u8> {
    let total_len = 1 + payload.len(); // type byte + payload
    let mut buf = Vec::with_capacity(4 + total_len);
    buf.extend_from_slice(&(total_len as u32).to_le_bytes());
    buf.push(packet_type as u8);
    buf.extend_from_slice(payload);
    buf
}

/// Decode a packet header from a raw buffer.
/// Returns (packet_type, payload_slice).
pub fn decode_packet_header(data: &[u8]) -> Result<(PacketType, &[u8]), String> {
    if data.is_empty() {
        return Err("Empty packet".into());
    }
    let packet_type = PacketType::try_from(data[0])?;
    Ok((packet_type, &data[1..]))
}

/// Read the 4-byte length prefix from a buffer.
pub fn read_length_prefix(buf: &[u8; 4]) -> u32 {
    u32::from_le_bytes(*buf)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_and_decode_roundtrip() {
        let payload = b"hello world";
        let encoded = encode_packet(PacketType::Binary, payload);

        // First 4 bytes = length (type byte + payload)
        let len = read_length_prefix(&encoded[..4].try_into().unwrap());
        assert_eq!(len as usize, 1 + payload.len());

        // Rest = type + payload
        let (ptype, decoded_payload) = decode_packet_header(&encoded[4..]).unwrap();
        assert_eq!(ptype, PacketType::Binary);
        assert_eq!(decoded_payload, payload);
    }

    #[test]
    fn encode_json_packet() {
        let header = serde_json::json!({
            "transfer_id": "abc",
            "item_count": 1,
            "total_size": 1024
        });
        let json_bytes = serde_json::to_vec(&header).unwrap();
        let encoded = encode_packet(PacketType::TransferHeader, &json_bytes);

        let len = read_length_prefix(&encoded[..4].try_into().unwrap());
        assert_eq!(len as usize, 1 + json_bytes.len());

        let (ptype, payload) = decode_packet_header(&encoded[4..]).unwrap();
        assert_eq!(ptype, PacketType::TransferHeader);

        let decoded: serde_json::Value = serde_json::from_slice(payload).unwrap();
        assert_eq!(decoded["transfer_id"], "abc");
    }

    #[test]
    fn decode_empty_packet_fails() {
        let result = decode_packet_header(&[]);
        assert!(result.is_err());
    }

    #[test]
    fn decode_unknown_type_fails() {
        let result = decode_packet_header(&[255, 0, 1, 2]);
        assert!(result.is_err());
    }
}

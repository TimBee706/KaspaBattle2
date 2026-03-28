//! Transaction Generator — builds Kaspa TXs with bit-pattern mining.
//!
//! Inlined from michaelsutton/kdapp (kdapp/src/generator.rs).
//! Builds transactions with specially formatted payloads and mines a TX-ID
//! that matches a predefined bit-pattern for efficient network filtering.

use kaspa_hashes::Hash;

use crate::kdapp_episode::Episode;
use crate::kdapp_engine::EpisodeMessage;

pub type PatternType = [(u8, u8); 10];
pub type PrefixType = u32;

/// Check if a transaction ID matches the expected bit-pattern.
pub fn check_pattern(tx_id: Hash, pattern: &PatternType) -> bool {
    let words = tx_id.as_bytes();
    for (pos, val) in pattern.iter().copied() {
        let word = words[pos as usize / 8];
        if ((word >> (pos % 8)) & 1) != val {
            return false;
        }
    }
    true
}

/// Payload header utilities.
pub struct Payload;

impl Payload {
    /// Pack inner data with a 4-byte prefix and 4-byte nonce header.
    pub fn pack_header(inner_data: Vec<u8>, prefix: PrefixType) -> Vec<u8> {
        // 4 byte prefix | 4 byte nonce | inner data
        prefix
            .to_le_bytes()
            .into_iter()
            .chain(0u32.to_le_bytes())
            .chain(inner_data)
            .collect()
    }

    /// Check if a payload starts with the expected prefix.
    pub fn check_header(payload: &[u8], prefix: PrefixType) -> bool {
        if payload.len() < 8 {
            return false;
        }
        payload[0..4] == prefix.to_le_bytes()
    }

    /// Set the 4-byte nonce in the payload header.
    pub fn set_nonce(data: &mut [u8], nonce: u32) {
        data[4..8].copy_from_slice(&nonce.to_le_bytes());
    }

    /// Strip the 8-byte header (prefix + nonce) from a payload.
    /// Assumes `check_header` was called and returned true.
    pub fn strip_header(mut payload: Vec<u8>) -> Vec<u8> {
        payload.drain(0..8);
        payload
    }
}

/// Serializes a command into the payload format for a TX.
///
/// NOTE: Full TransactionGenerator (with pattern-mining) requires
/// kaspa_consensus_core::sign and MutableTransaction which may differ
/// between kaspa crate versions. For now, this module provides the
/// payload helpers. The actual TX building can use the existing
/// battle-kaspa infrastructure.
pub fn serialize_command<G: Episode>(cmd: &EpisodeMessage<G>, prefix: PrefixType) -> Vec<u8> {
    let inner = borsh::to_vec(cmd).expect("command serialization failed");
    Payload::pack_header(inner, prefix)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_payload_header_roundtrip() {
        let prefix: PrefixType = 0x4B425432;
        let inner = vec![1, 2, 3, 4, 5];
        let packed = Payload::pack_header(inner.clone(), prefix);

        assert!(Payload::check_header(&packed, prefix));
        assert!(!Payload::check_header(&packed, 0x00000000));

        let stripped = Payload::strip_header(packed);
        assert_eq!(stripped, inner);
    }

    #[test]
    fn test_payload_nonce() {
        let prefix: PrefixType = 0x4B425432;
        let inner = vec![10, 20, 30];
        let mut packed = Payload::pack_header(inner, prefix);

        Payload::set_nonce(&mut packed, 42);
        assert_eq!(&packed[4..8], &42u32.to_le_bytes());

        // Prefix should be unchanged
        assert!(Payload::check_header(&packed, prefix));
    }

    #[test]
    fn test_check_header_too_short() {
        assert!(!Payload::check_header(&[1, 2, 3], 0x12345678));
    }
}

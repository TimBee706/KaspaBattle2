//! Proxy configuration — Pattern, Prefix constants.

use crate::kdapp_generator::{PatternType, PrefixType};

/// KaspaBattle-specific bit-pattern for TX-ID filtering.
/// 10-bit pattern → ~1:1024 collision rate.
pub const BATTLE_PATTERN: PatternType = [
    (0, 1), (1, 0), (2, 1), (3, 1),
    (4, 0), (5, 1), (6, 0), (7, 1),
    (8, 1), (9, 0),
];

/// 4-byte payload prefix: "KBT2" = 0x4B425432
pub const BATTLE_PREFIX: PrefixType = 0x4B42_5432;

/// Default network for development/testing.
pub const DEFAULT_NETWORK: &str = "testnet-10";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pattern_has_10_bits() {
        assert_eq!(BATTLE_PATTERN.len(), 10);
    }

    #[test]
    fn test_pattern_bit_positions_valid() {
        for (pos, val) in &BATTLE_PATTERN {
            assert!(*pos < 128);
            assert!(*val <= 1);
        }
    }

    #[test]
    fn test_prefix_is_nonzero() {
        assert_ne!(BATTLE_PREFIX, 0);
    }
}

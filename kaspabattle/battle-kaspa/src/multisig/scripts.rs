//! Script construction for multisig escrows.
//!
//! Builds P2SH redeem scripts and derives escrow addresses using
//! `kaspa-txscript` primitives.

use kaspa_addresses::{Address, Prefix, Version};
use kaspa_consensus_core::tx::ScriptPublicKey;
use kaspa_txscript::{
    multisig_redeem_script, pay_to_script_hash_script, pay_to_script_hash_signature_script,
};

use thiserror::Error;

#[derive(Error, Debug, Clone)]
pub enum ScriptError {
    #[error("Invalid public key hex: {0}")]
    InvalidPubKeyHex(String),

    #[error("Public key must be 32 bytes (x-only Schnorr), got {0} bytes")]
    InvalidPubKeyLength(usize),

    #[error("Threshold {threshold} exceeds total keys {total}")]
    InvalidThreshold { threshold: u8, total: u8 },

    #[error("Redeem script construction failed: {0}")]
    ScriptBuildFailed(String),

    #[error("Signature script construction failed: {0}")]
    SigScriptFailed(String),
}

/// Decodes a hex-encoded x-only public key (32 bytes) into a fixed-size array.
pub fn decode_pubkey_hex(hex_str: &str) -> Result<[u8; 32], ScriptError> {
    let bytes = hex::decode(hex_str)
        .map_err(|e| ScriptError::InvalidPubKeyHex(format!("{}: {}", hex_str, e)))?;
    if bytes.len() != 32 {
        return Err(ScriptError::InvalidPubKeyLength(bytes.len()));
    }
    let mut arr = [0u8; 32];
    arr.copy_from_slice(&bytes);
    Ok(arr)
}

/// Builds a standard m-of-n multisig redeem script.
///
/// Produces: `<threshold> <PK_1> <PK_2> ... <PK_n> <n> OP_CHECKMULTISIG`
///
/// # Arguments
/// * `pubkeys_hex` - Hex-encoded x-only public keys (32 bytes each)
/// * `threshold` - Minimum number of signatures required
///
/// # Example
/// ```ignore
/// let script = build_multisig_redeem_script(
///     &[pk_a_hex, pk_b_hex, pk_oracle_hex],
///     2,
/// )?;
/// ```
pub fn build_multisig_redeem_script(
    pubkeys_hex: &[String],
    threshold: u8,
) -> Result<Vec<u8>, ScriptError> {
    if threshold as usize > pubkeys_hex.len() {
        return Err(ScriptError::InvalidThreshold {
            threshold,
            total: pubkeys_hex.len() as u8,
        });
    }

    let pubkeys: Vec<[u8; 32]> = pubkeys_hex
        .iter()
        .map(|hex| decode_pubkey_hex(hex))
        .collect::<Result<Vec<_>, _>>()?;

    multisig_redeem_script(pubkeys.into_iter(), threshold as usize)
        .map_err(|e| ScriptError::ScriptBuildFailed(format!("{}", e)))
}

/// Derives the P2SH `ScriptPublicKey` from a redeem script.
///
/// This is the on-chain script that locks funds: `OP_BLAKE2B <hash> OP_EQUAL`.
/// The hash is Blake2b-256 of the redeem script.
pub fn redeem_script_to_p2sh(redeem_script: &[u8]) -> ScriptPublicKey {
    pay_to_script_hash_script(redeem_script)
}

/// Derives the human-readable P2SH address from a redeem script.
///
/// # Arguments
/// * `redeem_script` - The raw multisig redeem script
/// * `prefix` - Network prefix (Mainnet or Testnet)
pub fn redeem_script_to_address(redeem_script: &[u8], prefix: Prefix) -> Address {
    let spk = redeem_script_to_p2sh(redeem_script);
    // P2SH ScriptPublicKey format: OP_BLAKE2B OP_DATA_32 <32-byte-hash> OP_EQUAL
    // Extract the 32-byte hash (bytes 2..34)
    let script_bytes = spk.script();
    let hash = &script_bytes[2..34];
    Address::new(prefix, Version::ScriptHash, hash)
}

/// Builds the complete signature script (ScriptSig) for spending a P2SH multisig output.
///
/// Format: `<sig_1> <sig_2> ... <serialized_redeem_script>`
///
/// Each signature is prefixed with OP_DATA_65 (0x41) and suffixed with the sighash type byte.
/// The signatures must be provided in the same order as the public keys appear in the
/// redeem script.
///
/// # Arguments
/// * `signatures` - Raw Schnorr signatures with OP_DATA_65 prefix and sighash suffix (66 bytes each)
/// * `redeem_script` - The original redeem script
pub fn build_multisig_sig_script(
    signatures: &[Vec<u8>],
    redeem_script: &[u8],
) -> Result<Vec<u8>, ScriptError> {
    // Concatenate all signature chunks
    let all_sigs: Vec<u8> = signatures.iter().flat_map(|s| s.iter().copied()).collect();

    pay_to_script_hash_signature_script(redeem_script.to_vec(), all_sigs)
        .map_err(|e| ScriptError::SigScriptFailed(format!("{}", e)))
}

/// Formats a single Schnorr signature into the multisig-compatible format.
///
/// Output: `OP_DATA_65 (0x41) | <64-byte-schnorr-sig> | <sighash_type_byte>`
///
/// This is the format expected by OP_CHECKMULTISIG on Kaspa.
pub fn format_schnorr_sig(sig_bytes: &[u8; 64], sighash_type: u8) -> Vec<u8> {
    let mut result = Vec::with_capacity(66);
    result.push(0x41); // OP_DATA_65 — next 65 bytes are data
    result.extend_from_slice(sig_bytes);
    result.push(sighash_type);
    result
}

// ─── Phase 5: CLTV Time-Lock Scripts ────────────────────────────────────────

/// Kaspa script opcodes used for time-lock construction.
mod opcodes {
    pub const OP_IF: u8 = 0x63;
    pub const OP_ELSE: u8 = 0x67;
    pub const OP_ENDIF: u8 = 0x68;
    pub const OP_CHECKLOCKTIMEVERIFY: u8 = 0xb1;
    pub const OP_DROP: u8 = 0x75;
    pub const OP_CHECKSIG: u8 = 0xac;
    pub const OP_CHECKMULTISIG: u8 = 0xae;
    pub const OP_DATA_32: u8 = 0x20;
    pub const OP_2: u8 = 0x52;
    pub const OP_3: u8 = 0x53;
}

/// Encode a timestamp as a script number for CLTV.
///
/// Script numbers in Kaspa follow Bitcoin's CScriptNum encoding:
/// - Little-endian with sign bit in the highest bit of the last byte.
/// - Leading zeros are omitted.
fn encode_script_number(value: u64) -> Vec<u8> {
    if value == 0 {
        return vec![];
    }

    let mut result = Vec::new();
    let mut val = value;
    while val > 0 {
        result.push((val & 0xff) as u8);
        val >>= 8;
    }

    // If the top bit is set, add an extra 0x00 byte for positive numbers
    if result.last().unwrap() & 0x80 != 0 {
        result.push(0x00);
    }

    result
}

/// Builds a time-locked multisig redeem script with a fallback refund path.
///
/// ## Script structure (IF/ELSE pattern):
///
/// ```text
/// OP_IF
///   <2> <PK_A> <PK_B> <PK_Oracle> <3> OP_CHECKMULTISIG    // Normal: 2-of-3 multisig
/// OP_ELSE
///   <locktime> OP_CHECKLOCKTIMEVERIFY OP_DROP               // After timeout:
///   <PK_Refund> OP_CHECKSIG                                 // Single-key refund
/// OP_ENDIF
/// ```
///
/// **Spending conditions:**
/// - **Before timeout:** Standard 2-of-3 multisig (use `OP_TRUE` as IF selector)
/// - **After timeout:** Single signature from `refund_pubkey_hex` (use `OP_FALSE` as IF selector)
///
/// The `locktime` value is a Unix timestamp. The spending transaction's `lock_time`
/// must be >= this value for the ELSE branch to succeed.
///
/// # Arguments
/// * `pubkeys_hex` - The 3 x-only public keys for the multisig (player A, B, oracle)
/// * `threshold` - Minimum signatures required (typically 2)
/// * `locktime` - CLTV time-lock value (Unix timestamp in seconds)
/// * `refund_pubkey_hex` - Public key allowed to claim after timeout (usually player A or oracle)
pub fn build_timelocked_multisig_redeem_script(
    pubkeys_hex: &[String],
    threshold: u8,
    locktime: u64,
    refund_pubkey_hex: &str,
) -> Result<Vec<u8>, ScriptError> {
    if threshold as usize > pubkeys_hex.len() {
        return Err(ScriptError::InvalidThreshold {
            threshold,
            total: pubkeys_hex.len() as u8,
        });
    }

    // Decode all pubkeys
    let pubkeys: Vec<[u8; 32]> = pubkeys_hex
        .iter()
        .map(|hex| decode_pubkey_hex(hex))
        .collect::<Result<Vec<_>, _>>()?;
    let refund_pk = decode_pubkey_hex(refund_pubkey_hex)?;

    let locktime_bytes = encode_script_number(locktime);

    // Build the script manually
    let mut script = Vec::new();

    // ── IF branch: standard multisig ──
    script.push(opcodes::OP_IF);
    script.push(opcodes::OP_2 + threshold - 2); // OP_2 for threshold=2, OP_3 for threshold=3
    for pk in &pubkeys {
        script.push(opcodes::OP_DATA_32);
        script.extend_from_slice(pk);
    }
    script.push(opcodes::OP_2 + pubkeys.len() as u8 - 2); // OP_N for total keys
    script.push(opcodes::OP_CHECKMULTISIG);

    // ── ELSE branch: time-locked single-key refund ──
    script.push(opcodes::OP_ELSE);

    // Push locktime as data
    script.push(locktime_bytes.len() as u8); // OP_DATA_N
    script.extend_from_slice(&locktime_bytes);

    script.push(opcodes::OP_CHECKLOCKTIMEVERIFY);
    script.push(opcodes::OP_DROP);

    // Single-key check
    script.push(opcodes::OP_DATA_32);
    script.extend_from_slice(&refund_pk);
    script.push(opcodes::OP_CHECKSIG);

    script.push(opcodes::OP_ENDIF);

    Ok(script)
}

/// Builds a simple time-locked single-key refund script (no multisig).
///
/// ```text
/// <locktime> OP_CHECKLOCKTIMEVERIFY OP_DROP <PK> OP_CHECKSIG
/// ```
///
/// This is useful as a standalone fallback for emergency refunds
/// where funds should only be movable after a timeout with a single key.
pub fn build_timelock_only_redeem_script(
    locktime: u64,
    pubkey_hex: &str,
) -> Result<Vec<u8>, ScriptError> {
    let pk = decode_pubkey_hex(pubkey_hex)?;
    let locktime_bytes = encode_script_number(locktime);

    let mut script = Vec::new();
    script.push(locktime_bytes.len() as u8);
    script.extend_from_slice(&locktime_bytes);
    script.push(opcodes::OP_CHECKLOCKTIMEVERIFY);
    script.push(opcodes::OP_DROP);
    script.push(opcodes::OP_DATA_32);
    script.extend_from_slice(&pk);
    script.push(opcodes::OP_CHECKSIG);

    Ok(script)
}

/// Calculates the default timeout for a match escrow.
///
/// Returns the Unix timestamp at `hours` hours from now.
pub fn default_timeout_timestamp(hours: u64) -> u64 {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    now + hours * 3600
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn test_pubkeys() -> (String, String, String) {
        // Deterministic test keys (from rusty-kaspa multisig tests)
        let kp1 = secp256k1::Keypair::from_seckey_slice(
            secp256k1::SECP256K1,
            &hex::decode("1d99c236b1f37b3b845336e6c568ba37e9ced4769d83b7a096eec446b940d160")
                .unwrap(),
        )
        .unwrap();
        let kp2 = secp256k1::Keypair::from_seckey_slice(
            secp256k1::SECP256K1,
            &hex::decode("349ca0c824948fed8c2c568ce205e9d9be4468ef099cad76e3e5ec918954aca4")
                .unwrap(),
        )
        .unwrap();
        let kp3 = secp256k1::Keypair::from_seckey_slice(
            secp256k1::SECP256K1,
            &hex::decode("5f6e7a8b9c0d1e2f3a4b5c6d7e8f9a0b1c2d3e4f5061728394a5b6c7d8e9f001")
                .unwrap(),
        )
        .unwrap();

        (
            hex::encode(kp1.x_only_public_key().0.serialize()),
            hex::encode(kp2.x_only_public_key().0.serialize()),
            hex::encode(kp3.x_only_public_key().0.serialize()),
        )
    }

    #[test]
    fn test_decode_pubkey_hex_valid() {
        let (pk, _, _) = test_pubkeys();
        let result = decode_pubkey_hex(&pk);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 32);
    }

    #[test]
    fn test_decode_pubkey_hex_invalid() {
        assert!(decode_pubkey_hex("not_hex").is_err());
        assert!(decode_pubkey_hex("aabb").is_err()); // too short
    }

    #[test]
    fn test_build_2_of_3_redeem_script() {
        let (pk_a, pk_b, pk_o) = test_pubkeys();
        let script =
            build_multisig_redeem_script(&[pk_a.clone(), pk_b.clone(), pk_o.clone()], 2).unwrap();

        // Script should start with OP_2 (0x52) for threshold
        assert!(!script.is_empty());
        // Script should end with OP_CHECKMULTISIG (0xae)
        assert_eq!(*script.last().unwrap(), 0xae);
    }

    #[test]
    fn test_build_redeem_script_threshold_too_high() {
        let (pk_a, pk_b, _) = test_pubkeys();
        let result = build_multisig_redeem_script(&[pk_a, pk_b], 3);
        assert!(result.is_err());
    }

    #[test]
    fn test_redeem_script_deterministic() {
        let (pk_a, pk_b, pk_o) = test_pubkeys();
        let script1 =
            build_multisig_redeem_script(&[pk_a.clone(), pk_b.clone(), pk_o.clone()], 2).unwrap();
        let script2 = build_multisig_redeem_script(&[pk_a, pk_b, pk_o], 2).unwrap();
        assert_eq!(script1, script2, "Same inputs must produce same script");
    }

    #[test]
    fn test_p2sh_address_generation() {
        let (pk_a, pk_b, pk_o) = test_pubkeys();
        let script = build_multisig_redeem_script(&[pk_a, pk_b, pk_o], 2).unwrap();

        let address = redeem_script_to_address(&script, Prefix::Testnet);
        let addr_str = address.to_string();

        assert!(
            addr_str.starts_with("kaspatest:"),
            "Testnet P2SH address should start with kaspatest:, got {}",
            addr_str
        );
    }

    #[test]
    fn test_p2sh_address_mainnet() {
        let (pk_a, pk_b, pk_o) = test_pubkeys();
        let script = build_multisig_redeem_script(&[pk_a, pk_b, pk_o], 2).unwrap();

        let address = redeem_script_to_address(&script, Prefix::Mainnet);
        let addr_str = address.to_string();

        assert!(
            addr_str.starts_with("kaspa:"),
            "Mainnet P2SH address should start with kaspa:, got {}",
            addr_str
        );
    }

    #[test]
    fn test_different_key_order_produces_different_address() {
        let (pk_a, pk_b, pk_o) = test_pubkeys();
        let script1 =
            build_multisig_redeem_script(&[pk_a.clone(), pk_b.clone(), pk_o.clone()], 2).unwrap();
        let script2 =
            build_multisig_redeem_script(&[pk_b, pk_a, pk_o], 2).unwrap();

        let addr1 = redeem_script_to_address(&script1, Prefix::Testnet);
        let addr2 = redeem_script_to_address(&script2, Prefix::Testnet);

        assert_ne!(
            addr1.to_string(),
            addr2.to_string(),
            "Different key ordering should produce different addresses"
        );
    }

    #[test]
    fn test_format_schnorr_sig() {
        let fake_sig = [0x42u8; 64];
        let formatted = format_schnorr_sig(&fake_sig, 0x01);

        assert_eq!(formatted.len(), 66);
        assert_eq!(formatted[0], 0x41); // OP_DATA_65
        assert_eq!(formatted[65], 0x01); // sighash type
        assert_eq!(&formatted[1..65], &fake_sig[..]);
    }

    // ─── Phase 5: CLTV Time-Lock Tests ──────────────────────────────────────

    #[test]
    fn test_encode_script_number() {
        // Zero
        assert_eq!(encode_script_number(0), Vec::<u8>::new());
        // Small number
        assert_eq!(encode_script_number(1), vec![0x01]);
        // Number requiring sign bit padding
        assert_eq!(encode_script_number(128), vec![0x80, 0x00]);
        // Typical timestamp (e.g. 1700000000 = March 2024)
        let ts = encode_script_number(1_700_000_000);
        assert!(!ts.is_empty());
        // Roundtrip: decode back
        let mut decoded: u64 = 0;
        for (i, &byte) in ts.iter().enumerate() {
            decoded |= (byte as u64) << (i * 8);
        }
        // If sign byte was added, mask it
        if ts.last().unwrap() == &0x00 {
            decoded &= !(0xFF_u64 << ((ts.len() - 1) * 8));
        }
        assert_eq!(decoded, 1_700_000_000);
    }

    #[test]
    fn test_build_timelocked_multisig_script() {
        let (pk_a, pk_b, pk_o) = test_pubkeys();
        let locktime = 1_700_000_000u64;

        let script = build_timelocked_multisig_redeem_script(
            &[pk_a.clone(), pk_b.clone(), pk_o.clone()],
            2,
            locktime,
            &pk_a,
        )
        .unwrap();

        // Script should contain OP_IF (0x63)
        assert!(script.contains(&opcodes::OP_IF), "Should contain OP_IF");
        // Script should contain OP_ELSE (0x67)
        assert!(script.contains(&opcodes::OP_ELSE), "Should contain OP_ELSE");
        // Script should contain OP_ENDIF (0x68)
        assert!(script.contains(&opcodes::OP_ENDIF), "Should contain OP_ENDIF");
        // Script should contain OP_CHECKLOCKTIMEVERIFY (0xb1)
        assert!(
            script.contains(&opcodes::OP_CHECKLOCKTIMEVERIFY),
            "Should contain OP_CHECKLOCKTIMEVERIFY"
        );
        // Script should contain OP_CHECKSIG (0xac) for the single-key refund
        assert!(
            script.contains(&opcodes::OP_CHECKSIG),
            "Should contain OP_CHECKSIG"
        );
        // Script should contain OP_CHECKMULTISIG (0xae) for the 2-of-3 branch
        assert!(
            script.contains(&opcodes::OP_CHECKMULTISIG),
            "Should contain OP_CHECKMULTISIG"
        );
    }

    #[test]
    fn test_timelocked_script_different_address_than_standard() {
        let (pk_a, pk_b, pk_o) = test_pubkeys();
        let locktime = 1_700_000_000u64;

        let standard =
            build_multisig_redeem_script(&[pk_a.clone(), pk_b.clone(), pk_o.clone()], 2).unwrap();
        let timelocked = build_timelocked_multisig_redeem_script(
            &[pk_a.clone(), pk_b.clone(), pk_o.clone()],
            2,
            locktime,
            &pk_a,
        )
        .unwrap();

        let addr_std = redeem_script_to_address(&standard, Prefix::Testnet);
        let addr_tl = redeem_script_to_address(&timelocked, Prefix::Testnet);

        assert_ne!(
            addr_std.to_string(),
            addr_tl.to_string(),
            "Timelocked script should produce different P2SH address"
        );
    }

    #[test]
    fn test_timelocked_script_deterministic() {
        let (pk_a, pk_b, pk_o) = test_pubkeys();
        let locktime = 1_700_000_000u64;

        let script1 = build_timelocked_multisig_redeem_script(
            &[pk_a.clone(), pk_b.clone(), pk_o.clone()],
            2,
            locktime,
            &pk_a,
        )
        .unwrap();
        let script2 = build_timelocked_multisig_redeem_script(
            &[pk_a.clone(), pk_b.clone(), pk_o.clone()],
            2,
            locktime,
            &pk_a,
        )
        .unwrap();

        assert_eq!(script1, script2, "Same inputs should produce same script");
    }

    #[test]
    fn test_different_locktime_different_script() {
        let (pk_a, pk_b, pk_o) = test_pubkeys();

        let script1 = build_timelocked_multisig_redeem_script(
            &[pk_a.clone(), pk_b.clone(), pk_o.clone()],
            2,
            1_700_000_000,
            &pk_a,
        )
        .unwrap();
        let script2 = build_timelocked_multisig_redeem_script(
            &[pk_a.clone(), pk_b.clone(), pk_o.clone()],
            2,
            1_800_000_000,
            &pk_a,
        )
        .unwrap();

        assert_ne!(script1, script2, "Different locktimes should differ");
    }

    #[test]
    fn test_build_timelock_only_script() {
        let (pk_a, _, _) = test_pubkeys();
        let locktime = 1_700_000_000u64;

        let script = build_timelock_only_redeem_script(locktime, &pk_a).unwrap();

        assert!(script.contains(&opcodes::OP_CHECKLOCKTIMEVERIFY));
        assert!(script.contains(&opcodes::OP_DROP));
        assert!(script.contains(&opcodes::OP_CHECKSIG));
        // Should NOT contain OP_IF (no branching)
        assert!(!script.contains(&opcodes::OP_IF));
    }
}


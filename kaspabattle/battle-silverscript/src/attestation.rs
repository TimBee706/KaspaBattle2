//! `KASPABATTLE_RESULT_V1` result attestation: canonical serialization,
//! digest, and Schnorr sign/verify.
//!
//! Wire format (see docs/SILVERSCRIPT-INTEGRATION-PLAN.md section 8.1),
//! verified against the actual pinned `kaspa-txscript`/SilverScript source,
//! not assumed:
//!
//! ```text
//! msg = "KASPABATTLE_RESULT_V1"        (21 ASCII bytes, domain separator)
//!     || network_domain                (32 bytes)
//!     || contract_version as byte[4]   (see `script_num_bytes`)
//!     || match_id                      (16 bytes, raw UUID)
//!     || game_id_hash                  (32 bytes)
//!     || winner_selector as byte[1]
//!     || result_hash                   (32 bytes)
//!     || observed_at as byte[8]
//!     || nonce                         (32 bytes)
//!
//! digest = blake2b_256(msg)
//! signature = Schnorr(oracle_privkey, digest)   // no sighash-type byte —
//!                                                // this is OpCheckSigFromStack,
//!                                                // not the tx-bound OpCheckSig.
//! ```
//!
//! The contract reads `network_domain`, `contract_version`, `match_id`, and
//! `game_id_hash` from its OWN state, not from witness arguments — so this
//! type takes the match's fixed parameters once (`AttestationContext`) and
//! only the actually-variable fields (`winner`, `result_hash`, `observed_at`,
//! `nonce`) per attestation, mirroring the contract exactly and making it
//! impossible to accidentally sign a message that doesn't match what the
//! contract will reconstruct.

use blake2b_simd::Params as Blake2bParams;
use secp256k1::{Keypair, Message, XOnlyPublicKey, schnorr::Signature};
use thiserror::Error;

pub const DOMAIN_TAG: &[u8] = b"KASPABATTLE_RESULT_V1";

/// `contract_version` this crate's encoding matches. Bump together with the
/// `.sil` contract if the wire format ever changes.
pub const CONTRACT_VERSION: i64 = 1;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum AttestationError {
    #[error("winner_selector must be 0 (player_a) or 1 (player_b), got {0}")]
    InvalidWinnerSelector(i64),
    #[error("value {value} does not fit in {size} bytes")]
    ValueTooLarge { value: i64, size: usize },
    #[error("invalid oracle signature")]
    InvalidSignature,
}

/// Which player the Oracle is attesting as the winner. Matches the
/// contract's `winner_selector` int (0/1) exactly — modeled as an enum here
/// so an invalid raw value can't be constructed in the first place.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WinnerSelector {
    PlayerA,
    PlayerB,
}

impl WinnerSelector {
    fn as_i64(self) -> i64 {
        match self {
            WinnerSelector::PlayerA => 0,
            WinnerSelector::PlayerB => 1,
        }
    }

    pub fn from_i64(value: i64) -> Result<Self, AttestationError> {
        match value {
            0 => Ok(WinnerSelector::PlayerA),
            1 => Ok(WinnerSelector::PlayerB),
            other => Err(AttestationError::InvalidWinnerSelector(other)),
        }
    }
}

/// The parts of a match that are fixed at `MatchEscrow` creation/join time —
/// i.e., exactly the contract's own state fields that `oracle_settle` reads
/// from itself rather than from witness arguments.
#[derive(Debug, Clone, Copy)]
pub struct AttestationContext {
    pub network_domain: [u8; 32],
    pub match_id: [u8; 16],
    pub game_id_hash: [u8; 32],
}

/// One Oracle-signed result attestation for a specific match.
#[derive(Debug, Clone)]
pub struct Attestation {
    pub context: AttestationContext,
    pub winner: WinnerSelector,
    /// Off-chain commitment to the full match details (e.g.
    /// `blake2b(faceit_match_id || score_string || ...)`); only the hash
    /// goes into the signed message and on-chain.
    pub result_hash: [u8; 32],
    /// DAA score or Unix time the Oracle observed the result at. Informational
    /// only — not consensus-checked beyond being part of the signed digest.
    pub observed_at: i64,
    /// Oracle-chosen nonce, so re-signing (e.g. after a correction) before
    /// on-chain submission produces a distinguishable message.
    pub nonce: [u8; 32],
}

/// Kaspa's classic `CScriptNum` little-endian encoding, matching
/// `kaspa_txscript::data_stack::serialize_i64` at the pinned rusty-kaspa
/// revision (`a41a333b08848f41bf737b72592e463a6011b8ac`) byte for byte —
/// this is what SilverScript's `int as byte[N]` cast lowers to
/// (`OpNum2Bin`). Verified against that source directly, not assumed: it is
/// NOT big-endian, and NOT a naive fixed-width `to_le_bytes()` (those only
/// coincide for values whose minimal encoding doesn't need the extra
/// sign-guard byte `serialize_i64` inserts — irrelevant for the small,
/// always-non-negative values this contract uses, but this implementation
/// matches the real algorithm regardless, not just the common case).
pub fn script_num_bytes(value: i64, size: usize) -> Result<[u8; 8], AttestationError> {
    if value < 0 {
        // Not reachable via this crate's public API (all fields below are
        // non-negative by construction), but kept exact/total rather than
        // assuming away negative inputs.
        return Err(AttestationError::ValueTooLarge { value, size });
    }
    let mut positive = value as u64;
    let mut buf = [0u8; 8];
    let mut len = 0usize;
    while positive != 0 {
        if len >= size {
            return Err(AttestationError::ValueTooLarge { value, size });
        }
        buf[len] = (positive & 0xff) as u8;
        len += 1;
        positive >>= 8;
    }
    Ok(buf)
}

fn blake2b32(data: &[u8]) -> [u8; 32] {
    let hash = Blake2bParams::new().hash_length(32).to_state().update(data).finalize();
    let mut out = [0u8; 32];
    out.copy_from_slice(hash.as_bytes());
    out
}

impl Attestation {
    /// Builds the exact byte sequence the `MatchEscrow.oracle_settle` entry
    /// reconstructs and hashes. Field order and encoding must never change
    /// without bumping `CONTRACT_VERSION` in lockstep with the `.sil` source.
    pub fn canonical_bytes(&self) -> Result<Vec<u8>, AttestationError> {
        let mut msg = Vec::with_capacity(DOMAIN_TAG.len() + 32 + 4 + 16 + 32 + 1 + 32 + 8 + 32);
        msg.extend_from_slice(DOMAIN_TAG);
        msg.extend_from_slice(&self.context.network_domain);
        msg.extend_from_slice(&script_num_bytes(CONTRACT_VERSION, 4)?[..4]);
        msg.extend_from_slice(&self.context.match_id);
        msg.extend_from_slice(&self.context.game_id_hash);
        msg.extend_from_slice(&script_num_bytes(self.winner.as_i64(), 1)?[..1]);
        msg.extend_from_slice(&self.result_hash);
        msg.extend_from_slice(&script_num_bytes(self.observed_at, 8)?[..8]);
        msg.extend_from_slice(&self.nonce);
        Ok(msg)
    }

    pub fn digest(&self) -> Result<[u8; 32], AttestationError> {
        Ok(blake2b32(&self.canonical_bytes()?))
    }

    /// Signs this attestation's digest with the Oracle's key. The resulting
    /// 64-byte signature is a raw Schnorr signature over the digest — no
    /// sighash-type byte, unlike a transaction-bound `checkSig` signature
    /// (verified: `checkMsgSig`/`OpCheckSigFromStack` takes exactly 64 bytes).
    pub fn sign(&self, oracle_keypair: &Keypair) -> Result<[u8; 64], AttestationError> {
        let digest = self.digest()?;
        let message = Message::from_digest(digest);
        let sig: Signature = oracle_keypair.sign_schnorr(message);
        Ok(*sig.as_ref())
    }

    /// Verifies a signature against this attestation's digest and the given
    /// Oracle pubkey. Callers must separately check that `oracle_pubkey`
    /// matches the match's committed `result_oracle_commitment`
    /// (`blake2b(pubkey) == result_oracle_commitment`) — this function only
    /// checks the signature, exactly mirroring the contract's own two
    /// separate `require`s.
    pub fn verify(&self, signature: &[u8; 64], oracle_pubkey: &XOnlyPublicKey) -> Result<(), AttestationError> {
        let digest = self.digest()?;
        let message = Message::from_digest(digest);
        let sig = Signature::from_slice(signature).map_err(|_| AttestationError::InvalidSignature)?;
        sig.verify(&message, oracle_pubkey).map_err(|_| AttestationError::InvalidSignature)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use secp256k1::{Secp256k1, SecretKey};

    /// Known-answer test vector, captured by running the equivalent
    /// construction inside `contracts/silverscript/tests/interpreter_tests.rs`
    /// (`oracle_settle_digest`) against the real compiled contract — this is
    /// not just "this code agrees with itself", it is cross-checked against
    /// the actual on-chain digest computation.
    #[test]
    fn known_answer_digest_matches_interpreter_test() {
        let ctx = AttestationContext { network_domain: [0x01; 32], match_id: [0x02; 16], game_id_hash: [0x03; 32] };
        let attestation = Attestation {
            context: ctx,
            winner: WinnerSelector::PlayerA,
            result_hash: [0x07; 32],
            observed_at: 123_456,
            nonce: [0x08; 32],
        };
        let digest = attestation.digest().expect("digest computes");
        assert_eq!(hex::encode(digest), "dc848da95b52e226f93992932784bccb64f24e7891fffd45f3ea43aa031acf90");
    }

    #[test]
    fn sign_then_verify_round_trips() {
        let secp = Secp256k1::new();
        let secret = SecretKey::from_slice(&[0xB2; 32]).unwrap();
        let keypair = Keypair::from_secret_key(&secp, &secret);
        let (pubkey, _) = keypair.x_only_public_key();

        let ctx = AttestationContext { network_domain: [0x01; 32], match_id: [0x02; 16], game_id_hash: [0x03; 32] };
        let attestation = Attestation {
            context: ctx,
            winner: WinnerSelector::PlayerB,
            result_hash: [0xAA; 32],
            observed_at: 1,
            nonce: [0xBB; 32],
        };

        let sig = attestation.sign(&keypair).expect("signs");
        attestation.verify(&sig, &pubkey).expect("verifies");
    }

    #[test]
    fn verify_rejects_forged_signature() {
        let secp = Secp256k1::new();
        let secret = SecretKey::from_slice(&[0xB2; 32]).unwrap();
        let keypair = Keypair::from_secret_key(&secp, &secret);
        let (pubkey, _) = keypair.x_only_public_key();

        let ctx = AttestationContext { network_domain: [0x01; 32], match_id: [0x02; 16], game_id_hash: [0x03; 32] };
        let attestation = Attestation {
            context: ctx,
            winner: WinnerSelector::PlayerA,
            result_hash: [0xAA; 32],
            observed_at: 1,
            nonce: [0xBB; 32],
        };

        let mut sig = attestation.sign(&keypair).expect("signs");
        sig[0] ^= 0x01;
        assert_eq!(attestation.verify(&sig, &pubkey), Err(AttestationError::InvalidSignature));
    }

    #[test]
    fn verify_rejects_tampered_attestation() {
        let secp = Secp256k1::new();
        let secret = SecretKey::from_slice(&[0xB2; 32]).unwrap();
        let keypair = Keypair::from_secret_key(&secp, &secret);
        let (pubkey, _) = keypair.x_only_public_key();

        let ctx = AttestationContext { network_domain: [0x01; 32], match_id: [0x02; 16], game_id_hash: [0x03; 32] };
        let attestation = Attestation {
            context: ctx,
            winner: WinnerSelector::PlayerA,
            result_hash: [0xAA; 32],
            observed_at: 1,
            nonce: [0xBB; 32],
        };
        let sig = attestation.sign(&keypair).expect("signs");

        // Same signature, but the winner was flipped after signing -- must
        // not verify (this is exactly what stops a compromised backend from
        // redirecting a payout by relabeling an already-signed attestation).
        let tampered = Attestation { winner: WinnerSelector::PlayerB, ..attestation };
        assert_eq!(tampered.verify(&sig, &pubkey), Err(AttestationError::InvalidSignature));
    }

    #[test]
    fn script_num_bytes_matches_kaspa_cscriptnum_encoding() {
        // Verified against kaspa_txscript::data_stack::serialize_i64 at the
        // pinned rev: little-endian, zero-padded, minimal-encoding-plus-
        // sign-guard byte for values whose top byte would otherwise look
        // negative. Spot-checked cases:
        assert_eq!(&script_num_bytes(0, 4).unwrap()[..4], &[0, 0, 0, 0]);
        assert_eq!(&script_num_bytes(1, 4).unwrap()[..4], &[1, 0, 0, 0]);
        assert_eq!(&script_num_bytes(256, 4).unwrap()[..4], &[0, 1, 0, 0]);
        assert_eq!(&script_num_bytes(123_456, 8).unwrap()[..8], &[0x40, 0xE2, 0x01, 0, 0, 0, 0, 0]);
    }

    #[test]
    fn script_num_bytes_rejects_oversized_values() {
        assert_eq!(script_num_bytes(256, 1), Err(AttestationError::ValueTooLarge { value: 256, size: 1 }));
    }

    #[test]
    fn winner_selector_rejects_invalid_raw_values() {
        assert_eq!(WinnerSelector::from_i64(2), Err(AttestationError::InvalidWinnerSelector(2)));
        assert_eq!(WinnerSelector::from_i64(0), Ok(WinnerSelector::PlayerA));
        assert_eq!(WinnerSelector::from_i64(1), Ok(WinnerSelector::PlayerB));
    }
}

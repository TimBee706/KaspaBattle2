//! Core data types for the multisig escrow system.

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use kaspa_consensus_core::tx::{Transaction, UtxoEntry};
use serde::Serialize;
use uuid::Uuid;

// ─── Escrow Lifecycle ────────────────────────────────────────────────────────

/// Status of a multisig escrow through its lifecycle.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum EscrowStatus {
    /// Multisig address generated, awaiting deposits from both players
    Created,
    /// One player has deposited, waiting for the other
    PartiallyFunded,
    /// Both players have deposited the required wager
    Funded,
    /// Payout TX has been created, collecting signatures
    Settling,
    /// Payout TX broadcast and confirmed — winner received funds
    Settled,
    /// Refund is in progress (e.g. timeout or mutual cancellation)
    Refunding,
    /// Refund completed — both players received their deposits back
    Refunded,
    /// Match timed out without resolution
    TimedOut,
    /// Manual resolution needed (dispute)
    Disputed,
}

// ─── Multisig Configuration ──────────────────────────────────────────────────

/// Configuration for a multisig escrow instance.
///
/// For v1 (2-of-3): threshold = 2, total_keys = 3
/// For v2 (3-of-5): threshold = 3, total_keys = 5
#[derive(Debug, Clone, Serialize)]
pub struct MultisigConfig {
    pub threshold: u8,
    pub total_keys: u8,
}

impl MultisigConfig {
    /// Standard 2-of-3 configuration for KaspaBattle v1.
    pub fn v1() -> Self {
        Self {
            threshold: 2,
            total_keys: 3,
        }
    }

    /// Future 3-of-5 configuration for KaspaBattle v2.
    pub fn v2() -> Self {
        Self {
            threshold: 3,
            total_keys: 5,
        }
    }
}

// ─── Multisig Escrow ─────────────────────────────────────────────────────────

/// Represents a complete multisig escrow instance for a match.
///
/// Created when a match is initiated, tracks the full lifecycle from
/// address generation through deposit collection, game resolution, and payout.
#[derive(Debug, Clone, Serialize)]
pub struct MultisigEscrow {
    /// Unique match/challenge identifier
    pub match_id: Uuid,

    /// x-only public key of Player A (32 bytes, hex-encoded)
    pub pubkey_a_hex: String,

    /// x-only public key of Player B (32 bytes, hex-encoded)
    pub pubkey_b_hex: String,

    /// x-only public key of the Oracle/Platform (32 bytes, hex-encoded)
    pub pubkey_oracle_hex: String,

    /// Multisig configuration (threshold, total_keys)
    pub config: MultisigConfig,

    /// The raw redeem script bytes (hex-encoded for storage)
    pub redeem_script_hex: String,

    /// The P2SH address derived from the redeem script
    pub p2sh_address: String,

    /// Wager amount per player in sompi
    pub wager_per_player_sompi: u64,

    /// Current escrow status
    pub status: EscrowStatus,

    /// Optional CLTV time-lock timestamp (Unix seconds)
    pub timelock_timestamp: Option<u64>,

    /// When this escrow was created
    pub created_at: DateTime<Utc>,
}

// ─── Signature Bundle ────────────────────────────────────────────────────────

/// Tracks an unsigned transaction and the partial signatures collected so far.
///
/// Used during the signing phase: the backend creates the unsigned TX,
/// then collects signatures from the winner + oracle (or any 2-of-3 combo).
#[derive(Debug, Clone)]
pub struct SignatureBundle {
    /// The unsigned payout/refund transaction
    pub unsigned_tx: Transaction,

    /// UTXO entries for each input (needed for sighash computation)
    pub utxo_entries: Vec<UtxoEntry>,

    /// The redeem script (needed in the final ScriptSig)
    pub redeem_script: Vec<u8>,

    /// Collected signatures: pubkey_hex → signature bytes (per input)
    /// For single-input TXs (most escrows), each signer provides 1 signature.
    pub signatures: HashMap<String, Vec<u8>>,

    /// Number of signatures required (= threshold)
    pub required_sigs: u8,
}

impl SignatureBundle {
    /// Check whether enough signatures have been collected.
    pub fn is_complete(&self) -> bool {
        self.signatures.len() >= self.required_sigs as usize
    }

    /// Number of signatures still needed.
    pub fn sigs_remaining(&self) -> usize {
        (self.required_sigs as usize).saturating_sub(self.signatures.len())
    }
}

// ─── Payout Instruction ──────────────────────────────────────────────────────

/// Describes how the escrow funds should be distributed.
#[derive(Debug, Clone, Serialize)]
pub struct PayoutInstruction {
    /// Winner's Kaspa address (P2PK)
    pub winner_address: String,

    /// Amount to send to the winner (in sompi)
    pub winner_amount_sompi: u64,

    /// Platform/treasury address for fees
    pub platform_address: String,

    /// Platform fee amount (in sompi)
    pub platform_fee_sompi: u64,
}

// ─── Escrow Creation Result ──────────────────────────────────────────────────

/// Result returned when a new multisig escrow is created.
#[derive(Debug, Clone, Serialize)]
pub struct MultisigEscrowInfo {
    pub match_id: String,
    pub escrow_address: String,
    pub redeem_script_hex: String,
    pub pubkeys: Vec<String>,
    pub threshold: u8,
    pub wager_per_player_sompi: u64,
    pub wager_per_player_kas: f64,
    pub timelock_timestamp: Option<u64>,
}

// ─── Payout Result ───────────────────────────────────────────────────────────

/// Result of a successful multisig payout.
#[derive(Debug, Clone, Serialize)]
pub struct MultisigPayoutResult {
    pub match_id: String,
    pub tx_id: String,
    pub winner_address: String,
    pub winner_amount_sompi: u64,
    pub platform_fee_sompi: u64,
    pub network_fee_sompi: u64,
    pub timestamp: String,
}

// ─── Deposit Status ──────────────────────────────────────────────────────────

/// Deposit tracking for a multisig escrow.
#[derive(Debug, Clone, Serialize)]
pub struct MultisigDepositStatus {
    pub deposited_sompi: u64,
    pub required_sompi: u64,
    pub is_fully_funded: bool,
    pub utxo_count: u32,
}

// ─── Key Roles ───────────────────────────────────────────────────────────────

/// Identifies a signer role in the multisig.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub enum SignerRole {
    PlayerA,
    PlayerB,
    Oracle,
}

impl std::fmt::Display for SignerRole {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SignerRole::PlayerA => write!(f, "PlayerA"),
            SignerRole::PlayerB => write!(f, "PlayerB"),
            SignerRole::Oracle => write!(f, "Oracle"),
        }
    }
}

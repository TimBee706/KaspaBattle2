//! BattleCommand — On-chain command types for the KaspaBattle Episode.
//!
//! All commands are Borsh-serializable and sent as Kaspa TX payloads via
//! the kdapp TransactionGenerator. Each command advances the match state
//! machine inside `BattleEpisode`.

use borsh::{BorshDeserialize, BorshSerialize};
use crate::kdapp_pki::PubKey;

/// Commands that participants can send to a BattleEpisode.
///
/// Each variant maps to a state transition in the match lifecycle:
/// ```text
/// CreateMatch → JoinMatch → ConfirmDeposit (×2) → ReportResult → InitiatePayout
///                                                  └→ Dispute
///                    └→ CancelMatch
/// ```
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub enum BattleCommand {
    /// Player A creates a match with a wager amount and game type.
    CreateMatch {
        wager_sompi: u64,
        game_type: GameType,
    },
    /// Player B joins an existing match.
    JoinMatch,
    /// A player confirms their on-chain deposit.
    ConfirmDeposit {
        tx_hash: [u8; 32],
        amount_sompi: u64,
    },
    /// Oracle reports the match result.
    ReportResult {
        winner_pubkey: PubKey,
        faceit_match_id: [u8; 32],
        score_a: u8,
        score_b: u8,
    },
    /// Trigger payout after result is confirmed.
    InitiatePayout,
    /// A participant disputes the reported result.
    Dispute {
        reason_code: u8,
    },
    /// Cancel the match (only before Lock).
    CancelMatch {
        reason_code: u8,
    },
}

/// Supported game types.
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, PartialEq, Eq)]
pub enum GameType {
    CS2,
}

/// Identifies which player side in a match.
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayerSide {
    A,
    B,
}

/// Match phase — on-chain state of a BattleEpisode.
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, PartialEq, Eq)]
pub enum MatchPhase {
    WaitingForOpponent,
    WaitingForDeposits { a_deposited: bool, b_deposited: bool },
    Locked,
    Resolved { winner_idx: u8 },
    Disputed { reason_code: u8, by_idx: u8 },
    Cancelled { reason_code: u8 },
    Completed,
}

/// Rollback data for DAG re-org handling.
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub enum BattleRollback {
    UndoCreateMatch,
    UndoJoinMatch { previous_phase: MatchPhase },
    UndoConfirmDeposit { player: PlayerSide, amount: u64 },
    UndoReportResult,
    UndoInitiatePayout,
    UndoDispute,
    UndoCancelMatch { previous_phase: MatchPhase },
}

/// Reason codes for CancelMatch and Dispute commands.
pub mod reason {
    pub const CANCEL_PLAYER_REQUEST: u8 = 1;
    pub const CANCEL_TIMEOUT: u8 = 2;
    pub const DISPUTE_SCORE_MISMATCH: u8 = 10;
    pub const DISPUTE_CHEAT: u8 = 11;
    pub const DISPUTE_OTHER: u8 = 12;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_command_borsh_roundtrip() {
        let cmd = BattleCommand::CreateMatch {
            wager_sompi: 5_000_000,
            game_type: GameType::CS2,
        };
        let encoded = borsh::to_vec(&cmd).unwrap();
        let decoded: BattleCommand = borsh::from_slice(&encoded).unwrap();
        match decoded {
            BattleCommand::CreateMatch { wager_sompi, game_type } => {
                assert_eq!(wager_sompi, 5_000_000);
                assert_eq!(game_type, GameType::CS2);
            }
            _ => panic!("Expected CreateMatch"),
        }
    }

    #[test]
    fn test_rollback_borsh_roundtrip() {
        let rollback = BattleRollback::UndoConfirmDeposit {
            player: PlayerSide::A,
            amount: 5_000_000,
        };
        let encoded = borsh::to_vec(&rollback).unwrap();
        let decoded: BattleRollback = borsh::from_slice(&encoded).unwrap();
        match decoded {
            BattleRollback::UndoConfirmDeposit { player, amount } => {
                assert_eq!(player, PlayerSide::A);
                assert_eq!(amount, 5_000_000);
            }
            _ => panic!("Expected UndoConfirmDeposit"),
        }
    }

    #[test]
    fn test_match_phase_borsh_roundtrip() {
        let phase = MatchPhase::WaitingForDeposits {
            a_deposited: true,
            b_deposited: false,
        };
        let encoded = borsh::to_vec(&phase).unwrap();
        let decoded: MatchPhase = borsh::from_slice(&encoded).unwrap();
        assert_eq!(decoded, phase);
    }
}

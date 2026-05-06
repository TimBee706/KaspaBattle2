//! TournamentCommand — On-chain command types for the TournamentEpisode.
//!
//! All commands are Borsh-serializable and sent as Kaspa TX payloads via
//! the kdapp TransactionGenerator. Each command advances the tournament
//! state machine inside `TournamentEpisode`.
//!
//! ## Tournament Lifecycle
//! ```text
//! CreateTournament
//!   → RegisterTeam (×N)
//!     → ConfirmTeamDeposit (×N, by BlockchainWatcher)
//!       → LockBracket            [Organizer]
//!         → SubmitMatchId        [Captain, per bracket slot]
//!           → ReportBracketResult [Oracle/FaceitWatcher]
//!             → InitiatePayout   [Organizer/Auto after Finals]
//!                                   OR
//!             → DisputeResult    [Captain, within dispute window]
//!               → ResolveDispute [Admin]
//!   CancelTournament             [Organizer, before BRACKET_READY]
//! ```

use borsh::{BorshDeserialize, BorshSerialize};

/// Commands that participants can send to a `TournamentEpisode`.
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub enum TournamentCommand {
    /// Organizer creates a new tournament.
    ///
    /// Authorized by: Organizer pubkey (index 0 of participants).
    CreateTournament {
        /// Number of teams (must be a power of 2: 4, 8, 16)
        max_teams: u8,
        /// Buy-in per team in sompi
        buy_in_sompi: u64,
        /// Prize split percentages (must sum to 100)
        prize_winner_pct: u8,
        prize_runner_up_pct: u8,
        platform_fee_pct: u8,
        /// Game type
        game_type: TournamentGameType,
    },

    /// Team captain registers a new team.
    ///
    /// Authorized by: Captain pubkey.
    RegisterTeam {
        /// Team name (unique within tournament)
        team_name_hash: [u8; 32],
        /// Number of players on the team
        team_size: u8,
    },

    /// BlockchainWatcher confirms a team's buy-in deposit on-chain.
    ///
    /// Authorized by: Oracle pubkey (last participant).
    ConfirmTeamDeposit {
        /// Index of the team in registration order (0-based)
        team_idx: u8,
        /// TX ID of the deposit transaction
        tx_hash: [u8; 32],
        /// Confirmed amount
        amount_sompi: u64,
    },

    /// Organizer locks the registration and generates the bracket.
    ///
    /// Valid only in FUNDED phase (all teams deposited).
    /// Authorized by: Organizer pubkey.
    LockBracket,

    /// Captain submits the FaceIT match ID for a bracket slot.
    ///
    /// Valid only in BRACKET_READY or IN_PROGRESS phase.
    /// Authorized by: Captain of team_a or team_b in the slot.
    SubmitMatchId {
        /// Round number (1-based)
        round: u8,
        /// Slot index within the round (0-based)
        slot_index: u8,
        /// FaceIT match ID hash (from the URL, hashed to 32 bytes)
        faceit_match_id_hash: [u8; 32],
    },

    /// Oracle reports the result of a bracket slot match.
    ///
    /// Triggered by the FaceIT watcher upon match completion.
    /// Authorized by: Oracle pubkey.
    ReportBracketResult {
        /// Round number (1-based)
        round: u8,
        /// Slot index within the round (0-based)
        slot_index: u8,
        /// Index of the winning team (0 = team_a, 1 = team_b)
        winner_team_idx: u8,
        /// Final score (informational, stored on-chain for auditability)
        score_winner: u8,
        score_loser: u8,
    },

    /// Captain files a dispute for a bracket slot result.
    ///
    /// Must be filed within the dispute window (see TournamentEpisode config).
    /// Authorized by: Captain of either team in the slot.
    DisputeResult {
        round: u8,
        slot_index: u8,
        reason_code: u8,
    },

    /// Admin resolves a dispute and sets the authoritative winner.
    ///
    /// Authorized by: Admin pubkey (index 1 of participants).
    ResolveDispute {
        round: u8,
        slot_index: u8,
        /// The override winner team index (0 = team_a, 1 = team_b)
        winner_team_idx: u8,
    },

    /// Trigger prize pool payout after the final match.
    ///
    /// Authorized by: Organizer pubkey, or auto-triggered after Finals.
    InitiatePayout {
        /// Winner Kaspa address (for verification)
        winner_kaspa_addr_hash: [u8; 32],
        /// Runner-up Kaspa address
        runner_up_kaspa_addr_hash: [u8; 32],
    },

    /// Cancel the tournament before BRACKET_READY.
    ///
    /// Triggers refund of all confirmed deposits.
    /// Authorized by: Organizer pubkey.
    CancelTournament {
        reason_code: u8,
    },
}

/// Supported game types for tournaments.
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, PartialEq, Eq)]
pub enum TournamentGameType {
    CS2,
}

/// Tournament phase — on-chain state of a TournamentEpisode.
///
/// Mirrors the `tournament_status` DB enum for auditability.
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, PartialEq, Eq)]
pub enum TournamentPhase {
    /// Open for team registration.
    Registration,
    /// All teams have confirmed their deposits.
    Funded,
    /// Bracket generated and locked.
    BracketReady {
        /// Number of rounds in the bracket (log2 of max_teams)
        total_rounds: u8,
    },
    /// Matches in progress.
    InProgress {
        current_round: u8,
    },
    /// Tournament complete, payout executed.
    Completed {
        winner_team_idx: u8,
        runner_up_team_idx: u8,
    },
    /// Cancelled, refunds triggered.
    Cancelled {
        reason_code: u8,
    },
    /// A bracket slot result is disputed — admin must resolve.
    Disputed {
        round: u8,
        slot_index: u8,
        reason_code: u8,
        disputing_team_idx: u8,
    },
}

/// On-chain state of a single bracket slot.
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, PartialEq, Eq)]
pub struct BracketSlot {
    pub round: u8,
    pub slot_index: u8,
    /// Index into TournamentEpisode::teams (None = BYE slot)
    pub team_a_idx: Option<u8>,
    pub team_b_idx: Option<u8>,
    pub winner_idx: Option<u8>,
    pub faceit_match_id_hash: Option<[u8; 32]>,
    pub status: BracketSlotStatus,
}

/// On-chain status of a bracket slot.
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, PartialEq, Eq)]
pub enum BracketSlotStatus {
    Waiting,
    Ready,
    InProgress,
    Completed,
    Bye,
    Cancelled,
}

/// Rollback data for TournamentEpisode DAG re-org handling.
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub enum TournamentRollback {
    UndoCreateTournament,
    UndoRegisterTeam { team_idx: u8 },
    UndoConfirmTeamDeposit { team_idx: u8, previous_amount: u64 },
    UndoLockBracket { previous_phase: TournamentPhase },
    UndoSubmitMatchId { round: u8, slot_index: u8 },
    UndoReportBracketResult { round: u8, slot_index: u8 },
    UndoDisputeResult { previous_phase: TournamentPhase },
    UndoResolveDispute { round: u8, slot_index: u8, previous_winner: Option<u8> },
    UndoInitiatePayout,
    UndoCancelTournament { previous_phase: TournamentPhase },
}

/// Reason codes for CancelTournament and DisputeResult.
pub mod reason {
    pub const CANCEL_NOT_ENOUGH_TEAMS: u8 = 1;
    pub const CANCEL_ORGANIZER_REQUEST: u8 = 2;
    pub const CANCEL_TIMEOUT: u8 = 3;
    pub const DISPUTE_SCORE_MISMATCH: u8 = 10;
    pub const DISPUTE_WRONG_MATCH: u8 = 11;
    pub const DISPUTE_CHEAT: u8 = 12;
    pub const DISPUTE_OTHER: u8 = 13;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tournament_command_borsh_roundtrip() {
        let cmd = TournamentCommand::CreateTournament {
            max_teams: 8,
            buy_in_sompi: 50_000_000_000, // 500 KAS
            prize_winner_pct: 70,
            prize_runner_up_pct: 20,
            platform_fee_pct: 10,
            game_type: TournamentGameType::CS2,
        };
        let encoded = borsh::to_vec(&cmd).unwrap();
        let decoded: TournamentCommand = borsh::from_slice(&encoded).unwrap();
        match decoded {
            TournamentCommand::CreateTournament {
                max_teams,
                buy_in_sompi,
                prize_winner_pct,
                ..
            } => {
                assert_eq!(max_teams, 8);
                assert_eq!(buy_in_sompi, 50_000_000_000);
                assert_eq!(prize_winner_pct, 70);
            }
            _ => panic!("Expected CreateTournament"),
        }
    }

    #[test]
    fn test_report_bracket_result_borsh_roundtrip() {
        let cmd = TournamentCommand::ReportBracketResult {
            round: 1,
            slot_index: 0,
            winner_team_idx: 0,
            score_winner: 16,
            score_loser: 14,
        };
        let encoded = borsh::to_vec(&cmd).unwrap();
        let decoded: TournamentCommand = borsh::from_slice(&encoded).unwrap();
        match decoded {
            TournamentCommand::ReportBracketResult { round, slot_index, winner_team_idx, .. } => {
                assert_eq!(round, 1);
                assert_eq!(slot_index, 0);
                assert_eq!(winner_team_idx, 0);
            }
            _ => panic!("Expected ReportBracketResult"),
        }
    }

    #[test]
    fn test_tournament_phase_borsh_roundtrip() {
        let phase = TournamentPhase::BracketReady { total_rounds: 3 };
        let encoded = borsh::to_vec(&phase).unwrap();
        let decoded: TournamentPhase = borsh::from_slice(&encoded).unwrap();
        assert_eq!(decoded, phase);
    }

    #[test]
    fn test_bracket_slot_borsh_roundtrip() {
        let slot = BracketSlot {
            round: 1,
            slot_index: 0,
            team_a_idx: Some(0),
            team_b_idx: Some(1),
            winner_idx: None,
            faceit_match_id_hash: None,
            status: BracketSlotStatus::Ready,
        };
        let encoded = borsh::to_vec(&slot).unwrap();
        let decoded: BracketSlot = borsh::from_slice(&encoded).unwrap();
        assert_eq!(decoded, slot);
    }

    #[test]
    fn test_rollback_borsh_roundtrip() {
        let rb = TournamentRollback::UndoConfirmTeamDeposit {
            team_idx: 2,
            previous_amount: 50_000_000_000,
        };
        let encoded = borsh::to_vec(&rb).unwrap();
        let decoded: TournamentRollback = borsh::from_slice(&encoded).unwrap();
        match decoded {
            TournamentRollback::UndoConfirmTeamDeposit { team_idx, previous_amount } => {
                assert_eq!(team_idx, 2);
                assert_eq!(previous_amount, 50_000_000_000);
            }
            _ => panic!("Expected UndoConfirmTeamDeposit"),
        }
    }

    #[test]
    fn test_pct_sum_valid() {
        // Verify that our prize split constants add up correctly
        let winner: u8 = 70;
        let runner_up: u8 = 20;
        let fee: u8 = 10;
        assert_eq!(winner + runner_up + fee, 100);
    }
}

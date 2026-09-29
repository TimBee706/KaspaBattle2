//! BattleEpisode — implements the kdapp `Episode` trait for KaspaBattle matches.

use crate::commands::{BattleCommand, BattleRollback, GameType, MatchPhase, PlayerSide};
use crate::kdapp_episode::{Episode, EpisodeError, PayloadMetadata};
use crate::kdapp_pki::PubKey;

/// On-chain state of a single KaspaBattle match.
#[derive(Debug, Clone)]
pub struct BattleEpisode {
    pub participants: Vec<PubKey>,
    pub phase: MatchPhase,
    pub wager_sompi: u64,
    pub game_type: GameType,
    pub deposits: [u64; 2],
    pub winner_idx: Option<u8>,
    pub created_daa: u64,
}

/// Application-level errors for the BattleEpisode.
#[derive(Debug, Clone, thiserror::Error)]
pub enum BattleError {
    #[error("sender is not a participant in this match")]
    NotParticipant,
    #[error("invalid phase transition: {from} → {action}")]
    InvalidTransition { from: String, action: String },
    #[error("insufficient deposit: need {required} sompi, got {got}")]
    InsufficientDeposit { required: u64, got: u64 },
    #[error("unauthorized: only the oracle can report results")]
    UnauthorizedOracle,
    #[error("player has already deposited")]
    AlreadyDeposited,
    #[error("match is full, cannot join")]
    MatchFull,
    #[error("command requires authorization (signed command)")]
    AuthorizationRequired,
}

impl Episode for BattleEpisode {
    type Command = BattleCommand;
    type CommandRollback = BattleRollback;
    type CommandError = BattleError;

    fn initialize(participants: Vec<PubKey>, metadata: &PayloadMetadata) -> Self {
        BattleEpisode {
            participants,
            phase: MatchPhase::WaitingForOpponent,
            wager_sompi: 0,
            game_type: GameType::CS2,
            deposits: [0, 0],
            winner_idx: None,
            created_daa: metadata.accepting_daa,
        }
    }

    fn execute(
        &mut self,
        cmd: &Self::Command,
        authorization: Option<PubKey>,
        _metadata: &PayloadMetadata,
    ) -> Result<Self::CommandRollback, EpisodeError<Self::CommandError>> {
        match cmd {
            BattleCommand::CreateMatch {
                wager_sompi,
                game_type,
            } => {
                if !matches!(self.phase, MatchPhase::WaitingForOpponent) {
                    return Err(EpisodeError::InvalidCommand(
                        BattleError::InvalidTransition {
                            from: format!("{:?}", self.phase),
                            action: "CreateMatch".into(),
                        },
                    ));
                }
                self.wager_sompi = *wager_sompi;
                self.game_type = game_type.clone();
                Ok(BattleRollback::UndoCreateMatch)
            }

            BattleCommand::JoinMatch => {
                if !matches!(self.phase, MatchPhase::WaitingForOpponent) {
                    return Err(EpisodeError::InvalidCommand(
                        BattleError::InvalidTransition {
                            from: format!("{:?}", self.phase),
                            action: "JoinMatch".into(),
                        },
                    ));
                }
                if let Some(ref auth) = authorization {
                    if self.participants.first() == Some(auth) {
                        return Err(EpisodeError::InvalidCommand(BattleError::MatchFull));
                    }
                }
                let prev = self.phase.clone();
                self.phase = MatchPhase::WaitingForDeposits {
                    a_deposited: false,
                    b_deposited: false,
                };
                Ok(BattleRollback::UndoJoinMatch {
                    previous_phase: prev,
                })
            }

            BattleCommand::ConfirmDeposit { amount_sompi, .. } => {
                let auth = authorization.ok_or(EpisodeError::InvalidCommand(
                    BattleError::AuthorizationRequired,
                ))?;
                let side = self.identify_player(&auth)?;
                let idx = match side {
                    PlayerSide::A => 0usize,
                    PlayerSide::B => 1usize,
                };

                if !matches!(self.phase, MatchPhase::WaitingForDeposits { .. }) {
                    return Err(EpisodeError::InvalidCommand(
                        BattleError::InvalidTransition {
                            from: format!("{:?}", self.phase),
                            action: "ConfirmDeposit".into(),
                        },
                    ));
                }

                if self.deposits[idx] >= self.wager_sompi {
                    return Err(EpisodeError::InvalidCommand(BattleError::AlreadyDeposited));
                }

                self.deposits[idx] += amount_sompi;

                let a_done = self.deposits[0] >= self.wager_sompi;
                let b_done = self.deposits[1] >= self.wager_sompi;

                if a_done && b_done {
                    self.phase = MatchPhase::Locked;
                } else {
                    self.phase = MatchPhase::WaitingForDeposits {
                        a_deposited: a_done,
                        b_deposited: b_done,
                    };
                }

                Ok(BattleRollback::UndoConfirmDeposit {
                    player: side,
                    amount: *amount_sompi,
                })
            }

            BattleCommand::ReportResult { winner_pubkey, .. } => {
                if !matches!(self.phase, MatchPhase::Locked) {
                    return Err(EpisodeError::InvalidCommand(
                        BattleError::InvalidTransition {
                            from: format!("{:?}", self.phase),
                            action: "ReportResult".into(),
                        },
                    ));
                }
                if authorization.is_none() {
                    return Err(EpisodeError::InvalidCommand(
                        BattleError::UnauthorizedOracle,
                    ));
                }

                let w_idx = if self.participants.first() == Some(winner_pubkey) {
                    0u8
                } else if self.participants.get(1) == Some(winner_pubkey) {
                    1u8
                } else {
                    return Err(EpisodeError::InvalidCommand(BattleError::NotParticipant));
                };

                self.winner_idx = Some(w_idx);
                self.phase = MatchPhase::Resolved { winner_idx: w_idx };
                Ok(BattleRollback::UndoReportResult)
            }

            BattleCommand::InitiatePayout => {
                if !matches!(self.phase, MatchPhase::Resolved { .. }) {
                    return Err(EpisodeError::InvalidCommand(
                        BattleError::InvalidTransition {
                            from: format!("{:?}", self.phase),
                            action: "InitiatePayout".into(),
                        },
                    ));
                }
                self.phase = MatchPhase::Completed;
                Ok(BattleRollback::UndoInitiatePayout)
            }

            BattleCommand::Dispute { reason_code } => {
                if !matches!(self.phase, MatchPhase::Resolved { .. }) {
                    return Err(EpisodeError::InvalidCommand(
                        BattleError::InvalidTransition {
                            from: format!("{:?}", self.phase),
                            action: "Dispute".into(),
                        },
                    ));
                }
                let auth = authorization.ok_or(EpisodeError::InvalidCommand(
                    BattleError::AuthorizationRequired,
                ))?;
                let side = self.identify_player(&auth)?;
                let by_idx = match side {
                    PlayerSide::A => 0u8,
                    PlayerSide::B => 1u8,
                };
                self.phase = MatchPhase::Disputed {
                    reason_code: *reason_code,
                    by_idx,
                };
                Ok(BattleRollback::UndoDispute)
            }

            BattleCommand::CancelMatch { reason_code } => match &self.phase {
                MatchPhase::WaitingForOpponent | MatchPhase::WaitingForDeposits { .. } => {
                    let prev = self.phase.clone();
                    self.phase = MatchPhase::Cancelled {
                        reason_code: *reason_code,
                    };
                    Ok(BattleRollback::UndoCancelMatch {
                        previous_phase: prev,
                    })
                }
                _ => Err(EpisodeError::InvalidCommand(
                    BattleError::InvalidTransition {
                        from: format!("{:?}", self.phase),
                        action: "CancelMatch".into(),
                    },
                )),
            },
        }
    }

    fn rollback(&mut self, rollback: Self::CommandRollback) -> bool {
        match rollback {
            BattleRollback::UndoCreateMatch => {
                self.wager_sompi = 0;
                self.phase = MatchPhase::WaitingForOpponent;
                true
            }
            BattleRollback::UndoJoinMatch { previous_phase } => {
                self.phase = previous_phase;
                true
            }
            BattleRollback::UndoConfirmDeposit { player, amount } => {
                let idx = match player {
                    PlayerSide::A => 0usize,
                    PlayerSide::B => 1usize,
                };
                self.deposits[idx] = self.deposits[idx].saturating_sub(amount);
                if matches!(self.phase, MatchPhase::Locked) {
                    self.phase = MatchPhase::WaitingForDeposits {
                        a_deposited: self.deposits[0] >= self.wager_sompi,
                        b_deposited: self.deposits[1] >= self.wager_sompi,
                    };
                }
                true
            }
            BattleRollback::UndoReportResult => {
                self.winner_idx = None;
                self.phase = MatchPhase::Locked;
                true
            }
            BattleRollback::UndoInitiatePayout => {
                if let Some(w_idx) = self.winner_idx {
                    self.phase = MatchPhase::Resolved { winner_idx: w_idx };
                }
                true
            }
            BattleRollback::UndoDispute => {
                if let Some(w_idx) = self.winner_idx {
                    self.phase = MatchPhase::Resolved { winner_idx: w_idx };
                }
                true
            }
            BattleRollback::UndoCancelMatch { previous_phase } => {
                self.phase = previous_phase;
                true
            }
        }
    }
}

impl BattleEpisode {
    fn identify_player(&self, pubkey: &PubKey) -> Result<PlayerSide, EpisodeError<BattleError>> {
        if self.participants.first() == Some(pubkey) {
            Ok(PlayerSide::A)
        } else if self.participants.get(1) == Some(pubkey) {
            Ok(PlayerSide::B)
        } else {
            Err(EpisodeError::InvalidCommand(BattleError::NotParticipant))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kdapp_pki::generate_keypair;

    fn make_metadata() -> PayloadMetadata {
        PayloadMetadata {
            accepting_hash: kaspa_hashes::Hash::from_bytes([0u8; 32]),
            accepting_daa: 1000,
            accepting_time: 1234567890,
            tx_id: kaspa_hashes::Hash::from_bytes([1u8; 32]),
        }
    }

    #[test]
    fn test_full_lifecycle_happy_path() {
        let (_, pk_a) = generate_keypair();
        let (_, pk_b) = generate_keypair();
        let meta = make_metadata();

        let mut ep = BattleEpisode::initialize(vec![pk_a, pk_b], &meta);
        assert!(matches!(ep.phase, MatchPhase::WaitingForOpponent));

        // CreateMatch
        ep.execute(
            &BattleCommand::CreateMatch {
                wager_sompi: 5_000_000,
                game_type: GameType::CS2,
            },
            Some(pk_a),
            &meta,
        )
        .unwrap();
        assert_eq!(ep.wager_sompi, 5_000_000);

        // JoinMatch
        ep.execute(&BattleCommand::JoinMatch, Some(pk_b), &meta)
            .unwrap();
        assert!(matches!(
            ep.phase,
            MatchPhase::WaitingForDeposits {
                a_deposited: false,
                b_deposited: false
            }
        ));

        // ConfirmDeposit A
        ep.execute(
            &BattleCommand::ConfirmDeposit {
                tx_hash: [2u8; 32],
                amount_sompi: 5_000_000,
            },
            Some(pk_a),
            &meta,
        )
        .unwrap();
        assert!(matches!(
            ep.phase,
            MatchPhase::WaitingForDeposits {
                a_deposited: true,
                b_deposited: false
            }
        ));

        // ConfirmDeposit B
        ep.execute(
            &BattleCommand::ConfirmDeposit {
                tx_hash: [3u8; 32],
                amount_sompi: 5_000_000,
            },
            Some(pk_b),
            &meta,
        )
        .unwrap();
        assert!(matches!(ep.phase, MatchPhase::Locked));

        // ReportResult
        ep.execute(
            &BattleCommand::ReportResult {
                winner_pubkey: pk_a,
                faceit_match_id: [4u8; 32],
                score_a: 16,
                score_b: 14,
            },
            Some(pk_a),
            &meta,
        )
        .unwrap();
        assert!(matches!(ep.phase, MatchPhase::Resolved { winner_idx: 0 }));

        // InitiatePayout
        ep.execute(&BattleCommand::InitiatePayout, None, &meta)
            .unwrap();
        assert!(matches!(ep.phase, MatchPhase::Completed));
    }

    #[test]
    fn test_invalid_join_when_locked() {
        let (_, pk_a) = generate_keypair();
        let (_, pk_b) = generate_keypair();
        let (_, pk_c) = generate_keypair();
        let meta = make_metadata();

        let mut ep = BattleEpisode::initialize(vec![pk_a, pk_b], &meta);
        ep.phase = MatchPhase::Locked;

        assert!(ep
            .execute(&BattleCommand::JoinMatch, Some(pk_c), &meta)
            .is_err());
    }

    #[test]
    fn test_dispute_after_resolved() {
        let (_, pk_a) = generate_keypair();
        let (_, pk_b) = generate_keypair();
        let meta = make_metadata();

        let mut ep = BattleEpisode::initialize(vec![pk_a, pk_b], &meta);
        ep.phase = MatchPhase::Resolved { winner_idx: 0 };
        ep.winner_idx = Some(0);

        ep.execute(
            &BattleCommand::Dispute {
                reason_code: crate::commands::reason::DISPUTE_SCORE_MISMATCH,
            },
            Some(pk_b),
            &meta,
        )
        .unwrap();
        assert!(matches!(
            ep.phase,
            MatchPhase::Disputed {
                reason_code: 10,
                by_idx: 1
            }
        ));
    }

    #[test]
    fn test_rollback_confirm_deposit() {
        let (_, pk_a) = generate_keypair();
        let (_, pk_b) = generate_keypair();
        let meta = make_metadata();

        let mut ep = BattleEpisode::initialize(vec![pk_a, pk_b], &meta);
        ep.wager_sompi = 5_000_000;
        ep.phase = MatchPhase::WaitingForDeposits {
            a_deposited: false,
            b_deposited: false,
        };

        let rb = ep
            .execute(
                &BattleCommand::ConfirmDeposit {
                    tx_hash: [0u8; 32],
                    amount_sompi: 5_000_000,
                },
                Some(pk_a),
                &meta,
            )
            .unwrap();
        assert_eq!(ep.deposits[0], 5_000_000);

        assert!(ep.rollback(rb));
        assert_eq!(ep.deposits[0], 0);
    }

    #[test]
    fn test_cancel_only_before_lock() {
        let (_, pk_a) = generate_keypair();
        let (_, pk_b) = generate_keypair();
        let meta = make_metadata();

        let mut ep = BattleEpisode::initialize(vec![pk_a, pk_b], &meta);
        assert!(ep
            .execute(
                &BattleCommand::CancelMatch {
                    reason_code: crate::commands::reason::CANCEL_PLAYER_REQUEST
                },
                Some(pk_a),
                &meta,
            )
            .is_ok());

        ep.phase = MatchPhase::Locked;
        assert!(ep
            .execute(
                &BattleCommand::CancelMatch {
                    reason_code: crate::commands::reason::CANCEL_PLAYER_REQUEST
                },
                Some(pk_a),
                &meta,
            )
            .is_err());
    }

    #[test]
    fn test_payout_blocked_in_disputed_state() {
        let (_, pk_a) = generate_keypair();
        let (_, pk_b) = generate_keypair();
        let meta = make_metadata();

        let mut ep = BattleEpisode::initialize(vec![pk_a, pk_b], &meta);
        ep.phase = MatchPhase::Disputed {
            reason_code: 10,
            by_idx: 1,
        };

        assert!(ep
            .execute(&BattleCommand::InitiatePayout, None, &meta)
            .is_err());
    }
}

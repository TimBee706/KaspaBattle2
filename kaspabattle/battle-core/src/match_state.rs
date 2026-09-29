use crate::errors::MatchError;
use serde::{Deserialize, Serialize};

/// Match states — reflects the full lifecycle including FaceIT integration and Dispute.
/// F-009: Added `Disputed` variant to block payouts and enable dispute resolution.
/// F-010: Added FaceIT integration states: GameIdInput, InGame, FinishedFaceit, ReadyForPayout.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum MatchState {
    WaitingForOpponent,
    WaitingForDeposits {
        player_a_deposited: bool,
        player_b_deposited: bool,
    },
    Locked,
    /// F-010: Both deposits confirmed, waiting for both players to enter FaceIT match ID.
    /// Entered automatically from Locked/Funded state (no manual trigger needed).
    GameIdInput {
        faceit_id_a: Option<String>,
        faceit_id_b: Option<String>,
    },
    /// F-010: Both players confirmed the same FaceIT match ID. Watcher is active.
    InGame {
        faceit_match_id: String,
    },
    /// F-010: FaceIT reported the match as finished. Winner has been mapped.
    FinishedFaceit {
        winner_id: String,
        loser_id: String,
        score: String,
    },
    /// NATIVE: both deposits confirmed and the game session exists; waiting for the start.
    ReadyToPlay,
    /// NATIVE: the server-authoritative game is running.
    NativeInGame,
    /// NATIVE: the server engine decided the game (winner known). Payout worker input.
    FinishedGame {
        winner_id: String,
        loser_id: String,
    },
    /// NATIVE: the game ended without a winner (draw) — both stakes are refunded.
    RefundPending,
    /// F-010: PSKT has been created and signed by the oracle. Waiting for winner to sign.
    ReadyForPayout {
        winner_id: String,
        pskt_hex: String,
    },
    Resolved {
        winner_id: String,
    },
    /// F-009: A player has disputed the outcome. No further payouts are allowed
    /// until an admin/DAO resolves or reverts the dispute.
    Disputed {
        reason: String,
        disputed_by: String,
    },
    Cancelled {
        reason: String,
    },
    /// F-REFUND: Deposits have been returned to players on-chain.
    Refunded {
        refund_tx_hash: Option<String>,
    },
    Completed,
}

impl MatchState {
    /// Returns the simple state name as a string for DB storage.
    pub fn state_name(&self) -> &str {
        match self {
            MatchState::WaitingForOpponent => "WaitingForOpponent",
            MatchState::WaitingForDeposits { .. } => "WaitingForDeposits",
            MatchState::Locked => "Locked",
            MatchState::GameIdInput { .. } => "GameIdInput",
            MatchState::InGame { .. } => "InGame",
            MatchState::FinishedFaceit { .. } => "FinishedFaceit",
            MatchState::ReadyToPlay => "ReadyToPlay",
            MatchState::NativeInGame => "NativeInGame",
            MatchState::FinishedGame { .. } => "FinishedGame",
            MatchState::RefundPending => "RefundPending",
            MatchState::ReadyForPayout { .. } => "ReadyForPayout",
            MatchState::Resolved { .. } => "Resolved",
            MatchState::Disputed { .. } => "Disputed",
            MatchState::Cancelled { .. } => "Cancelled",
            MatchState::Refunded { .. } => "Refunded",
            MatchState::Completed => "Completed",
        }
    }

    /// Returns true if payout is allowed in this state.
    /// F-009: Payouts are blocked when a dispute is active.
    pub fn allows_payout(&self) -> bool {
        matches!(self, MatchState::Resolved { .. })
    }
}

/// Actions that can be applied to a match.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MatchAction {
    Join {
        player_id: String,
        kaspa_address: String,
    },
    Cancel {
        player_id: String,
        reason: String,
    },
    DepositConfirmed {
        player_id: String,
        tx_hash: String,
        amount: u64,
    },
    /// F-010: Automatically transitions from Locked/Funded to GameIdInput.
    /// Triggered by the episode runner immediately after both deposits are confirmed.
    TransitionToGameIdInput,
    /// NATIVE: both deposits confirmed → create the game session (Locked → ReadyToPlay).
    PrepareNativeGame,
    /// NATIVE: start the game (ReadyToPlay → NativeInGame). Idempotent while running.
    StartNativeGame,
    /// NATIVE: the engine produced a winner (win, resignation or timeout).
    NativeGameFinished {
        winner_id: String,
        loser_id: String,
    },
    /// NATIVE: the engine reported a draw. No winner payout may ever be created.
    NativeGameDrawn,
    /// F-010: A player submits their FaceIT match ID.
    SubmitFaceitMatchId {
        player_id: String,
        faceit_match_id: String,
    },
    /// F-010: FaceIT Watcher determined the match result.
    FaceitMatchFinished {
        winner_id: String,
        loser_id: String,
        score: String,
    },
    /// F-010: Backend oracle created the PSKT and signed it.
    PsktCreated {
        winner_id: String,
        pskt_hex: String,
    },
    /// F-010: Winner submitted their signature, TX has been broadcast.
    PayoutBroadcast {
        tx_hash: String,
        winner_id: String,
    },
    ResolveWinner {
        winner_id: String,
    },
    /// F-009: A match participant disputes the resolved outcome.
    InitiateDispute {
        player_id: String,
        reason: String,
    },
}

/// Pure state machine transition function.
/// Takes the current state, action, and player_a_id (the match creator)
/// and returns the new state or an error.
/// No I/O, no DB access – perfectly testable.
pub fn transition(
    current_state: &MatchState,
    action: &MatchAction,
    player_a_id: &str,
    player_b_id: Option<&str>,
) -> Result<MatchState, MatchError> {
    match (current_state, action) {
        // === JOIN ===
        (MatchState::WaitingForOpponent, MatchAction::Join { player_id, .. }) => {
            // Player A cannot join their own match
            if player_id == player_a_id {
                return Err(MatchError::SamePlayerCannotJoin);
            }
            Ok(MatchState::WaitingForDeposits {
                player_a_deposited: false,
                player_b_deposited: false,
            })
        }

        // Join on a non-WaitingForOpponent state → MatchFull
        (_, MatchAction::Join { .. }) => Err(MatchError::MatchFull),

        // === CANCEL ===
        (MatchState::WaitingForOpponent, MatchAction::Cancel { player_id, reason }) => {
            // Only the creator can cancel while waiting
            if player_id != player_a_id {
                return Err(MatchError::NotAPlayer);
            }
            Ok(MatchState::Cancelled {
                reason: reason.clone(),
            })
        }

        (MatchState::WaitingForDeposits { .. }, MatchAction::Cancel { player_id, reason }) => {
            // Either player can cancel before lock
            let is_player_a = player_id == player_a_id;
            let is_player_b = player_b_id.map(|b| player_id == b).unwrap_or(false);

            if !is_player_a && !is_player_b {
                return Err(MatchError::NotAPlayer);
            }

            Ok(MatchState::Cancelled {
                reason: reason.clone(),
            })
        }

        // GameIdInput can be cancelled by either player (before game starts)
        (MatchState::GameIdInput { .. }, MatchAction::Cancel { player_id, reason }) => {
            let is_player_a = player_id == player_a_id;
            let is_player_b = player_b_id.map(|b| player_id == b).unwrap_or(false);
            if !is_player_a && !is_player_b {
                return Err(MatchError::NotAPlayer);
            }
            Ok(MatchState::Cancelled { reason: reason.clone() })
        }

        // NATIVE: once the session exists the stakes are locked into the game.
        (
            MatchState::ReadyToPlay
            | MatchState::NativeInGame
            | MatchState::FinishedGame { .. }
            | MatchState::RefundPending,
            MatchAction::Cancel { .. },
        ) => Err(MatchError::CannotCancelLockedMatch),

        // Cancel after Locked → Error
        (MatchState::Locked, MatchAction::Cancel { .. }) => {
            Err(MatchError::CannotCancelLockedMatch)
        }
        (MatchState::InGame { .. }, MatchAction::Cancel { .. }) => {
            Err(MatchError::CannotCancelLockedMatch)
        }
        // Match is finished on FaceIT side — result is final, no cancellation
        (MatchState::FinishedFaceit { .. }, MatchAction::Cancel { .. }) => {
            Err(MatchError::CannotCancelLockedMatch)
        }
        // Payout is pending winner signature — no cancellation allowed
        (MatchState::ReadyForPayout { .. }, MatchAction::Cancel { .. }) => {
            Err(MatchError::CannotCancelLockedMatch)
        }
        (MatchState::Resolved { .. }, MatchAction::Cancel { .. }) => {
            Err(MatchError::CannotCancelLockedMatch)
        }
        (MatchState::Completed, MatchAction::Cancel { .. }) => {
            Err(MatchError::CannotCancelLockedMatch)
        }
        (MatchState::Disputed { .. }, MatchAction::Cancel { .. }) => {
            Err(MatchError::CannotCancelLockedMatch)
        }
        (MatchState::Cancelled { .. }, MatchAction::Cancel { .. }) => {
            Err(MatchError::InvalidTransition {
                from: current_state.state_name().to_string(),
                action: "Cancel".to_string(),
            })
        }
        (MatchState::Refunded { .. }, MatchAction::Cancel { .. }) => {
            Err(MatchError::InvalidTransition {
                from: current_state.state_name().to_string(),
                action: "Cancel".to_string(),
            })
        }

        // === DEPOSIT CONFIRMED ===
        (
            MatchState::WaitingForDeposits {
                player_a_deposited,
                player_b_deposited,
            },
            MatchAction::DepositConfirmed { player_id, .. },
        ) => {
            let is_player_a = player_id == player_a_id;
            let is_player_b = player_b_id.map(|b| player_id == b).unwrap_or(false);

            if !is_player_a && !is_player_b {
                return Err(MatchError::NotAPlayer);
            }

            if is_player_a && *player_a_deposited {
                return Err(MatchError::AlreadyDeposited);
            }
            if is_player_b && *player_b_deposited {
                return Err(MatchError::AlreadyDeposited);
            }

            let new_a = if is_player_a {
                true
            } else {
                *player_a_deposited
            };
            let new_b = if is_player_b {
                true
            } else {
                *player_b_deposited
            };

            if new_a && new_b {
                Ok(MatchState::Locked)
            } else {
                Ok(MatchState::WaitingForDeposits {
                    player_a_deposited: new_a,
                    player_b_deposited: new_b,
                })
            }
        }

        // Deposit in wrong state
        (_, MatchAction::DepositConfirmed { .. }) => Err(MatchError::InvalidTransition {
            from: current_state.state_name().to_string(),
            action: "DepositConfirmed".to_string(),
        }),

        // === TRANSITION TO GAME ID INPUT ===
        // F-010: Episode runner triggers this automatically after both deposits are confirmed.
        (MatchState::Locked, MatchAction::TransitionToGameIdInput) => {
            Ok(MatchState::GameIdInput {
                faceit_id_a: None,
                faceit_id_b: None,
            })
        }

        // Already in GameIdInput — idempotent
        (MatchState::GameIdInput { .. }, MatchAction::TransitionToGameIdInput) => {
            Ok(current_state.clone())
        }

        (_, MatchAction::TransitionToGameIdInput) => Err(MatchError::InvalidTransition {
            from: current_state.state_name().to_string(),
            action: "TransitionToGameIdInput".to_string(),
        }),

        // === NATIVE GAME LIFECYCLE ===
        (MatchState::Locked, MatchAction::PrepareNativeGame) => Ok(MatchState::ReadyToPlay),
        (MatchState::ReadyToPlay, MatchAction::PrepareNativeGame) => Ok(MatchState::ReadyToPlay),
        (_, MatchAction::PrepareNativeGame) => Err(MatchError::InvalidTransition {
            from: current_state.state_name().to_string(),
            action: "PrepareNativeGame".to_string(),
        }),

        (MatchState::ReadyToPlay, MatchAction::StartNativeGame) => Ok(MatchState::NativeInGame),
        (MatchState::NativeInGame, MatchAction::StartNativeGame) => Ok(MatchState::NativeInGame),
        (_, MatchAction::StartNativeGame) => Err(MatchError::InvalidTransition {
            from: current_state.state_name().to_string(),
            action: "StartNativeGame".to_string(),
        }),

        (MatchState::NativeInGame, MatchAction::NativeGameFinished { winner_id, loser_id }) => {
            let is_a = |id: &String| id == player_a_id;
            let is_b = |id: &String| player_b_id.map(|b| id == b).unwrap_or(false);
            // Winner and loser must be the two (distinct) participants.
            let valid = (is_a(winner_id) && is_b(loser_id)) || (is_b(winner_id) && is_a(loser_id));
            if !valid {
                return Err(MatchError::NotAPlayer);
            }
            Ok(MatchState::FinishedGame {
                winner_id: winner_id.clone(),
                loser_id: loser_id.clone(),
            })
        }
        (_, MatchAction::NativeGameFinished { .. }) => Err(MatchError::InvalidTransition {
            from: current_state.state_name().to_string(),
            action: "NativeGameFinished".to_string(),
        }),

        (MatchState::NativeInGame, MatchAction::NativeGameDrawn) => Ok(MatchState::RefundPending),
        (_, MatchAction::NativeGameDrawn) => Err(MatchError::InvalidTransition {
            from: current_state.state_name().to_string(),
            action: "NativeGameDrawn".to_string(),
        }),

        // === SUBMIT FACEIT MATCH ID ===
        // F-010: A player submits their FaceIT match ID.
        (
            MatchState::GameIdInput { faceit_id_a, faceit_id_b },
            MatchAction::SubmitFaceitMatchId { player_id, faceit_match_id },
        ) => {
            let is_player_a = player_id == player_a_id;
            let is_player_b = player_b_id.map(|b| player_id == b).unwrap_or(false);

            if !is_player_a && !is_player_b {
                return Err(MatchError::NotAPlayer);
            }

            let new_id_a = if is_player_a {
                Some(faceit_match_id.clone())
            } else {
                faceit_id_a.clone()
            };
            let new_id_b = if is_player_b {
                Some(faceit_match_id.clone())
            } else {
                faceit_id_b.clone()
            };

            // Check if both are submitted and whether they match
            match (&new_id_a, &new_id_b) {
                (Some(id_a), Some(id_b)) => {
                    if id_a == id_b {
                        Ok(MatchState::InGame {
                            faceit_match_id: faceit_match_id.clone(),
                        })
                    } else {
                        // IDs mismatch — store both but remain in GameIdInput
                        // The handler layer should return a 409 and inform the user
                        Err(MatchError::FaceitMatchIdMismatch {
                            id_a: id_a.clone(),
                            id_b: id_b.clone(),
                        })
                    }
                }
                // Only one player has submitted so far
                _ => Ok(MatchState::GameIdInput {
                    faceit_id_a: new_id_a,
                    faceit_id_b: new_id_b,
                }),
            }
        }

        (_, MatchAction::SubmitFaceitMatchId { .. }) => Err(MatchError::InvalidTransition {
            from: current_state.state_name().to_string(),
            action: "SubmitFaceitMatchId".to_string(),
        }),

        // === FACEIT MATCH FINISHED ===
        // F-010: FaceIT Watcher reports match result.
        (MatchState::InGame { .. }, MatchAction::FaceitMatchFinished { winner_id, loser_id, score }) => {
            Ok(MatchState::FinishedFaceit {
                winner_id: winner_id.clone(),
                loser_id: loser_id.clone(),
                score: score.clone(),
            })
        }

        (_, MatchAction::FaceitMatchFinished { .. }) => Err(MatchError::InvalidTransition {
            from: current_state.state_name().to_string(),
            action: "FaceitMatchFinished".to_string(),
        }),

        // === PSKT CREATED ===
        // F-010: Backend created the PSKT after FaceIT confirmed the winner.
        (MatchState::FinishedFaceit { winner_id, .. }, MatchAction::PsktCreated { winner_id: pskt_winner, pskt_hex }) => {
            if winner_id != pskt_winner {
                return Err(MatchError::InvalidTransition {
                    from: "FinishedFaceit".to_string(),
                    action: "PsktCreated (winner mismatch)".to_string(),
                });
            }
            Ok(MatchState::ReadyForPayout {
                winner_id: winner_id.clone(),
                pskt_hex: pskt_hex.clone(),
            })
        }

        // NATIVE: the payout worker creates the PSKT from FinishedGame.
        (MatchState::FinishedGame { winner_id, .. }, MatchAction::PsktCreated { winner_id: pskt_winner, pskt_hex }) => {
            if winner_id != pskt_winner {
                return Err(MatchError::InvalidTransition {
                    from: "FinishedGame".to_string(),
                    action: "PsktCreated (winner mismatch)".to_string(),
                });
            }
            Ok(MatchState::ReadyForPayout {
                winner_id: winner_id.clone(),
                pskt_hex: pskt_hex.clone(),
            })
        }

        (_, MatchAction::PsktCreated { .. }) => Err(MatchError::InvalidTransition {
            from: current_state.state_name().to_string(),
            action: "PsktCreated".to_string(),
        }),

        // === PAYOUT BROADCAST ===
        // F-010: Winner signed and TX was broadcast.
        (MatchState::ReadyForPayout { winner_id, .. }, MatchAction::PayoutBroadcast { winner_id: broadcast_winner, .. }) => {
            if winner_id != broadcast_winner {
                return Err(MatchError::InvalidTransition {
                    from: "ReadyForPayout".to_string(),
                    action: "PayoutBroadcast (winner mismatch)".to_string(),
                });
            }
            Ok(MatchState::Resolved {
                winner_id: winner_id.clone(),
            })
        }

        (_, MatchAction::PayoutBroadcast { .. }) => Err(MatchError::InvalidTransition {
            from: current_state.state_name().to_string(),
            action: "PayoutBroadcast".to_string(),
        }),

        // === RESOLVE WINNER ===
        // F-009: Resolve is blocked if match is already Disputed.
        (MatchState::Disputed { .. }, MatchAction::ResolveWinner { .. }) => {
            Err(MatchError::InvalidTransition {
                from: "Disputed".to_string(),
                action: "ResolveWinner".to_string(),
            })
        }

        (MatchState::Locked, MatchAction::ResolveWinner { winner_id }) => {
            Ok(MatchState::Resolved {
                winner_id: winner_id.clone(),
            })
        }

        // Resolve in wrong state
        (_, MatchAction::ResolveWinner { .. }) => Err(MatchError::InvalidTransition {
            from: current_state.state_name().to_string(),
            action: "ResolveWinner".to_string(),
        }),

        // === INITIATE DISPUTE (F-009) ===
        // Only valid from Resolved state, by a match participant.
        (MatchState::Resolved { .. }, MatchAction::InitiateDispute { player_id, reason }) => {
            let is_player_a = player_id == player_a_id;
            let is_player_b = player_b_id.map(|b| player_id == b).unwrap_or(false);

            if !is_player_a && !is_player_b {
                return Err(MatchError::NotAPlayer);
            }

            Ok(MatchState::Disputed {
                reason: reason.clone(),
                disputed_by: player_id.clone(),
            })
        }

        // Dispute in invalid states
        (_, MatchAction::InitiateDispute { .. }) => Err(MatchError::InvalidTransition {
            from: current_state.state_name().to_string(),
            action: "InitiateDispute".to_string(),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Existing Tests (preserved) ───────────────────────────────────────────

    #[test]
    fn test_join_match_success() {
        let state = MatchState::WaitingForOpponent;
        let action = MatchAction::Join {
            player_id: "bob".to_string(),
            kaspa_address: "kaspatest:qbob".to_string(),
        };

        let result = transition(&state, &action, "alice", None);
        assert!(result.is_ok());

        let new_state = result.unwrap();
        assert_eq!(
            new_state,
            MatchState::WaitingForDeposits {
                player_a_deposited: false,
                player_b_deposited: false,
            }
        );
    }

    #[test]
    fn test_same_player_cannot_join() {
        let state = MatchState::WaitingForOpponent;
        let action = MatchAction::Join {
            player_id: "alice".to_string(),
            kaspa_address: "kaspatest:qalice".to_string(),
        };

        let result = transition(&state, &action, "alice", None);
        assert!(result.is_err());

        match result.unwrap_err() {
            MatchError::SamePlayerCannotJoin => {}
            other => panic!("Expected SamePlayerCannotJoin, got {:?}", other),
        }
    }

    #[test]
    fn test_join_full_match_rejected() {
        let state = MatchState::WaitingForDeposits {
            player_a_deposited: false,
            player_b_deposited: false,
        };
        let action = MatchAction::Join {
            player_id: "charlie".to_string(),
            kaspa_address: "kaspatest:qcharlie".to_string(),
        };

        let result = transition(&state, &action, "alice", Some("bob"));
        assert!(result.is_err());

        match result.unwrap_err() {
            MatchError::MatchFull => {}
            other => panic!("Expected MatchFull, got {:?}", other),
        }
    }

    #[test]
    fn test_cancel_before_lock() {
        let state = MatchState::WaitingForOpponent;
        let action = MatchAction::Cancel {
            player_id: "alice".to_string(),
            reason: "Changed my mind".to_string(),
        };

        let result = transition(&state, &action, "alice", None);
        assert!(result.is_ok());

        match result.unwrap() {
            MatchState::Cancelled { reason } => {
                assert_eq!(reason, "Changed my mind");
            }
            other => panic!("Expected Cancelled, got {:?}", other),
        }

        let state = MatchState::WaitingForDeposits {
            player_a_deposited: false,
            player_b_deposited: false,
        };
        let action = MatchAction::Cancel {
            player_id: "bob".to_string(),
            reason: "No time".to_string(),
        };

        let result = transition(&state, &action, "alice", Some("bob"));
        assert!(result.is_ok());
    }

    #[test]
    fn test_cancel_after_lock_fails() {
        let state = MatchState::Locked;
        let action = MatchAction::Cancel {
            player_id: "alice".to_string(),
            reason: "Want to cancel".to_string(),
        };

        let result = transition(&state, &action, "alice", Some("bob"));
        assert!(result.is_err());

        match result.unwrap_err() {
            MatchError::CannotCancelLockedMatch => {}
            other => panic!("Expected CannotCancelLockedMatch, got {:?}", other),
        }
    }

    #[test]
    fn test_stranger_cannot_cancel() {
        let state = MatchState::WaitingForOpponent;
        let action = MatchAction::Cancel {
            player_id: "stranger".to_string(),
            reason: "Trolling".to_string(),
        };

        let result = transition(&state, &action, "alice", None);
        assert!(result.is_err());

        match result.unwrap_err() {
            MatchError::NotAPlayer => {}
            other => panic!("Expected NotAPlayer, got {:?}", other),
        }
    }

    // ── F-009 Dispute Tests ──────────────────────────────────────────────────

    #[test]
    fn test_initiate_dispute_from_resolved_by_player_a() {
        let state = MatchState::Resolved {
            winner_id: "bob".to_string(),
        };
        let action = MatchAction::InitiateDispute {
            player_id: "alice".to_string(),
            reason: "I won the match!".to_string(),
        };

        let result = transition(&state, &action, "alice", Some("bob"));
        assert!(
            result.is_ok(),
            "Player A should be able to dispute a Resolved match"
        );

        match result.unwrap() {
            MatchState::Disputed {
                reason,
                disputed_by,
            } => {
                assert_eq!(reason, "I won the match!");
                assert_eq!(disputed_by, "alice");
            }
            other => panic!("Expected Disputed, got {:?}", other),
        }
    }

    #[test]
    fn test_initiate_dispute_from_resolved_by_player_b() {
        let state = MatchState::Resolved {
            winner_id: "alice".to_string(),
        };
        let action = MatchAction::InitiateDispute {
            player_id: "bob".to_string(),
            reason: "Score was wrong".to_string(),
        };

        let result = transition(&state, &action, "alice", Some("bob"));
        assert!(result.is_ok());

        match result.unwrap() {
            MatchState::Disputed { disputed_by, .. } => {
                assert_eq!(disputed_by, "bob");
            }
            other => panic!("Expected Disputed, got {:?}", other),
        }
    }

    #[test]
    fn test_stranger_cannot_dispute() {
        let state = MatchState::Resolved {
            winner_id: "alice".to_string(),
        };
        let action = MatchAction::InitiateDispute {
            player_id: "stranger".to_string(),
            reason: "Just trolling".to_string(),
        };

        let result = transition(&state, &action, "alice", Some("bob"));
        assert!(result.is_err());
        match result.unwrap_err() {
            MatchError::NotAPlayer => {}
            other => panic!("Expected NotAPlayer, got {:?}", other),
        }
    }

    #[test]
    fn test_dispute_from_locked_is_invalid() {
        let state = MatchState::Locked;
        let action = MatchAction::InitiateDispute {
            player_id: "alice".to_string(),
            reason: "Pre-emptive".to_string(),
        };

        let result = transition(&state, &action, "alice", Some("bob"));
        assert!(result.is_err());
        match result.unwrap_err() {
            MatchError::InvalidTransition { .. } => {}
            other => panic!("Expected InvalidTransition, got {:?}", other),
        }
    }

    #[test]
    fn test_resolve_is_blocked_when_disputed() {
        let state = MatchState::Disputed {
            reason: "Some dispute".to_string(),
            disputed_by: "alice".to_string(),
        };
        let action = MatchAction::ResolveWinner {
            winner_id: "alice".to_string(),
        };

        let result = transition(&state, &action, "alice", Some("bob"));
        assert!(result.is_err(), "Resolve should be blocked when Disputed");
        match result.unwrap_err() {
            MatchError::InvalidTransition { .. } => {}
            other => panic!("Expected InvalidTransition, got {:?}", other),
        }
    }

    #[test]
    fn test_allows_payout_flag() {
        assert!(!MatchState::Locked.allows_payout());
        assert!(MatchState::Resolved {
            winner_id: "alice".to_string()
        }
        .allows_payout());
        assert!(!MatchState::Disputed {
            reason: "dispute".to_string(),
            disputed_by: "alice".to_string()
        }
        .allows_payout());
        assert!(!MatchState::Cancelled {
            reason: "test".to_string()
        }
        .allows_payout());
    }

    // ── F-010 FaceIT Integration Tests ───────────────────────────────────────

    #[test]
    fn test_transition_locked_to_game_id_input() {
        let state = MatchState::Locked;
        let result = transition(&state, &MatchAction::TransitionToGameIdInput, "alice", Some("bob"));
        assert!(result.is_ok());
        match result.unwrap() {
            MatchState::GameIdInput { faceit_id_a, faceit_id_b } => {
                assert!(faceit_id_a.is_none());
                assert!(faceit_id_b.is_none());
            }
            other => panic!("Expected GameIdInput, got {:?}", other),
        }
    }

    #[test]
    fn test_transition_to_game_id_input_idempotent() {
        let state = MatchState::GameIdInput {
            faceit_id_a: Some("abc".to_string()),
            faceit_id_b: None,
        };
        let result = transition(&state, &MatchAction::TransitionToGameIdInput, "alice", Some("bob"));
        assert!(result.is_ok());
        // Should stay in GameIdInput unchanged
        match result.unwrap() {
            MatchState::GameIdInput { .. } => {}
            other => panic!("Expected GameIdInput, got {:?}", other),
        }
    }

    #[test]
    fn test_transition_to_game_id_input_from_wrong_state_fails() {
        let state = MatchState::WaitingForOpponent;
        let result = transition(&state, &MatchAction::TransitionToGameIdInput, "alice", Some("bob"));
        assert!(result.is_err());
        match result.unwrap_err() {
            MatchError::InvalidTransition { .. } => {}
            other => panic!("Expected InvalidTransition, got {:?}", other),
        }
    }

    #[test]
    fn test_submit_faceit_id_player_a_first() {
        let state = MatchState::GameIdInput {
            faceit_id_a: None,
            faceit_id_b: None,
        };
        let action = MatchAction::SubmitFaceitMatchId {
            player_id: "alice".to_string(),
            faceit_match_id: "match-uuid-123".to_string(),
        };
        let result = transition(&state, &action, "alice", Some("bob"));
        assert!(result.is_ok());
        match result.unwrap() {
            MatchState::GameIdInput { faceit_id_a, faceit_id_b } => {
                assert_eq!(faceit_id_a, Some("match-uuid-123".to_string()));
                assert!(faceit_id_b.is_none());
            }
            other => panic!("Expected GameIdInput, got {:?}", other),
        }
    }

    #[test]
    fn test_submit_faceit_id_both_same_triggers_in_game() {
        let state = MatchState::GameIdInput {
            faceit_id_a: Some("match-uuid-123".to_string()),
            faceit_id_b: None,
        };
        let action = MatchAction::SubmitFaceitMatchId {
            player_id: "bob".to_string(),
            faceit_match_id: "match-uuid-123".to_string(),
        };
        let result = transition(&state, &action, "alice", Some("bob"));
        assert!(result.is_ok());
        match result.unwrap() {
            MatchState::InGame { faceit_match_id } => {
                assert_eq!(faceit_match_id, "match-uuid-123");
            }
            other => panic!("Expected InGame, got {:?}", other),
        }
    }

    #[test]
    fn test_submit_faceit_id_mismatch_returns_error() {
        let state = MatchState::GameIdInput {
            faceit_id_a: Some("match-uuid-AAA".to_string()),
            faceit_id_b: None,
        };
        let action = MatchAction::SubmitFaceitMatchId {
            player_id: "bob".to_string(),
            faceit_match_id: "match-uuid-BBB".to_string(),
        };
        let result = transition(&state, &action, "alice", Some("bob"));
        assert!(result.is_err());
        match result.unwrap_err() {
            MatchError::FaceitMatchIdMismatch { id_a, id_b } => {
                assert_eq!(id_a, "match-uuid-AAA");
                assert_eq!(id_b, "match-uuid-BBB");
            }
            other => panic!("Expected FaceitMatchIdMismatch, got {:?}", other),
        }
    }

    #[test]
    fn test_submit_faceit_id_stranger_fails() {
        let state = MatchState::GameIdInput {
            faceit_id_a: None,
            faceit_id_b: None,
        };
        let action = MatchAction::SubmitFaceitMatchId {
            player_id: "stranger".to_string(),
            faceit_match_id: "match-uuid-123".to_string(),
        };
        let result = transition(&state, &action, "alice", Some("bob"));
        assert!(result.is_err());
        match result.unwrap_err() {
            MatchError::NotAPlayer => {}
            other => panic!("Expected NotAPlayer, got {:?}", other),
        }
    }

    #[test]
    fn test_submit_faceit_id_in_wrong_state_fails() {
        let state = MatchState::Locked;
        let action = MatchAction::SubmitFaceitMatchId {
            player_id: "alice".to_string(),
            faceit_match_id: "match-uuid-123".to_string(),
        };
        let result = transition(&state, &action, "alice", Some("bob"));
        assert!(result.is_err());
        match result.unwrap_err() {
            MatchError::InvalidTransition { .. } => {}
            other => panic!("Expected InvalidTransition, got {:?}", other),
        }
    }

    #[test]
    fn test_faceit_match_finished_from_in_game() {
        let state = MatchState::InGame {
            faceit_match_id: "match-uuid-123".to_string(),
        };
        let action = MatchAction::FaceitMatchFinished {
            winner_id: "alice".to_string(),
            loser_id: "bob".to_string(),
            score: "16:10".to_string(),
        };
        let result = transition(&state, &action, "alice", Some("bob"));
        assert!(result.is_ok());
        match result.unwrap() {
            MatchState::FinishedFaceit { winner_id, score, .. } => {
                assert_eq!(winner_id, "alice");
                assert_eq!(score, "16:10");
            }
            other => panic!("Expected FinishedFaceit, got {:?}", other),
        }
    }

    #[test]
    fn test_faceit_match_finished_from_wrong_state_fails() {
        let state = MatchState::Locked;
        let action = MatchAction::FaceitMatchFinished {
            winner_id: "alice".to_string(),
            loser_id: "bob".to_string(),
            score: "16:10".to_string(),
        };
        let result = transition(&state, &action, "alice", Some("bob"));
        assert!(result.is_err());
        match result.unwrap_err() {
            MatchError::InvalidTransition { .. } => {}
            other => panic!("Expected InvalidTransition, got {:?}", other),
        }
    }

    #[test]
    fn test_pskt_created_from_finished_faceit() {
        let state = MatchState::FinishedFaceit {
            winner_id: "alice".to_string(),
            loser_id: "bob".to_string(),
            score: "16:10".to_string(),
        };
        let action = MatchAction::PsktCreated {
            winner_id: "alice".to_string(),
            pskt_hex: "deadbeef".to_string(),
        };
        let result = transition(&state, &action, "alice", Some("bob"));
        assert!(result.is_ok());
        match result.unwrap() {
            MatchState::ReadyForPayout { winner_id, pskt_hex } => {
                assert_eq!(winner_id, "alice");
                assert_eq!(pskt_hex, "deadbeef");
            }
            other => panic!("Expected ReadyForPayout, got {:?}", other),
        }
    }

    #[test]
    fn test_pskt_created_winner_mismatch_fails() {
        let state = MatchState::FinishedFaceit {
            winner_id: "alice".to_string(),
            loser_id: "bob".to_string(),
            score: "16:10".to_string(),
        };
        let action = MatchAction::PsktCreated {
            winner_id: "bob".to_string(), // WRONG: alice won
            pskt_hex: "deadbeef".to_string(),
        };
        let result = transition(&state, &action, "alice", Some("bob"));
        assert!(result.is_err());
        match result.unwrap_err() {
            MatchError::InvalidTransition { .. } => {}
            other => panic!("Expected InvalidTransition, got {:?}", other),
        }
    }

    #[test]
    fn test_payout_broadcast_from_ready_for_payout() {
        let state = MatchState::ReadyForPayout {
            winner_id: "alice".to_string(),
            pskt_hex: "deadbeef".to_string(),
        };
        let action = MatchAction::PayoutBroadcast {
            winner_id: "alice".to_string(),
            tx_hash: "on-chain-tx-hash".to_string(),
        };
        let result = transition(&state, &action, "alice", Some("bob"));
        assert!(result.is_ok());
        match result.unwrap() {
            MatchState::Resolved { winner_id } => {
                assert_eq!(winner_id, "alice");
            }
            other => panic!("Expected Resolved, got {:?}", other),
        }
    }

    #[test]
    fn test_payout_broadcast_wrong_winner_fails() {
        let state = MatchState::ReadyForPayout {
            winner_id: "alice".to_string(),
            pskt_hex: "deadbeef".to_string(),
        };
        let action = MatchAction::PayoutBroadcast {
            winner_id: "bob".to_string(), // WRONG
            tx_hash: "on-chain-tx-hash".to_string(),
        };
        let result = transition(&state, &action, "alice", Some("bob"));
        assert!(result.is_err());
        match result.unwrap_err() {
            MatchError::InvalidTransition { .. } => {}
            other => panic!("Expected InvalidTransition, got {:?}", other),
        }
    }

    #[test]
    fn test_cancel_in_game_id_input_by_player_a() {
        let state = MatchState::GameIdInput {
            faceit_id_a: None,
            faceit_id_b: None,
        };
        let action = MatchAction::Cancel {
            player_id: "alice".to_string(),
            reason: "Taking too long".to_string(),
        };
        let result = transition(&state, &action, "alice", Some("bob"));
        assert!(result.is_ok());
        match result.unwrap() {
            MatchState::Cancelled { reason } => assert_eq!(reason, "Taking too long"),
            other => panic!("Expected Cancelled, got {:?}", other),
        }
    }

    #[test]
    fn test_cancel_in_game_fails() {
        let state = MatchState::InGame {
            faceit_match_id: "match-uuid-123".to_string(),
        };
        let action = MatchAction::Cancel {
            player_id: "alice".to_string(),
            reason: "Rage quit".to_string(),
        };
        let result = transition(&state, &action, "alice", Some("bob"));
        assert!(result.is_err());
        match result.unwrap_err() {
            MatchError::CannotCancelLockedMatch => {}
            other => panic!("Expected CannotCancelLockedMatch, got {:?}", other),
        }
    }

    // ── NATIVE lifecycle ─────────────────────────────────────────────────────

    fn t(state: &MatchState, action: MatchAction) -> Result<MatchState, MatchError> {
        transition(state, &action, "alice", Some("bob"))
    }

    #[test]
    fn native_happy_path_to_ready_for_payout() {
        let s = t(&MatchState::Locked, MatchAction::PrepareNativeGame).unwrap();
        assert_eq!(s, MatchState::ReadyToPlay);
        let s = t(&s, MatchAction::StartNativeGame).unwrap();
        assert_eq!(s, MatchState::NativeInGame);
        // Idempotent start.
        assert_eq!(t(&s, MatchAction::StartNativeGame).unwrap(), MatchState::NativeInGame);
        let s = t(&s, MatchAction::NativeGameFinished { winner_id: "bob".into(), loser_id: "alice".into() }).unwrap();
        assert_eq!(s, MatchState::FinishedGame { winner_id: "bob".into(), loser_id: "alice".into() });
        let s = t(&s, MatchAction::PsktCreated { winner_id: "bob".into(), pskt_hex: "aa".into() }).unwrap();
        assert!(matches!(s, MatchState::ReadyForPayout { .. }));
    }

    #[test]
    fn native_draw_leads_to_refund_and_never_to_payout() {
        let s = t(&MatchState::NativeInGame, MatchAction::NativeGameDrawn).unwrap();
        assert_eq!(s, MatchState::RefundPending);
        assert!(!s.allows_payout());
        // No PSKT / winner can be attached to a refund-pending match.
        assert!(t(&s, MatchAction::PsktCreated { winner_id: "alice".into(), pskt_hex: "aa".into() }).is_err());
        assert!(t(&s, MatchAction::NativeGameFinished { winner_id: "alice".into(), loser_id: "bob".into() }).is_err());
    }

    #[test]
    fn native_finish_requires_the_two_participants() {
        let bad = t(&MatchState::NativeInGame, MatchAction::NativeGameFinished { winner_id: "mallory".into(), loser_id: "alice".into() });
        assert!(matches!(bad, Err(MatchError::NotAPlayer)));
        let same = t(&MatchState::NativeInGame, MatchAction::NativeGameFinished { winner_id: "alice".into(), loser_id: "alice".into() });
        assert!(matches!(same, Err(MatchError::NotAPlayer)));
    }

    #[test]
    fn native_states_cannot_be_cancelled_or_skipped() {
        for st in [MatchState::ReadyToPlay, MatchState::NativeInGame, MatchState::RefundPending,
                   MatchState::FinishedGame { winner_id: "alice".into(), loser_id: "bob".into() }] {
            let r = t(&st, MatchAction::Cancel { player_id: "alice".into(), reason: "x".into() });
            assert!(matches!(r, Err(MatchError::CannotCancelLockedMatch)), "{:?}", st);
        }
        // Cannot start before funding completed, nor finish without a running game.
        assert!(t(&MatchState::WaitingForOpponent, MatchAction::StartNativeGame).is_err());
        assert!(t(&MatchState::ReadyToPlay, MatchAction::NativeGameFinished { winner_id: "alice".into(), loser_id: "bob".into() }).is_err());
        assert!(t(&MatchState::ReadyToPlay, MatchAction::NativeGameDrawn).is_err());
    }
}

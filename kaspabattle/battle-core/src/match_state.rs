use crate::errors::MatchError;
use serde::{Deserialize, Serialize};

/// Match states — reflects the full lifecycle including Dispute.
/// F-009: Added `Disputed` variant to block payouts and enable dispute resolution.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum MatchState {
    WaitingForOpponent,
    WaitingForDeposits {
        player_a_deposited: bool,
        player_b_deposited: bool,
    },
    Locked,
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
    Completed,
}

impl MatchState {
    /// Returns the simple state name as a string for DB storage.
    pub fn state_name(&self) -> &str {
        match self {
            MatchState::WaitingForOpponent => "WaitingForOpponent",
            MatchState::WaitingForDeposits { .. } => "WaitingForDeposits",
            MatchState::Locked => "Locked",
            MatchState::Resolved { .. } => "Resolved",
            MatchState::Disputed { .. } => "Disputed",
            MatchState::Cancelled { .. } => "Cancelled",
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

        // Cancel after Locked → Error
        (MatchState::Locked, MatchAction::Cancel { .. }) => {
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

    // === F-009 Dispute Tests ===

    #[test]
    fn test_initiate_dispute_from_resolved_by_player_a() {
        // Scenario: Oracle resolved with player_b but player_a disputes
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
        // Can only dispute after Resolved, not from Locked
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
        // F-009: Once disputed, can't resolve again
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
}

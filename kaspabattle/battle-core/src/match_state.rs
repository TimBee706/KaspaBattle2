use crate::errors::MatchError;
use serde::{Deserialize, Serialize};

/// Match states
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
    Cancelled {
        reason: String,
    },
    Completed,
}

impl MatchState {
    /// Returns the simple state name as a string for DB storage
    pub fn state_name(&self) -> &str {
        match self {
            MatchState::WaitingForOpponent => "WaitingForOpponent",
            MatchState::WaitingForDeposits { .. } => "WaitingForDeposits",
            MatchState::Locked => "Locked",
            MatchState::Resolved { .. } => "Resolved",
            MatchState::Cancelled { .. } => "Cancelled",
            MatchState::Completed => "Completed",
        }
    }
}

/// Actions that can be applied to a match
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
            MatchError::SamePlayerCannotJoin => {} // expected
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
            MatchError::MatchFull => {} // expected
            other => panic!("Expected MatchFull, got {:?}", other),
        }
    }

    #[test]
    fn test_cancel_before_lock() {
        // Cancel from WaitingForOpponent
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

        // Cancel from WaitingForDeposits
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

        match result.unwrap() {
            MatchState::Cancelled { reason } => {
                assert_eq!(reason, "No time");
            }
            other => panic!("Expected Cancelled, got {:?}", other),
        }
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
            MatchError::CannotCancelLockedMatch => {} // expected
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
            MatchError::NotAPlayer => {} // expected
            other => panic!("Expected NotAPlayer, got {:?}", other),
        }

        // Also test with WaitingForDeposits
        let state = MatchState::WaitingForDeposits {
            player_a_deposited: false,
            player_b_deposited: false,
        };
        let action = MatchAction::Cancel {
            player_id: "stranger".to_string(),
            reason: "Trolling".to_string(),
        };

        let result = transition(&state, &action, "alice", Some("bob"));
        assert!(result.is_err());

        match result.unwrap_err() {
            MatchError::NotAPlayer => {} // expected
            other => panic!("Expected NotAPlayer, got {:?}", other),
        }
    }
}

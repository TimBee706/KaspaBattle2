use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MatchError {
    MatchFull,
    MatchNotFound,
    NotAPlayer,
    SamePlayerCannotJoin,
    InvalidTransition { from: String, action: String },
    InvalidWagerAmount { min: u64, max: u64, provided: u64 },
    InvalidGameType(String),
    AlreadyDeposited,
    DepositAmountMismatch { expected: u64, received: u64 },
    MatchAlreadyResolved,
    CannotCancelLockedMatch,
    InternalError(String),
    /// F-010: Both players submitted FaceIT match IDs but they don't match.
    FaceitMatchIdMismatch { id_a: String, id_b: String },
}

impl fmt::Display for MatchError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            MatchError::MatchFull => write!(f, "Match is full"),
            MatchError::MatchNotFound => write!(f, "Match not found"),
            MatchError::NotAPlayer => write!(f, "Not a participant of this match"),
            MatchError::SamePlayerCannotJoin => write!(f, "Cannot join your own match"),
            MatchError::InvalidTransition { from, action } => {
                write!(f, "Invalid transition from {} with action {}", from, action)
            }
            MatchError::InvalidWagerAmount { min, max, provided } => {
                write!(f, "Wager {} KAS not in range [{}, {}]", provided, min, max)
            }
            MatchError::InvalidGameType(g) => write!(f, "Unknown game type: {}", g),
            MatchError::AlreadyDeposited => write!(f, "Already deposited"),
            MatchError::DepositAmountMismatch { expected, received } => {
                write!(f, "Expected {} sompi, received {}", expected, received)
            }
            MatchError::MatchAlreadyResolved => write!(f, "Match already resolved"),
            MatchError::CannotCancelLockedMatch => write!(f, "Cannot cancel a locked match"),
            MatchError::InternalError(e) => write!(f, "Internal error: {}", e),
            MatchError::FaceitMatchIdMismatch { id_a, id_b } => {
                write!(f, "FaceIT match ID mismatch: player A entered '{}', player B entered '{}'", id_a, id_b)
            }
        }
    }
}

impl std::error::Error for MatchError {}

#[derive(thiserror::Error, Debug)]
pub enum OracleError {
    #[error("Network Error: {0}")]
    NetworkError(#[from] reqwest::Error),
    #[error("API Error - Status: {status}, Message: {message}")]
    ApiError { status: u16, message: String },
    #[error("Parse Error: {0}")]
    ParseError(String),
    #[error("Signature Error: {0}")]
    SignatureError(String),
    #[error("Confirmation Mismatch: First ({first}) vs Second ({second})")]
    ConfirmationMismatch { first: String, second: String },
}

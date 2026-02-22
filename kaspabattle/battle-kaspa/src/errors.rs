use thiserror::Error;

#[derive(Error, Debug)]
pub enum PayoutError {
    #[error("Already paid out")]
    AlreadyPaidOut,
    #[error("Match not resolved. Current status: {current_status}")]
    MatchNotResolved { current_status: String },
    #[error("Insufficient escrow balance. Expected: {expected}, Found: {found}")]
    InsufficientEscrowBalance { expected: u64, found: u64 },
    #[error("Kaspa RPC Error: {0}")]
    KaspaRpcError(#[from] crate::rpc::KaspaError),
    #[error("TX Build Error: {0}")]
    TxBuildError(String),
    // We will use sqlx::Error later, currently db is abstracted
    #[error("Database Error: {0}")]
    DatabaseError(String),
}

#[derive(Error, Debug)]
pub enum EscrowError {
    #[error("Invalid Public Key: {0}")]
    InvalidPublicKey(String),
    #[error("Derivation Failed: {0}")]
    DerivationFailed(String),
}

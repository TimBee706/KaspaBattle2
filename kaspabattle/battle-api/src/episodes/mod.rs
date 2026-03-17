use async_trait::async_trait;
use std::sync::Arc;
use uuid::Uuid;

/// Which player role is performing an action
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayerRole {
    A,
    B,
}

/// kdapp-inspired Episode trait for the full match lifecycle.
///
/// Each match is an "Episode" with a state machine:
/// Open → AwaitingFunding → Funded → Locked → InGame → Resolving → Resolved → PaidOut
#[async_trait]
#[allow(dead_code)]
pub trait EpisodeTrait: Sized {
    type Context;
    type Error;

    /// Create the episode — sets up match state, derives escrow address
    async fn initialize(ctx: &Self::Context, id: Uuid) -> Result<Self, Self::Error>;

    /// Advance the state machine:
    /// - AwaitingFunding → check on-chain deposits → Funded
    /// - Funded → lock escrow, create FACEIT match → Locked
    async fn execute(&mut self) -> Result<(), Self::Error>;

    /// Revert — cancel match, optionally trigger refund
    async fn rollback(&mut self) -> Result<(), Self::Error>;

    /// Returns true when episode has reached a terminal state
    async fn poll(&mut self) -> Result<bool, Self::Error>;

    // ── v0.2 Extensions ──

    /// Record a player's deposit TX hash (called after frontend broadcasts TX)
    async fn record_deposit(
        &mut self,
        player: PlayerRole,
        tx_hash: &str,
    ) -> Result<(), Self::Error>;

    /// Check on-chain deposit confirmations and advance state if complete
    async fn confirm_deposits(&mut self) -> Result<DepositCheckResult, Self::Error>;

    /// Submit optional FaceID hash (off-chain, anti-fraud)
    async fn submit_faceid(&mut self, player: PlayerRole, hash: &str) -> Result<(), Self::Error>;
}

/// Result of an on-chain deposit confirmation check
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DepositCheckResult {
    /// Neither player has deposited
    None,
    /// Only one player has deposited
    Partial { balance_sompi: u64 },
    /// Both players deposited, match advanced to Funded
    Complete,
}

pub mod match_episode;

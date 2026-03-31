use async_trait::async_trait;
use uuid::Uuid;

/// Which player role is performing an action
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayerRole {
    A,
    B,
}

/// KDAPP-inspired Episode trait for match lifecycle.
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
}

pub mod match_episode;

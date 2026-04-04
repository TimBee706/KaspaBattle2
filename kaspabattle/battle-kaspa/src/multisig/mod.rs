//! Multisig Escrow Module for KaspaBattle
//!
//! Provides 2-of-3 (or m-of-n) P2SH multisig escrow functionality on Kaspa L1.
//!
//! # Architecture
//!
//! - **types**: Core data structures (MultisigEscrow, EscrowStatus, SignatureBundle)
//! - **scripts**: Redeem script construction and P2SH address derivation
//! - **transaction**: Unsigned TX building, sighash computation, signature assembly
//! - **service**: High-level lifecycle management (create → fund → settle/refund)

pub mod scripts;
pub mod service;
pub mod transaction;
pub mod types;

pub use scripts::*;
pub use service::MultisigEscrowService;
pub use types::*;

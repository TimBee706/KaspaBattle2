//! KaspaBattle SilverScript support crate.
//!
//! Currently scoped to the `KASPABATTLE_RESULT_V1` result attestation
//! (see `docs/SILVERSCRIPT-INTEGRATION-PLAN.md`, section 8) — the canonical
//! off-chain-signed message an authorized Oracle produces, which the
//! `MatchEscrow` SilverScript contract's `oracle_settle` entry verifies
//! on-chain via `checkMsgSig`/`OpCheckSigFromStack`.
//!
//! Does not depend on `kaspa-txscript`/`kaspa-consensus-core`: see
//! `Cargo.toml` for why (version conflict with this workspace's existing
//! rusty-kaspa 0.15 tree). The on-chain contract artifact/builder pieces need
//! that pinned tree and live in a separate standalone Cargo workspace.

pub mod attestation;

pub use attestation::{Attestation, AttestationError, WinnerSelector};

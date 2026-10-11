// CI runs clippy on the moving `stable` toolchain. Newer clippy releases flag the `#[must_use]` that
// `async_trait` expands onto methods returning a (must-use) future (`double_must_use`). The code is
// generated, not hand-written, so the lint is silenced for this crate only; remove once
// async-trait stops emitting it.
#![allow(clippy::double_must_use)]

pub mod errors;
pub mod escrow;
pub mod faceit_api;
pub mod mock;
pub mod models;
pub mod multisig;
pub mod oracle;
pub mod payout;
pub mod rpc;
pub mod wallet;
pub mod watcher;

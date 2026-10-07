// CI runs clippy on the moving `stable` toolchain. Newer clippy releases flag the `#[must_use]` that
// `async_trait` expands onto trait methods returning a (must-use) future (`double_must_use`, 14
// hits in `kaspa_backend.rs` / `workers/oracle_worker.rs`). The code is generated, not hand-written,
// so the lint is silenced for this crate only; remove once async-trait stops emitting it.
#![allow(clippy::double_must_use)]

pub mod auth;
pub mod constants;
pub mod errors;
pub mod faceit_client; // F-07: Zentraler HTTP-Client mit Circuit-Breaker (F-10)
pub mod faceit_data;
pub mod faceit_oauth;
pub mod kaspa_backend;
pub mod match_state;
pub mod models;
pub mod native_games;
pub mod oracle;
pub mod secret_provider; // F-11: Secrets-Abstraktion (Docker Secrets / ENV)
pub mod types;
pub mod workers;

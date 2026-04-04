//! Match lifecycle handlers: create, join, lobbies, history, deposits, faceit, admin.
//!
//! **CQ-01 Phase 1** — This module exists as a structural placeholder.
//! Handler implementations and type definitions currently reside in `api/mod.rs`.
//!
//! **Post-beta migration plan:**
//! 1. Move CreateReq, DepositReq, ResolveReq, FaceitWebhookPayload etc. here
//! 2. Move handler functions (create_challenge, join_challenge, submit_deposit...) here
//! 3. Remove definitions from api/mod.rs

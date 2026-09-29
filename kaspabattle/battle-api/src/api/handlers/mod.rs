//! Route handler modules (CQ-01: split from api/mod.rs monolith).
//!
//! Each sub-module owns one logical domain area.
//! All public symbols are re-exported from this module so that `mod.rs`
//! routes continue to compile without changes.

pub mod admin;
pub mod auth;
pub mod deposits;
pub mod matches;
pub mod ws;

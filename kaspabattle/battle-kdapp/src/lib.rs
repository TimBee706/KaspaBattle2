//! battle-kdapp — KaspaBattle on-chain Episode integration via kdapp framework.
//!
//! This crate inlines the core kdapp framework (Episode, Engine, Generator, Proxy, PKI)
//! to avoid transitive dependency conflicts with rusty-kaspa versions.
//!
//! ## kdapp modules (inlined from michaelsutton/kdapp)
//! - `kdapp_episode` — Episode trait definition
//! - `kdapp_pki` — Public Key Infrastructure (PubKey, Sig, signing)
//! - `kdapp_generator` — TX builder with bit-pattern mining
//! - `kdapp_engine` — Episode lifecycle + DAG reorg handling
//! - `kdapp_proxy` — Resolver-based wRPC client + TX listener
//!
//! ## KaspaBattle modules
//! - `commands` — BattleCommand + BattleRollback enums
//! - `episode` — BattleEpisode implementing Episode trait
//! - `handler` — BattleHandler implementing EpisodeEventHandler
//! - `proxy_config` — Pattern/Prefix constants

// ── kdapp framework (inlined) ───────────────────────────────────────────
pub mod kdapp_pki;
pub mod kdapp_episode;
pub mod kdapp_generator;
pub mod kdapp_engine;
pub mod kdapp_proxy;

// ── KaspaBattle application modules ─────────────────────────────────────
pub mod commands;
pub mod episode;
pub mod handler;
pub mod proxy_config;
pub mod startup;

// ── Tournament modules (Phase 1) ─────────────────────────────────────────
pub mod tournament_commands;
pub mod tournament_episode;


#[cfg(test)]
mod engine_integration_test;

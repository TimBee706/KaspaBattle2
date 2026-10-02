//! Native (browser) games — pure, deterministic domain logic.
//!
//! Nothing in this module touches HTTP, the database or the clock, so the same code can later be
//! wrapped as a kdapp `Episode` (see `docs/09-NATIVE-GAMES.md`, "Phase 2"). The API mirrors the
//! kdapp tic-tac-toe example: `execute` validates authorization + rules and returns a rollback
//! record, `rollback` restores the exact previous state.

pub mod connect_four;
pub mod connect_four_bot;

use sha2::{Digest, Sha256};

/// Lower-case hex encoding (kept local so `battle-core` needs no extra dependency).
pub(crate) fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push(HEX[(b >> 4) as usize] as char);
        out.push(HEX[(b & 0x0f) as usize] as char);
    }
    out
}

/// Why a natively played match ended (persisted as `native_game_sessions.end_reason`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EndReason {
    /// Four in a row.
    ConnectFour,
    /// Board full, no winner.
    Draw,
    /// A player resigned.
    Resignation,
    /// A player's turn deadline expired (abandoned game).
    Timeout,
}

impl EndReason {
    pub fn as_str(&self) -> &'static str {
        match self {
            EndReason::ConnectFour => "CONNECT_FOUR",
            EndReason::Draw => "DRAW",
            EndReason::Resignation => "RESIGNATION",
            EndReason::Timeout => "TIMEOUT",
        }
    }
}

/// Deterministic settlement hash binding the result to the match and the final board.
///
/// `winner == None` means draw. The value is stored as `matches.result_hash` and is what an
/// on-chain attestation would later commit to.
pub fn result_hash(
    match_id: &str,
    game_type: &str,
    final_state_hash: &str,
    winner: Option<&str>,
    reason: EndReason,
) -> String {
    let mut h = Sha256::new();
    h.update(b"kaspabattle-native-result-v1|");
    h.update(match_id.as_bytes());
    h.update(b"|");
    h.update(game_type.as_bytes());
    h.update(b"|");
    h.update(final_state_hash.as_bytes());
    h.update(b"|");
    h.update(winner.unwrap_or("DRAW").as_bytes());
    h.update(b"|");
    h.update(reason.as_str().as_bytes());
    hex_encode(&h.finalize())
}

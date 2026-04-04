//! Platform-wide constants shared across crates.
//!
//! Single source of truth: import from `battle_core::constants` in all crates.

// Financial constants

/// Platform fee deducted from the pot on payout (5 %).
/// Used in `EscrowService` and `MultisigEscrowService`.
pub const PLATFORM_FEE_PERCENT: u64 = 5;

// Session constants

/// Default session lifetime in days.
///
/// Can be overridden at runtime via `SESSION_LIFETIME_DAYS` env var.
pub const SESSION_LIFETIME_DAYS_DEFAULT: i64 = 7;

/// Read session lifetime from `SESSION_LIFETIME_DAYS` env var, or fall back
/// to `SESSION_LIFETIME_DAYS_DEFAULT`.
pub fn session_lifetime_days() -> i64 {
    std::env::var("SESSION_LIFETIME_DAYS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(SESSION_LIFETIME_DAYS_DEFAULT)
}

// WebSocket constants

/// Default broadcast channel capacity for the WebSocket event bus.
///
/// Can be overridden at runtime via `WS_BROADCAST_CAPACITY` env var.
pub const WS_BROADCAST_CAPACITY_DEFAULT: usize = 100;

/// Read broadcast channel capacity from `WS_BROADCAST_CAPACITY` env var.
pub fn ws_broadcast_capacity() -> usize {
    std::env::var("WS_BROADCAST_CAPACITY")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(WS_BROADCAST_CAPACITY_DEFAULT)
}

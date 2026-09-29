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

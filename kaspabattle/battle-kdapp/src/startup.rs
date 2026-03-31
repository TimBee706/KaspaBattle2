//! Convenience module for starting the kdapp Engine + Proxy as background tasks.
//!
//! Provides `spawn_kdapp_services()` which hides NetworkId parsing,
//! EngineMap construction, and the blocking/async split from the caller.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::Arc;

use sqlx::PgPool;
use tokio::sync::broadcast;

use kaspa_consensus_core::network::{NetworkId, NetworkType};

use crate::episode::BattleEpisode;
use crate::handler::BattleHandler;
use crate::kdapp_engine::Engine;
use crate::kdapp_proxy::{self, EngineMap};
use crate::proxy_config::{BATTLE_PATTERN, BATTLE_PREFIX};

/// Parse a network string (e.g. "testnet-10") into a Kaspa `NetworkId`.
pub fn parse_network(network: &str) -> Result<NetworkId, String> {
    match network {
        "mainnet" => Ok(NetworkId::new(NetworkType::Mainnet)),
        "testnet-10" | "testnet10" => Ok(NetworkId::with_suffix(NetworkType::Testnet, 10)),
        "testnet-11" | "testnet11" => Ok(NetworkId::with_suffix(NetworkType::Testnet, 11)),
        "testnet-12" | "testnet12" => Ok(NetworkId::with_suffix(NetworkType::Testnet, 12)),
        "testnet" => Ok(NetworkId::new(NetworkType::Testnet)),
        other => Err(format!("Unknown Kaspa network: {}", other)),
    }
}

/// Handle to the running kdapp services — can be used to request shutdown.
pub struct KdappHandle {
    exit_signal: Arc<AtomicBool>,
}

impl KdappHandle {
    /// Signal the Proxy listener and Engine to shut down gracefully.
    pub fn shutdown(&self) {
        self.exit_signal.store(true, Ordering::Relaxed);
    }
}

/// Spawn the kdapp Engine and Proxy as background tasks.
///
/// - **Engine**: Runs on a blocking thread (`tokio::task::spawn_blocking`).
///   Receives `EngineMsg` via std::sync::mpsc and dispatches to `BattleEpisode`
///   + `BattleHandler`.
///
/// - **Proxy**: Runs as an async task. Connects to a Kaspa node via Resolver,
///   polls the virtual chain every ~1s, and forwards matching TXs to the Engine.
///
/// Returns a `KdappHandle` that can be used to shut down both services.
///
/// # Arguments
/// * `pool` — PostgreSQL connection pool (shared with the handler)
/// * `ws_tx` — WebSocket broadcast sender (shared with the handler)
/// * `network` — Network string (e.g. "testnet-10")
/// * `rpc_url` — Optional explicit node URL (None = use Resolver)
pub fn spawn_kdapp_services(
    pool: PgPool,
    ws_tx: broadcast::Sender<String>,
    network: &str,
    rpc_url: Option<String>,
) -> Result<KdappHandle, String> {
    let network_id = parse_network(network)?;
    let exit_signal = Arc::new(AtomicBool::new(false));

    let (engine_tx, engine_rx) = mpsc::channel();

    // Build the EngineMap: one entry mapping our BATTLE_PREFIX → (BATTLE_PATTERN, sender)
    let mut engine_map: EngineMap = HashMap::new();
    engine_map.insert(BATTLE_PREFIX, (BATTLE_PATTERN, engine_tx));

    // ── Engine thread (blocking) ────────────────────────────────────────
    let engine_pool = pool.clone();
    let engine_ws = ws_tx.clone();
    tokio::task::spawn_blocking(move || {
        let handler = BattleHandler::new(engine_pool, engine_ws);
        let mut engine = Engine::<BattleEpisode, BattleHandler>::new(engine_rx);
        tracing::info!("🔗 kdapp Engine started (blocking thread)");
        engine.start(vec![handler]);
        tracing::info!("🔗 kdapp Engine exited");
    });

    // ── Proxy task (async) ──────────────────────────────────────────────
    let proxy_exit = exit_signal.clone();
    tokio::spawn(async move {
        tracing::info!("🌐 kdapp Proxy connecting (network: {})...", network_id);

        let client = match kdapp_proxy::connect_client(network_id, rpc_url).await {
            Ok(c) => {
                tracing::info!("✅ kdapp Proxy connected to Kaspa node");
                c
            }
            Err(e) => {
                tracing::error!("❌ kdapp Proxy connection failed: {} — disabled", e);
                return;
            }
        };

        tracing::info!("🔄 kdapp Proxy listener started (prefix={:#010x}, pattern={} bits)",
            BATTLE_PREFIX, BATTLE_PATTERN.len()
        );
        kdapp_proxy::run_listener(client, engine_map, proxy_exit).await;
        tracing::info!("🌐 kdapp Proxy listener exited");
    });

    tracing::info!("✅ kdapp Engine + Proxy background tasks launched");
    Ok(KdappHandle { exit_signal })
}

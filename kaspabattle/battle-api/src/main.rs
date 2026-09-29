use axum::{
    http::{
        header::{AUTHORIZATION, CONTENT_TYPE},
        Method,
    },
    routing::get,
    Router,
};
use sqlx::postgres::PgPoolOptions;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::broadcast;
use tower_http::cors::CorsLayer;

mod api;
mod episodes;
mod models;
mod native_game;
#[cfg(test)]
mod native_tests;
mod services;
mod payout_worker;
mod refund_worker;
mod tournament_payout_worker;

fn try_load_dotenv() -> Vec<std::path::PathBuf> {
    let mut candidates = Vec::new();

    if let Ok(cwd) = std::env::current_dir() {
        candidates.push(cwd.join(".env"));
        candidates.push(cwd.join("../.env"));
        candidates.push(cwd.join("kaspabattle/.env"));
        candidates.push(cwd.join("battle-api/.env"));
        candidates.push(cwd.join("kaspabattle/battle-api/.env"));
    }

    if let Ok(exe) = std::env::current_exe() {
        if let Some(exe_dir) = exe.parent() {
            candidates.push(exe_dir.join(".env"));
            candidates.push(exe_dir.join("../.env"));
            candidates.push(exe_dir.join("../../.env"));
            candidates.push(exe_dir.join("../../../.env"));
        }
    }

    candidates.sort();
    candidates.dedup();

    for path in &candidates {
        if path.exists() {
            match dotenvy::from_path(path) {
                Ok(()) => {
                    tracing::info!("Loaded environment from {}", path.display());
                    break;
                }
                Err(e) => {
                    tracing::error!("Failed to load {}: {}", path.display(), e);
                }
            }
        }
    }

    candidates
}

fn env_flag(name: &str) -> bool {
    matches!(
        std::env::var(name)
            .ok()
            .map(|v| v.trim().to_ascii_lowercase()),
        Some(v) if matches!(v.as_str(), "1" | "true" | "yes" | "on")
    )
}

fn explicit_kaspa_node_enabled() -> bool {
    env_flag("KASPA_USE_EXPLICIT_NODE") || env_flag("KASPA_USE_NODE_URL")
}

fn normalized_kaspa_node_url() -> Option<String> {
    let url = std::env::var("KASPA_NODE_URL").ok()?;
    let trimmed = url.trim();

    if trimmed.is_empty() {
        return None;
    }

    if !explicit_kaspa_node_enabled() {
        tracing::info!(
            "Ignoring configured KASPA_NODE_URL={} because explicit node usage is disabled; falling back to Kaspa Resolver",
            trimmed
        );
        return None;
    }

    // Retire the legacy public testnet endpoint so stale local env files no longer
    // bypass Resolver-based discovery.
    if trimmed.contains("photon-10.kaspa.red") {
        tracing::info!("Ignoring legacy KASPA_NODE_URL={} and falling back to Kaspa Resolver",
            trimmed
        );
        return None;
    }

    Some(trimmed.to_string())
}

#[cfg(test)]
#[allow(clippy::items_after_test_module)]
mod tests {
    use super::{explicit_kaspa_node_enabled, normalized_kaspa_node_url};

    #[test]
    fn test_kaspa_node_url_resolution() {
        // Make sure environment is clean
        std::env::remove_var("KASPA_USE_EXPLICIT_NODE");
        std::env::remove_var("KASPA_USE_NODE_URL");

        // 1. explicit_node_is_opt_in
        std::env::set_var("KASPA_NODE_URL", "ws://kaspa-node:16111");
        assert_eq!(normalized_kaspa_node_url(), None);

        // 2. explicit_node_can_be_enabled
        std::env::set_var("KASPA_USE_EXPLICIT_NODE", "true");
        // Ensure KASPA_NODE_URL is still set to what we expect
        std::env::set_var("KASPA_NODE_URL", "ws://kaspa-node:16111");
        
        assert!(explicit_kaspa_node_enabled());
        assert_eq!(
            normalized_kaspa_node_url(),
            Some("ws://kaspa-node:16111".to_string())
        );

        // Cleanup
        std::env::remove_var("KASPA_USE_EXPLICIT_NODE");
        std::env::remove_var("KASPA_NODE_URL");
    }
}

#[tokio::main]
async fn main() {
    // ── Structured Logging ──────────────────────────────────────────────────
    // Without this, ALL tracing::info!/warn!/error! calls are silently dropped.
    // RUST_LOG env var controls verbosity, e.g. RUST_LOG=battle_api=debug,battle_kaspa=debug,info
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "battle_api=debug,battle_kaspa=debug,info".parse().unwrap()),
        )
        .init();

    let dotenv_candidates = try_load_dotenv();

    let db_url = std::env::var("DATABASE_URL").unwrap_or_else(|_| {
        let searched = dotenv_candidates
            .iter()
            .map(|path| path.display().to_string())
            .collect::<Vec<_>>()
            .join(", ");
        panic!(
            "DATABASE_URL must be set. Tried loading .env from: {}",
            searched
        );
    });
    let pool = PgPoolOptions::new()
        .min_connections(5)
        .max_connections(20)
        .acquire_timeout(Duration::from_secs(5))
        .idle_timeout(Duration::from_secs(300))
        .max_lifetime(Duration::from_secs(1800))
        .connect(&db_url)
        .await
        .expect("Failed to connect to PostgreSQL. Is the database running?");

    // Run all SQL migrations from the `migrations` folder
    // If any migration checksum mismatches (e.g. migration was made idempotent),
    // clear the tracking table and re-apply all migrations cleanly.
    let migration_result = sqlx::migrate!("../migrations")
        .run(&pool)
        .await;

    match migration_result {
        Ok(_) => {
            tracing::info!("✅ DB migrations applied successfully");
        }
        Err(e) if e.to_string().contains("VersionMismatch")
            || e.to_string().contains("checksum")
            || e.to_string().contains("Checksum")
            || e.to_string().contains("previously applied but has been modified")
            || e.to_string().contains("previously applied but is missing") => {
            tracing::warn!(
                "⚠️ Migration state mismatch detected ('{}') — \
                resetting _sqlx_migrations and re-applying all (idempotent) migrations...",
                e
            );
            sqlx::query("DELETE FROM _sqlx_migrations")
                .execute(&pool)
                .await
                .expect("Failed to reset _sqlx_migrations");
            sqlx::migrate!("../migrations")
                .run(&pool)
                .await
                .expect("Failed to run database migrations after reset");
            tracing::info!("✅ DB migrations re-applied successfully after reset");
        }
        Err(e) => {
            panic!("Failed to run database migrations: {}", e);
        }
    }

    let (tx, _) = broadcast::channel(battle_core::constants::ws_broadcast_capacity());

    // ── SecretProvider (F-11): Auto-detects Docker Secrets vs ENV ────────────
    let secrets = battle_core::secret_provider::SecretProvider::auto_detect();

    let auth_service = Arc::new(battle_core::auth::AuthService::new(pool.clone()));

    let faceit_config = battle_core::models::faceit::FaceitOAuthConfig {
        client_id: secrets.require("FACEIT_CLIENT_ID").expect("Missing FACEIT_CLIENT_ID"),
        client_secret: secrets.require("FACEIT_CLIENT_SECRET").expect("Missing FACEIT_CLIENT_SECRET"),
        redirect_uri: secrets.require("FACEIT_REDIRECT_URI").expect("Missing FACEIT_REDIRECT_URI"),
        auth_url: "https://accounts.faceit.com".to_string(),
        token_url: "https://api.faceit.com/auth/v1/oauth/token".to_string(),
        userinfo_url: "https://api.faceit.com/auth/v1/resources/userinfo".to_string(),
    };
    let faceit_service = Arc::new(battle_core::faceit_oauth::FaceitOAuthService::new(
        faceit_config,
        pool.clone(),
    ));

    // ── battle-kaspa: Initialize Kaspa escrow infrastructure ──
    // If KASPA_NODE_URL is set, use it directly. Otherwise, Resolver auto-discovers the best node.
    let kaspa_node_url = normalized_kaspa_node_url();
    let kaspa_network = std::env::var("KASPA_NETWORK").unwrap_or_else(|_| "testnet-12".to_string());
    let kaspa_mnemonic = std::env::var("KASPA_MNEMONIC").ok();



    let escrow_wallet = Arc::new(
        battle_kaspa::wallet::EscrowWallet::new(kaspa_mnemonic, &kaspa_network)
            .expect("Failed to initialize EscrowWallet"),
    );
    tracing::info!("✅ EscrowWallet initialized (network: {})", kaspa_network);

    // Connect to Kaspa node via Resolver (or explicit URL if set)
    let kaspa_rpc: Option<Arc<dyn battle_kaspa::rpc::KaspaBackend>> =
        match battle_kaspa::rpc::RealKaspaClient::new_with_resolver(
            kaspa_node_url.as_deref(),
            &kaspa_network,
        ).await {
            Ok(client) => {
                tracing::info!("✅ Connected to Kaspa node{}",
                    kaspa_node_url.as_ref()
                        .map(|u| format!(": {}", u))
                        .unwrap_or_else(|| " (via Resolver)".to_string())
                );
                Some(Arc::new(client))
            }
            Err(e) => {
                tracing::error!("⚠️ Kaspa RPC connection failed (escrow features disabled): {}",
                    e
                );
                None
            }
        };

    // Note: We do NOT block the HTTP server startup waiting for node sync.
    // The episode runner (background task) will wait for sync before polling.
    // This ensures API endpoints are available immediately.

    let escrow_service = kaspa_rpc.as_ref().map(|rpc| {
        Arc::new(battle_kaspa::escrow::EscrowService::new(
            escrow_wallet.clone(),
            rpc.clone(),
        ))
    });

    // Initialize PayoutService (real TX signing + submission)
    // Derive treasury address from TREASURY_MNEMONIC (or use TREASURY_ADDRESS directly)
    let treasury_mnemonic_result = std::env::var("TREASURY_MNEMONIC");
    tracing::debug!("TREASURY_MNEMONIC: present={}", treasury_mnemonic_result.is_ok());
    let treasury_address = if let Ok(treasury_mnemonic) = treasury_mnemonic_result {
        let treasury_wallet =
            battle_kaspa::wallet::EscrowWallet::new(Some(treasury_mnemonic), &kaspa_network)
                .expect("Failed to initialize treasury wallet from TREASURY_MNEMONIC");
        let (addr, _) = treasury_wallet
            .derive_escrow_address("treasury-main")
            .expect("Failed to derive treasury address");
        let addr_str = addr.to_string();
        tracing::info!("✅ Treasury address derived: {}", addr_str);
        addr_str
    } else {
        std::env::var("TREASURY_ADDRESS").unwrap_or_else(|_| {
            "kaspatest:qpqehja8q7549wkjjrxl3qkc63a252v9c9pu8zp5rtrc8efll8dhyh9qep0q2".to_string()
        })
    };
    let payout_service = kaspa_rpc.as_ref().map(|rpc| {
        Arc::new(battle_kaspa::payout::PayoutService::new(
            rpc.clone(),
            treasury_address.clone(),
            std::collections::HashMap::new(), // Keys registered dynamically per match
        ))
    });
    if payout_service.is_some() {
        tracing::info!("✅ PayoutService initialized (treasury: {})",
            treasury_address
        );
    }

    // Initialize MultisigEscrowService (2-of-3 P2SH escrows)
    let multisig_service = kaspa_rpc.as_ref().and_then(|rpc| {
        // Determine network prefix
        let prefix = if kaspa_network.contains("mainnet") {
            kaspa_addresses::Prefix::Mainnet
        } else {
            kaspa_addresses::Prefix::Testnet
        };

        // Derive oracle private key from ORACLE_PRIVATE_KEY env — hard-fail if missing
        let oracle_sk_hex = std::env::var("ORACLE_PRIVATE_KEY").unwrap_or_else(|_| {
            panic!("CRITICAL: ORACLE_PRIVATE_KEY must be set in all environments. Generate with: openssl rand -hex 32");
        });

        let oracle_sk_bytes: [u8; 32] = match hex::decode(oracle_sk_hex.trim()) {
            Ok(bytes) if bytes.len() == 32 => bytes.try_into().unwrap(),
            _ => {
                tracing::warn!("⚠️ Invalid ORACLE_PRIVATE_KEY — MultisigEscrowService disabled");
                return None;
            }
        };

        // SEC-MULTISIG-01: derive player escrow keys using a server-side secret,
        // not just the (public) match ID — see derive_player_key doc comment.
        // Hard-fail if missing, same as ORACLE_PRIVATE_KEY: silently falling back
        // to a fixed/default secret would recreate the original vulnerability.
        let key_derivation_secret_hex = std::env::var("MULTISIG_KEY_DERIVATION_SECRET")
            .unwrap_or_else(|_| {
                panic!("CRITICAL: MULTISIG_KEY_DERIVATION_SECRET must be set in all environments. Generate with: openssl rand -hex 32");
            });
        let key_derivation_secret: [u8; 32] = match hex::decode(key_derivation_secret_hex.trim()) {
            Ok(bytes) if bytes.len() == 32 => bytes.try_into().unwrap(),
            _ => {
                tracing::warn!("⚠️ Invalid MULTISIG_KEY_DERIVATION_SECRET — MultisigEscrowService disabled");
                return None;
            }
        };

        match battle_kaspa::multisig::service::MultisigEscrowService::new(
            rpc.clone(),
            prefix,
            oracle_sk_bytes,
            treasury_address.clone(),
            key_derivation_secret,
        ) {
            Ok(service) => {
                tracing::info!("✅ MultisigEscrowService initialized (prefix: {:?})", prefix);
                Some(Arc::new(service))
            }
            Err(e) => {
                tracing::error!("⚠️ MultisigEscrowService failed: {} — disabled", e);
                None
            }
        }
    });

    // ── FACEIT Data API Service (F-11/F-15) ──────────────────────────────────
    // Key resolution order (F-15 — Multi-Environment):
    //   1. FACEIT_DATA_API_KEY_{APP_ENV}  (z.B. FACEIT_DATA_API_KEY_PRODUCTION)
    //   2. FACEIT_DATA_API_KEY            (generischer Fallback)
    //
    // SecretProvider (F-11) prüft zuerst Docker Secrets, dann ENV.
    let app_env = std::env::var("APP_ENV")
        .or_else(|_| std::env::var("RUST_ENV"))
        .unwrap_or_else(|_| "development".to_string());
    let app_env_upper = app_env.to_uppercase();

    let faceit_data_api_key: Option<String> = {
        // Env-spezifischer Key (z.B. FACEIT_DATA_API_KEY_PRODUCTION)
        let env_specific_name = format!("FACEIT_DATA_API_KEY_{}", app_env_upper);
        let env_key = secrets.get(&env_specific_name)
            .unwrap_or(None)
            .filter(|k| !k.is_empty());

        if env_key.is_some() {
            tracing::info!(
                "✅ FACEIT Data API key loaded from env-specific secret '{}' (APP_ENV={})",
                env_specific_name, app_env
            );
            env_key
        } else {
            // Generischer Fallback
            let generic_key = secrets.get("FACEIT_DATA_API_KEY")
                .unwrap_or(None)
                .filter(|k| !k.is_empty());

            if generic_key.is_some() {
                tracing::info!("✅ FACEIT Data API key loaded from generic FACEIT_DATA_API_KEY");
            } else {
                tracing::warn!(
                    "⚠️ No FACEIT Data API key found (tried '{}' and 'FACEIT_DATA_API_KEY') — \
                     /faceit/profile, /faceit/stats and /faceit/matches will be unavailable",
                    env_specific_name
                );
            }
            generic_key
        }
    };

    let faceit_data_service = faceit_data_api_key.map(|api_key| {
        tracing::info!("✅ FaceitDataService initialized (API key set)");
        Arc::new(battle_core::faceit_data::FaceitDataService::new(api_key))
    });

    // ── BlockchainWatcher (per-player UTXO attribution + confirmation tracking) ──
    let blockchain_watcher = kaspa_rpc.as_ref().map(|rpc| {
        let watcher = battle_kaspa::watcher::BlockchainWatcher::new(
            rpc.clone(),
            std::time::Duration::from_secs(5),
        );
        tracing::info!("✅ BlockchainWatcher initialized");
        Arc::new(watcher)
    });

    let state = api::AppState {
        pool,
        tx,
        auth_service,
        faceit_service,
        faceit_data_service,
        escrow_wallet: Some(escrow_wallet),
        escrow_service,
        kaspa_rpc,
        payout_service,
        blockchain_watcher,
        multisig_service,
    };

    // ── CORS: Multi-Origin Support (SEC-02) ───────────────────────────────────
    // CORS_ALLOWED_ORIGINS: comma-separated allowed origins.
    // Fallback: FRONTEND_URL for backwards compatibility.
    let _frontend_url_str =
        std::env::var("FRONTEND_URL").unwrap_or_else(|_| "http://localhost:5173".to_string());

    let allowed_origins: Vec<String> = std::env::var("CORS_ALLOWED_ORIGINS")
        .map(|s| {
            s.split(',')
                .map(|o| o.trim().to_string())
                .filter(|o| !o.is_empty())
                .collect()
        })
        .unwrap_or_else(|_| vec![_frontend_url_str.clone()]);

    tracing::info!("CORS allowed origins: {:?}", allowed_origins);

    let cors = {
        use tower_http::cors::AllowOrigin;
        let origins_for_cors = allowed_origins.clone();
        CorsLayer::new()
            .allow_origin(AllowOrigin::predicate(move |origin, _| {
                origin
                    .to_str()
                    .map(|o| origins_for_cors.iter().any(|allowed| allowed == o))
                    .unwrap_or(false)
            }))
            .allow_methods([Method::GET, Method::POST, Method::PUT, Method::PATCH, Method::DELETE])
            .allow_headers([
                AUTHORIZATION, 
                CONTENT_TYPE, 
                axum::http::header::HeaderName::from_static("ngrok-skip-browser-warning")
            ])
            .allow_credentials(true)
    };

    // CSRF: allow all configured origins
    let csrf_allowed_origins = allowed_origins.clone();

    let app = Router::new()
        .nest("/api/v1", api::router())
        .route("/health", get(api::health))
        .route("/ws", get(api::ws_handler))
        .layer(cors)
        .layer(axum::middleware::from_fn(move |req, next| {
            let extra_origins = csrf_allowed_origins.clone();
            async move {
                api::csrf_guard::csrf_protection_layer_multi(extra_origins, req, next).await
            }
        }))
        .with_state(state.clone());

    tracing::info!("🚀 KaspaBattle API running on 0.0.0.0:8080");
    let addr = SocketAddr::from(([0, 0, 0, 0], 8080));
    let listener = match tokio::net::TcpListener::bind(&addr).await {
        Ok(l) => l,
        Err(e) if e.kind() == std::io::ErrorKind::AddrInUse => {
            tracing::error!(
                "❌ Port 8080 is already in use (another battle-api instance is running).\n\
                 Kill it first with:\n\
                 PowerShell: Stop-Process -Id (Get-NetTCPConnection -LocalPort 8080).OwningProcess -Force\n\
                 Linux/Mac:  kill $(lsof -ti:8080)"
            );
            std::process::exit(1);
        }
        Err(e) => {
            panic!("Failed to bind TCP listener on 0.0.0.0:8080: {}", e);
        }
    };

    // ── Background Episode-Runner (v0.4) ──────────────────────────────────────
    // Polls every 5s for AWAITING_FUNDING matches (fast confirmation detection),
    // every 30s for FUNDED and LOCKED matches.
    tokio::spawn({
        let ep_pool = state.pool.clone();
        let ep_escrow = state.escrow_service.clone();
        let ep_watcher = state.blockchain_watcher.clone();
        let ep_rpc = state.kaspa_rpc.clone();
        let ep_tx = state.tx.clone();
        async move {
            // First poll runs immediately on startup (catch-up for deposits made while backend was down)
            tracing::info!("🚀 Episode runner starting — performing initial catch-up poll");
            tracing::info!("🚀 Episode runner: initial catch-up poll (no delay)");

            // Wait for Kaspa node to be fully synced before starting deposit detection.
            // This runs in the background so it does NOT block the HTTP server.
            if let Some(ref rpc) = ep_rpc {
                tracing::debug!("⏳ Episode runner: waiting for Kaspa node to sync (timeout: 5 min)...");
                match rpc.wait_for_sync(std::time::Duration::from_secs(300)).await {
                    Ok(()) => tracing::info!("✅ Episode runner: Kaspa node is synced and UTXO-indexed — starting deposit detection"),
                    Err(e) => {
                        tracing::error!("⚠️ Episode runner: node sync wait failed: {} — will retry via health-check", e);
                    }
                }
            }
            let mut consecutive_errors = 0;
            loop {
                // Phase 5.2: All statuses needing episode runner attention
                let db_result = sqlx::query_as::<_, (uuid::Uuid,)>(
                    "SELECT id FROM matches WHERE status IN \
                     ('OPEN', 'AWAITING_FUNDING', 'FUNDED', 'LOCKED', \
                      'GAME_ID_INPUT', 'IN_GAME', 'FINISHED_FACEIT', 'READY_FOR_PAYOUT', \
                      'READY_TO_PLAY')",
                )
                .fetch_all(&ep_pool)
                .await;

                let active_ids = match &db_result {
                    Ok(ids) => ids.clone(),
                    Err(e) => {
                        tracing::error!(error = %e, "Episode runner: DB fetch active matches failed");
                        vec![]
                    }
                };

                if !active_ids.is_empty() {
                    tracing::info!("🔄 Episode runner: polling {} active match(es)",
                        active_ids.len()
                    );
                }

                // Periodic RPC health-check: detect stale connections early.
                // If DAA=0, the node is not ready — skip the poll cycle.
                // If the RPC call fails, ensure_connected() will auto-reconnect
                // on the next call inside MatchEpisode::execute().
                if let Some(ref rpc) = ep_rpc {
                    match rpc.get_current_daa_score().await {
                        Ok(0) => {
                            tracing::warn!("⚠️ Episode runner: DAA=0, node may not be ready — deposits won't be confirmed until node is synced");
                        }
                        Err(e) => {
                            tracing::error!("🚨 Episode runner: RPC health-check failed: {} — reconnect will be attempted on next RPC call", e);
                        }
                        Ok(daa) => {
                            tracing::debug!(current_daa = daa, "Episode runner: RPC healthy");
                        }
                    }
                }

                let mut loop_had_errors = db_result.is_err();

                for (match_id,) in active_ids {
                    use crate::episodes::match_episode::MatchEpisode;
                    use crate::episodes::EpisodeTrait;
                    let ctx = (
                        ep_pool.clone(),
                        ep_escrow.clone(),
                        ep_tx.clone(),
                        ep_watcher.clone(),
                        ep_rpc.clone(),
                    );
                    match MatchEpisode::initialize(&ctx, match_id).await {
                        Ok(mut ep) => {
                            if let Err(e) = ep.execute().await {
                                tracing::error!(match_id = %match_id, error = %e, "Episode runner failed");
                                loop_had_errors = true;
                            }
                        }
                        Err(e) => {
                            tracing::error!(match_id = %match_id, error = %e, "Episode init failed");
                            loop_had_errors = true;
                        }
                    }
                }

                if loop_had_errors {
                    consecutive_errors += 1;
                } else {
                    consecutive_errors = 0;
                }

                let sleep_secs = match consecutive_errors {
                    0 => 5,
                    1 => 10,
                    2 => 20,
                    3 => 40,
                    _ => 60,
                };

                tokio::time::sleep(std::time::Duration::from_secs(sleep_secs)).await;
            }
        }
    });

    // ── FaceIT Watcher (Phase 3 / F-010) ────────────────────────────────────
    // Polls faceit_watcher_jobs every FACEIT_WATCHER_POLL_INTERVAL_SECS seconds.
    // Only started when FACEIT_DATA_API_KEY is set.
    if let Some(ref faceit_data_svc) = state.faceit_data_service {
        let watcher_pool = Arc::new(state.pool.clone());
        let watcher_faceit = faceit_data_svc.clone();
        let watcher_config = battle_core::workers::faceit_watcher::FaceitWatcherConfig::default();
        tokio::spawn(async move {
            battle_core::workers::faceit_watcher::run_faceit_watcher(
                watcher_pool,
                watcher_faceit,
                watcher_config,
            )
            .await;
        });
        tracing::info!("✅ FaceIT Watcher started");
    } else {
        tracing::info!("ℹ️ FaceIT Watcher disabled (FACEIT_DATA_API_KEY not set)");
    }

    // ── Payout Worker (Phase 4a) ─────────────────────────────────────────────
    // Polls FINISHED_FACEIT matches and auto-creates PSKTs → READY_FOR_PAYOUT.
    // Only started when MultisigEscrowService is available (Kaspa RPC connected).
    if let Some(ref multisig_svc) = state.multisig_service {
        let pw_pool = Arc::new(state.pool.clone());
        let pw_multisig = multisig_svc.clone();
        tokio::spawn(async move {
            crate::payout_worker::run_payout_worker(pw_pool, pw_multisig).await;
        });
        tracing::info!("✅ Payout Worker started");
    } else {
        tracing::info!("ℹ️ Payout Worker disabled (MultisigEscrowService not available)");
    }

    // ── Refund Worker ─────────────────────────────────────────────────────────
    // Polls CANCELLED/DISPUTED matches and auto-executes on-chain refunds.
    // Only started when MultisigEscrowService is available (Kaspa RPC connected).
    if let Some(ref multisig_svc) = state.multisig_service {
        let rw_pool = Arc::new(state.pool.clone());
        let rw_multisig = multisig_svc.clone();
        tokio::spawn(async move {
            crate::refund_worker::run_refund_worker(rw_pool, rw_multisig).await;
        });
        tracing::info!("✅ Refund Worker started");
    } else {
        tracing::info!("ℹ️ Refund Worker disabled (MultisigEscrowService not available)");
    }

    // ── Tournament Workers (Phase 2) ──────────────────────────────────────────
    // Deposit watcher: scans tournament escrow UTXOs every 30s.
    // Payout executor: runs every 60s for COMPLETED tournaments.
    crate::tournament_payout_worker::spawn_tournament_workers(
        Arc::new(state.pool.clone()),
        state.escrow_service.clone(),
        state.blockchain_watcher.clone(),
        state.payout_service.clone(),
    );
    tracing::info!("✅ Tournament workers spawned");

    // ── kdapp Engine + Proxy (v0.7 — on-chain Episode processing) ─────────
    // Runs parallel to the legacy episode-runner above.
    // Uses battle-kdapp's convenience function to hide Engine/Proxy internals.
    {
        let kdapp_network = std::env::var("KASPA_NETWORK")
            .unwrap_or_else(|_| "testnet-12".to_string());
        let kdapp_rpc_url = normalized_kaspa_node_url();

        match battle_kdapp::startup::spawn_kdapp_services(
            state.pool.clone(),
            state.tx.clone(),
            &kdapp_network,
            kdapp_rpc_url,
        ) {
            Ok(_handle) => {
                tracing::info!("✅ kdapp Engine + Proxy started (network: {})", kdapp_network);
            }
            Err(e) => {
                tracing::error!("⚠️ kdapp Engine + Proxy failed to start: {} — disabled", e);
            }
        }
    }

    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
    .await
    .unwrap();
}

use axum::{
    http::{
        header::{AUTHORIZATION, CONTENT_TYPE},
        HeaderValue, Method,
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
mod services;

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
                    eprintln!("Loaded environment from {}", path.display());
                    break;
                }
                Err(e) => {
                    eprintln!("Failed to load {}: {}", path.display(), e);
                }
            }
        }
    }

    candidates
}

fn normalized_kaspa_node_url() -> Option<String> {
    let url = std::env::var("KASPA_NODE_URL").ok()?;
    let trimmed = url.trim();

    if trimmed.is_empty() {
        return None;
    }

    // Retire the legacy public testnet endpoint so stale local env files no longer
    // bypass Resolver-based discovery.
    if trimmed.contains("photon-10.kaspa.red") {
        eprintln!(
            "Ignoring legacy KASPA_NODE_URL={} and falling back to Kaspa Resolver",
            trimmed
        );
        return None;
    }

    Some(trimmed.to_string())
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

    // One-time migration: add escrow_address column if missing
    sqlx::query("ALTER TABLE matches ADD COLUMN IF NOT EXISTS escrow_address TEXT")
        .execute(&pool)
        .await
        .expect("Failed to add escrow_address column");
    eprintln!("✅ DB migration: escrow_address column ensured");

    // v0.2 migrations: deposit tracking + FaceID
    let v02_migrations = [
        "ALTER TABLE matches ADD COLUMN IF NOT EXISTS player_a_deposit_tx_hash TEXT",
        "ALTER TABLE matches ADD COLUMN IF NOT EXISTS player_b_deposit_tx_hash TEXT",
        "ALTER TABLE matches ADD COLUMN IF NOT EXISTS player_a_deposit_confirmed BOOLEAN DEFAULT FALSE",
        "ALTER TABLE matches ADD COLUMN IF NOT EXISTS player_b_deposit_confirmed BOOLEAN DEFAULT FALSE",
        "ALTER TABLE matches ADD COLUMN IF NOT EXISTS player_a_faceid_hash TEXT",
        "ALTER TABLE matches ADD COLUMN IF NOT EXISTS player_b_faceid_hash TEXT",
    ];
    for migration in &v02_migrations {
        if let Err(e) = sqlx::query(migration).execute(&pool).await {
            eprintln!("⚠️ v0.2 migration skipped (may already exist): {}", e);
        }
    }
    eprintln!("✅ DB migration: v0.2 columns ensured (deposit tracking + FaceID)");

    // v0.3 migrations: FACEIT cache columns
    let v03_migrations = [
        "ALTER TABLE faceit_links ADD COLUMN IF NOT EXISTS faceit_elo INTEGER",
        "ALTER TABLE faceit_links ADD COLUMN IF NOT EXISTS faceit_skill_level INTEGER",
        "ALTER TABLE faceit_links ADD COLUMN IF NOT EXISTS faceit_cache_updated_at TIMESTAMPTZ",
    ];
    for migration in &v03_migrations {
        if let Err(e) = sqlx::query(migration).execute(&pool).await {
            eprintln!("⚠️ v0.3 migration skipped (may already exist): {}", e);
        }
    }
    eprintln!("✅ DB migration: v0.3 FACEIT cache columns ensured");

    // v0.4 migrations: payment detection system
    // 1. Extend match_status enum (ADD VALUE IF NOT EXISTS is idempotent)
    let enum_variants = [
        "FUNDED", "PAID_OUT", "DISPUTED", "RESOLVING", "IN_GAME", "DRAFT",
    ];
    for variant in &enum_variants {
        let sql = format!(
            "DO $$ BEGIN \
             IF NOT EXISTS (SELECT 1 FROM pg_enum WHERE enumtypid = 'match_status'::regtype AND enumlabel = '{}') \
             THEN ALTER TYPE match_status ADD VALUE '{}'; END IF; END $$",
            variant, variant
        );
        if let Err(e) = sqlx::query(&sql).execute(&pool).await {
            eprintln!("⚠️ v0.4 enum migration skipped for {}: {}", variant, e);
        }
    }
    eprintln!("✅ DB migration: v0.4 match_status enum variants ensured");

    // 2. Create payments table for on-chain UTXO tracking
    let create_payments = "
        CREATE TABLE IF NOT EXISTS payments (
            id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
            match_id UUID NOT NULL REFERENCES matches(id) ON DELETE CASCADE,
            player_id UUID REFERENCES users(id),
            player_role TEXT NOT NULL CHECK (player_role IN ('A', 'B')),
            tx_id TEXT NOT NULL,
            amount_sompi BIGINT NOT NULL,
            block_daa_score BIGINT NOT NULL DEFAULT 0,
            confirmations INTEGER NOT NULL DEFAULT 0,
            created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
            UNIQUE(tx_id, match_id)
        )";
    if let Err(e) = sqlx::query(create_payments).execute(&pool).await {
        eprintln!("⚠️ v0.4 payments table migration failed: {}", e);
    } else {
        eprintln!("✅ DB migration: v0.4 payments table ensured");
    }

    let create_wallet_login_challenges = "
        CREATE TABLE IF NOT EXISTS wallet_login_challenges (
            id UUID PRIMARY KEY,
            kaspa_address TEXT NOT NULL,
            challenge_message TEXT NOT NULL,
            expires_at TIMESTAMPTZ NOT NULL,
            used_at TIMESTAMPTZ,
            created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
        )";
    if let Err(e) = sqlx::query(create_wallet_login_challenges).execute(&pool).await {
        eprintln!("⚠️ wallet login challenge migration failed: {}", e);
    } else {
        eprintln!("✅ DB migration: wallet login challenge table ensured");
    }

    // 3. Add wager_amount_sompi to matches (mirrors stake_kas but preserves the domain field)
    let v04_columns = [
        "ALTER TABLE matches ADD COLUMN IF NOT EXISTS wager_amount_sompi BIGINT",
        "ALTER TABLE matches ADD COLUMN IF NOT EXISTS player_a_deposit_amount_sompi BIGINT",
        "ALTER TABLE matches ADD COLUMN IF NOT EXISTS player_b_deposit_amount_sompi BIGINT",
    ];
    for migration in &v04_columns {
        if let Err(e) = sqlx::query(migration).execute(&pool).await {
            eprintln!("⚠️ v0.4 column migration skipped (may already exist): {}", e);
        }
    }
    eprintln!("✅ DB migration: v0.4 payment tracking columns ensured");

    // v0.5 migrations: deposit integrity constraints
    // First: clear duplicate (match_id, player_role) rows that block index creation
    let v05_cleanup = "DELETE FROM payments p1 USING payments p2 \
         WHERE p1.ctid < p2.ctid \
         AND p1.match_id = p2.match_id \
         AND p1.player_role = p2.player_role";
    if let Err(e) = sqlx::query(v05_cleanup).execute(&pool).await {
        eprintln!("⚠️ v0.5 dedup skipped: {}", e);
    }
    let v05_migrations = [
        "CREATE UNIQUE INDEX IF NOT EXISTS uq_payments_match_player_role \
         ON payments (match_id, player_role)",
    ];
    for migration in &v05_migrations {
        if let Err(e) = sqlx::query(migration).execute(&pool).await {
            eprintln!("⚠️ v0.5 migration skipped: {}", e);
        }
    }
    eprintln!("✅ DB migration: v0.5 deposit integrity constraints ensured");

    // v0.6 migration: Redesign payments table for aggregate-per-role semantics.
    //
    // Old model (v0.5): one row per UTXO, UNIQUE(tx_id, match_id) + uq_payments_match_player_role
    //   → caused duplicate-key errors when multiple UTXOs were attributed to same role.
    //
    // New model (v0.6): one row per (match_id, player_role), aggregating all UTXOs for that role.
    //   → UPSERT with ON CONFLICT (match_id, player_role) DO UPDATE is now idempotent.
    let v06_migrations: &[&str] = &[
        // Drop conflicting per-tx constraint (the old unique index name varies)
        "ALTER TABLE payments DROP CONSTRAINT IF EXISTS payments_tx_id_match_id_key",
        // Drop old per-role index (created in v0.5)
        "DROP INDEX IF EXISTS uq_payments_match_player_role",
        // Drop old per-tx unique index if it exists under another name
        "DROP INDEX IF EXISTS payments_tx_id_match_id_idx",
        // Create new canonical unique index: one row per (match_id, player_role)
        "CREATE UNIQUE INDEX IF NOT EXISTS uq_payments_match_role ON payments (match_id, player_role)",
    ];
    for migration in v06_migrations {
        if let Err(e) = sqlx::query(migration).execute(&pool).await {
            eprintln!("⚠️ v0.6 migration skipped ({}): {}", migration.split_whitespace().take(4).collect::<Vec<_>>().join(" "), e);
        }
    }
    eprintln!("✅ DB migration: v0.6 payments aggregate-per-role constraints ensured");


    let (tx, _) = broadcast::channel(100);

    let auth_service = Arc::new(battle_core::auth::AuthService::new(pool.clone()));

    let faceit_config = battle_core::models::faceit::FaceitOAuthConfig {
        client_id: std::env::var("FACEIT_CLIENT_ID").expect("Missing FACEIT_CLIENT_ID"),
        client_secret: std::env::var("FACEIT_CLIENT_SECRET").expect("Missing FACEIT_CLIENT_SECRET"),
        redirect_uri: std::env::var("FACEIT_REDIRECT_URI").expect("Missing FACEIT_REDIRECT_URI"),
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
    let kaspa_network = std::env::var("KASPA_NETWORK").unwrap_or_else(|_| "testnet-10".to_string());
    let kaspa_mnemonic = std::env::var("KASPA_MNEMONIC").ok();

    let escrow_wallet = Arc::new(
        battle_kaspa::wallet::EscrowWallet::new(kaspa_mnemonic, &kaspa_network)
            .expect("Failed to initialize EscrowWallet"),
    );
    eprintln!("✅ EscrowWallet initialized (network: {})", kaspa_network);

    // Connect to Kaspa node via Resolver (or explicit URL if set)
    let kaspa_rpc: Option<Arc<dyn battle_kaspa::rpc::KaspaRpc>> =
        match battle_kaspa::rpc::RealKaspaClient::new_with_resolver(
            kaspa_node_url.as_deref(),
            &kaspa_network,
        ).await {
            Ok(client) => {
                eprintln!("✅ Connected to Kaspa node{}", 
                    kaspa_node_url.as_ref()
                        .map(|u| format!(": {}", u))
                        .unwrap_or_else(|| " (via Resolver)".to_string())
                );
                Some(Arc::new(client))
            }
            Err(e) => {
                eprintln!(
                    "⚠️ Kaspa RPC connection failed (escrow features disabled): {}",
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
    eprintln!(
        "🔍 TREASURY_MNEMONIC: {:?}",
        treasury_mnemonic_result
            .as_ref()
            .map(|s| format!("{}...", &s[..20.min(s.len())]))
    );
    let treasury_address = if let Ok(treasury_mnemonic) = treasury_mnemonic_result {
        let treasury_wallet =
            battle_kaspa::wallet::EscrowWallet::new(Some(treasury_mnemonic), &kaspa_network)
                .expect("Failed to initialize treasury wallet from TREASURY_MNEMONIC");
        let (addr, _) = treasury_wallet
            .derive_escrow_address("treasury-main")
            .expect("Failed to derive treasury address");
        let addr_str = addr.to_string();
        eprintln!("✅ Treasury address derived: {}", addr_str);
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
        eprintln!(
            "✅ PayoutService initialized (treasury: {})",
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

        // Derive oracle private key from ORACLE_PRIVATE_KEY env or deterministic fallback
        let oracle_sk_hex = std::env::var("ORACLE_PRIVATE_KEY").unwrap_or_else(|_| {
            // Deterministic fallback: SHA256("kaspabattle-oracle-v1")
            use sha2::{Digest, Sha256};
            let mut hasher = Sha256::new();
            hasher.update(b"kaspabattle-oracle-v1");
            hex::encode(hasher.finalize())
        });

        let oracle_sk_bytes: [u8; 32] = match hex::decode(&oracle_sk_hex) {
            Ok(bytes) if bytes.len() == 32 => bytes.try_into().unwrap(),
            _ => {
                eprintln!("⚠️ Invalid ORACLE_PRIVATE_KEY — MultisigEscrowService disabled");
                return None;
            }
        };

        match battle_kaspa::multisig::service::MultisigEscrowService::new(
            rpc.clone(),
            prefix,
            oracle_sk_bytes,
            treasury_address.clone(),
        ) {
            Ok(service) => {
                eprintln!("✅ MultisigEscrowService initialized (prefix: {:?})", prefix);
                Some(Arc::new(service))
            }
            Err(e) => {
                eprintln!("⚠️ MultisigEscrowService failed: {} — disabled", e);
                None
            }
        }
    });

    // ── FACEIT Data API Service (for profile/stats endpoints) ──
    let faceit_data_service = match std::env::var("FACEIT_DATA_API_KEY") {
        Ok(api_key) if !api_key.is_empty() => {
            eprintln!("✅ FaceitDataService initialized (API key set)");
            Some(Arc::new(battle_core::faceit_data::FaceitDataService::new(api_key)))
        }
        _ => {
            eprintln!("⚠️ FACEIT_DATA_API_KEY not set — /faceit/profile and /faceit/stats will be unavailable");
            None
        }
    };

    // ── BlockchainWatcher (per-player UTXO attribution + confirmation tracking) ──
    let blockchain_watcher = kaspa_rpc.as_ref().map(|rpc| {
        let watcher = battle_kaspa::watcher::BlockchainWatcher::new(
            rpc.clone(),
            std::time::Duration::from_secs(5),
        );
        eprintln!("✅ BlockchainWatcher initialized");
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

    let frontend_url_str =
        std::env::var("FRONTEND_URL").unwrap_or_else(|_| "http://localhost:5173".to_string());
    let frontend_url = frontend_url_str.parse::<HeaderValue>().unwrap_or_else(|_| {
        eprintln!("⚠️ Invalid FRONTEND_URL: {}", frontend_url_str);
        "http://localhost:5173".parse::<HeaderValue>().unwrap()
    });

    let cors = CorsLayer::new()
        .allow_origin(frontend_url)
        .allow_methods([Method::GET, Method::POST, Method::PUT, Method::DELETE])
        .allow_headers([AUTHORIZATION, CONTENT_TYPE])
        .allow_credentials(true);

    let app = Router::new()
        .nest("/api/v1", api::router())
        .route("/health", get(api::health))
        .route("/ws", get(api::ws_handler))
        .layer(cors)
        .with_state(state.clone());

    eprintln!("🚀 KaspaBattle API running on 0.0.0.0:8080");
    let addr = SocketAddr::from(([0, 0, 0, 0], 8080));
    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();

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
            eprintln!("🚀 Episode runner: initial catch-up poll (no delay)");

            // Wait for Kaspa node to be fully synced before starting deposit detection.
            // This runs in the background so it does NOT block the HTTP server.
            if let Some(ref rpc) = ep_rpc {
                eprintln!("⏳ Episode runner: waiting for Kaspa node to sync (timeout: 5 min)...");
                match rpc.wait_for_sync(std::time::Duration::from_secs(300)).await {
                    Ok(()) => eprintln!("✅ Episode runner: Kaspa node is synced and UTXO-indexed — starting deposit detection"),
                    Err(e) => {
                        eprintln!("⚠️ Episode runner: node sync wait failed: {} — will retry via health-check", e);
                    }
                }
            }
            loop {
                let active_ids: Vec<(uuid::Uuid,)> = sqlx::query_as(
                    "SELECT id FROM matches WHERE status IN \
                     ('OPEN', 'AWAITING_FUNDING', 'FUNDED', 'LOCKED')",
                )
                .fetch_all(&ep_pool)
                .await
                .unwrap_or_default();

                if !active_ids.is_empty() {
                    eprintln!(
                        "🔄 Episode runner: polling {} active match(es)",
                        active_ids.len()
                    );
                }

                // Periodic RPC health-check: detect stale connections early.
                // If DAA=0, the node is not ready — skip the poll cycle.
                // If the RPC call fails, ensure_connected() will auto-reconnect
                // on the next call inside MatchEpisode::execute().
                if let Some(ref rpc) = ep_rpc {
                    match rpc.get_current_daa_score().await {
                        Ok(daa) if daa == 0 => {
                            eprintln!("⚠️ Episode runner: DAA=0, node may not be ready — deposits won't be confirmed until node is synced");
                        }
                        Err(e) => {
                            eprintln!("🚨 Episode runner: RPC health-check failed: {} — reconnect will be attempted on next RPC call", e);
                        }
                        Ok(daa) => {
                            tracing::debug!(current_daa = daa, "Episode runner: RPC healthy");
                        }
                    }
                }

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
                            }
                        }
                        Err(e) => {
                            tracing::error!(match_id = %match_id, error = %e, "Episode init failed");
                        }
                    }
                }
                // Sleep after poll (not before) so first cycle runs immediately
                tokio::time::sleep(std::time::Duration::from_secs(5)).await;
            }
        }
    });

    // ── kdapp Engine + Proxy (v0.7 — on-chain Episode processing) ─────────
    // Runs parallel to the legacy episode-runner above.
    // Uses battle-kdapp's convenience function to hide Engine/Proxy internals.
    {
        let kdapp_network = std::env::var("KASPA_NETWORK")
            .unwrap_or_else(|_| "testnet-10".to_string());
        let kdapp_rpc_url = normalized_kaspa_node_url();

        match battle_kdapp::startup::spawn_kdapp_services(
            state.pool.clone(),
            state.tx.clone(),
            &kdapp_network,
            kdapp_rpc_url,
        ) {
            Ok(_handle) => {
                eprintln!("✅ kdapp Engine + Proxy started (network: {})", kdapp_network);
            }
            Err(e) => {
                eprintln!("⚠️ kdapp Engine + Proxy failed to start: {} — disabled", e);
            }
        }
    }

    axum::serve(listener, app).await.unwrap();
}

use axum::{
    http::{
        header::{AUTHORIZATION, CONTENT_TYPE},
        HeaderValue, Method,
    },
    routing::get,
    Router,
};
use dotenvy::dotenv;
use sqlx::postgres::PgPoolOptions;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::broadcast;
use tower_http::cors::CorsLayer;

mod api;
mod episodes;
mod models;
mod services;

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

    // Load .env from cwd first, then try parent (workspace root)
    if let Err(e) = dotenv() {
        eprintln!("⚠️ dotenv() failed: {}", e);
    }
    // Also try parent directory in case running from battle-api/ subdirectory
    if let Ok(cwd) = std::env::current_dir() {
        let parent_env = cwd.join("../.env");
        if parent_env.exists() {
            if let Err(e) = dotenvy::from_path(&parent_env) {
                eprintln!("⚠️ dotenvy::from_path() failed: {}", e);
            }
        }
    }

    let db_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let pool = PgPoolOptions::new()
        .max_connections(5)
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

    let (tx, _) = broadcast::channel(100);

    let auth_service = Arc::new(battle_core::auth::AuthService::new(pool.clone()));

    let faceit_config = battle_core::models::faceit::FaceitOAuthConfig {
        client_id: std::env::var("FACEIT_CLIENT_ID").expect("Missing FACEIT_CLIENT_ID"),
        client_secret: std::env::var("FACEIT_CLIENT_SECRET").expect("Missing FACEIT_CLIENT_SECRET"),
        redirect_uri: std::env::var("FACEIT_REDIRECT_URI").expect("Missing FACEIT_REDIRECT_URI"),
        auth_url: "https://accounts.faceit.com/oauth/authorize".to_string(),
        token_url: "https://api.faceit.com/auth/v1/oauth/token".to_string(),
        userinfo_url: "https://api.faceit.com/auth/v1/resources/userinfo".to_string(),
    };
    let faceit_service = Arc::new(battle_core::faceit_oauth::FaceitOAuthService::new(
        faceit_config,
        pool.clone(),
    ));

    // ── battle-kaspa: Initialize Kaspa escrow infrastructure ──
    let kaspa_node_url = std::env::var("KASPA_NODE_URL")
        .unwrap_or_else(|_| "wss://photon-10.kaspa.red/kaspa/testnet-10/wrpc/borsh".to_string());
    let kaspa_network = std::env::var("KASPA_NETWORK").unwrap_or_else(|_| "testnet-10".to_string());
    let kaspa_mnemonic = std::env::var("KASPA_MNEMONIC").ok();

    let escrow_wallet = Arc::new(
        battle_kaspa::wallet::EscrowWallet::new(kaspa_mnemonic, &kaspa_network)
            .expect("Failed to initialize EscrowWallet"),
    );
    eprintln!("✅ EscrowWallet initialized (network: {})", kaspa_network);

    // Connect to Kaspa node (optional — don't crash if node is unreachable during dev)
    let kaspa_rpc: Option<Arc<dyn battle_kaspa::rpc::KaspaRpc>> =
        match battle_kaspa::rpc::RealKaspaClient::new(&kaspa_node_url, &kaspa_network).await {
            Ok(client) => {
                eprintln!("✅ Connected to Kaspa node: {}", kaspa_node_url);
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

    axum::serve(listener, app).await.unwrap();
}

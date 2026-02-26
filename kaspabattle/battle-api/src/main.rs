use actix_cors::Cors;
use actix_governor::{Governor, GovernorConfigBuilder};
use actix_web::{middleware, web, App, HttpServer};
use std::sync::Arc;
use std::time::Duration;

use battle_api::db;
use battle_api::oracle_auth;
use battle_api::routes;
use battle_api::watcher_task;

use battle_kaspa::escrow::EscrowService;
use battle_kaspa::mock::MockKaspaClient;
use battle_kaspa::payout::PayoutService;
use battle_kaspa::rpc::{KaspaRpc, RealKaspaClient};
use battle_kaspa::wallet::EscrowWallet;
use battle_kaspa::watcher::BlockchainWatcher;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();
    env_logger::init_from_env(env_logger::Env::default().default_filter_or("info"));

    log::info!("🎮 KaspaBattle Backend v0.3.0 (Real Testnet-10) startet...");

    // F-002: Initialize Oracle API keys from ORACLE_API_KEYS env var
    let key_count = oracle_auth::init_oracle_keys().unwrap_or(0);
    log::info!("🔑 Oracle auth: {} key(s) configured", key_count);

    // === Database ===
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "sqlite:kaspabattle.db?mode=rwc".to_string());

    let db = Arc::new(db::Database::new(&database_url).await?);
    log::info!("✅ SQLite Datenbank bereit (inkl. Migration 002)");

    // === AuthService ===
    let rusqlite_conn = rusqlite::Connection::open("kaspabattle.db")?;
    let auth_db = Arc::new(tokio::sync::Mutex::new(rusqlite_conn));
    let auth = Arc::new(battle_core::auth::AuthService::new(auth_db.clone()).await?);
    log::info!("✅ AuthService bereit (inkl. Migration 004)");

    // === Faceit OAuth ===
    let faceit_config = battle_core::models::faceit::FaceitOAuthConfig {
        client_id: std::env::var("FACEIT_CLIENT_ID").unwrap_or_else(|_| "client_id".to_string()),
        client_secret: std::env::var("FACEIT_CLIENT_SECRET")
            .unwrap_or_else(|_| "secret".to_string()),
        redirect_uri: std::env::var("FACEIT_REDIRECT_URI")
            .unwrap_or_else(|_| "http://localhost:8080/api/v1/faceit/callback".to_string()),
        auth_url: "https://accounts.faceit.com/authorize".to_string(),
        token_url: "https://api.faceit.com/auth/v1/oauth/token".to_string(),
        userinfo_url: "https://api.faceit.com/auth/v1/resources/userinfo".to_string(),
    };
    let faceit_oauth = Arc::new(battle_core::faceit_oauth::FaceitOAuthService::new(
        faceit_config,
        auth_db.clone(),
    ));
    log::info!("✅ FaceitOAuthService bereit");

    // F-015: FACEIT_API_KEY is mandatory in production — provide fallback for dev.
    let faceit_api_key = std::env::var("FACEIT_API_KEY").unwrap_or_else(|_| {
        let env_mode = std::env::var("RUST_ENV").unwrap_or_else(|_| "development".to_string());
        if env_mode == "production" {
            panic!("FACEIT_API_KEY must be set in production mode!");
        }
        log::warn!("⚠️ FACEIT_API_KEY not set — using dummy key for development");
        "dummy_key".to_string()
    });
    let oracle_signing_key = ed25519_dalek::SigningKey::generate(&mut rand::rngs::OsRng);
    let oracle = Arc::new(battle_core::oracle::faceit::FaceitOracleService::new(
        faceit_api_key.clone(),
        oracle_signing_key,
    ));
    log::info!("✅ FaceitOracleService bereit");

    let faceit_data = Arc::new(battle_core::faceit_data::FaceitDataService::new(
        faceit_api_key,
    ));
    log::info!("✅ FaceitDataService bereit");

    // === Kaspa Client ===
    let kaspa_node_url = std::env::var("KASPA_NODE_URL")
        .unwrap_or_else(|_| "wss://photon-10.kaspa.red/kaspa/testnet-10/wrpc/borsh".to_string());
    let network = std::env::var("KASPA_NETWORK").unwrap_or_else(|_| "testnet-10".to_string());

    let kaspa: Arc<dyn KaspaRpc> = match RealKaspaClient::new(&kaspa_node_url, &network).await {
        Ok(client) => {
            log::info!("✅ Kaspa Testnet-10 verbunden (wRPC): {}", kaspa_node_url);
            Arc::new(client)
        }
        Err(e) => {
            log::warn!(
                "⚠️  Real Kaspa node not reachable ({}) – using MockKaspaClient for development",
                e
            );
            Arc::new(MockKaspaClient::new())
        }
    };

    // === Escrow Wallet & Service ===
    let mnemonic = std::env::var("KASPA_MNEMONIC").ok();

    let wallet = Arc::new(EscrowWallet::new(mnemonic, &network)?);
    log::info!("✅ EscrowWallet bereit (Network: {})", network);

    let escrow = Arc::new(EscrowService::new(wallet.clone(), kaspa.clone()));

    // === Blockchain Watcher ===
    let poll_interval = Duration::from_secs(
        std::env::var("WATCHER_POLL_SECONDS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(3),
    );

    let watcher = Arc::new(BlockchainWatcher::new(kaspa.clone(), poll_interval));

    // F-015: TREASURY_ADDRESS is mandatory in production — provide fallback for dev.
    let treasury_address = std::env::var("TREASURY_ADDRESS").unwrap_or_else(|_| {
        let env_mode = std::env::var("RUST_ENV").unwrap_or_else(|_| "development".to_string());
        if env_mode == "production" {
            panic!("TREASURY_ADDRESS must be set in production mode!");
        }
        log::warn!("⚠️ TREASURY_ADDRESS not set — using dev-only dummy address");
        "kaspatest:qz4mv06zlvay4l8k3m3m00000000000000000000000000000000g7q3r4".to_string()
    });
    let payout = Arc::new(PayoutService::new(
        kaspa.clone(),
        treasury_address,
        // F-016: Keys are registered dynamically by create_challenge_escrow after
        // EscrowWallet derives each address. This HashMap starts empty and is
        // populated at runtime.
        std::collections::HashMap::new(),
    ));

    // === Start Watcher Background Task ===
    // F-006: payout service passed in for timeout refund
    let _watcher_handle = watcher_task::start_watcher_task(
        db.clone(),
        watcher.clone(),
        payout.clone(),
        poll_interval,
    );

    // === App State ===
    let app_state = web::Data::new(routes::AppState {
        db,
        kaspa,
        wallet,
        watcher,
        payout,
        escrow,
        auth,
        faceit_oauth,
        faceit_data,
        oracle,
    });

    let port = std::env::var("PORT").unwrap_or_else(|_| "8080".to_string());
    let bind_addr = format!("0.0.0.0:{}", port);
    log::info!("🚀 HTTP-Server startet auf {}", bind_addr);

    HttpServer::new(move || {
        // F-005: Read allowed origins from ENV, no wildcard in production.
        let cors = {
            let env_mode = std::env::var("RUST_ENV").unwrap_or_else(|_| "development".to_string());
            if env_mode == "production" {
                let origins_raw = std::env::var("ALLOWED_ORIGINS")
                    .unwrap_or_else(|_| "https://kaspabattle.com".to_string());
                let mut cors = Cors::default().allow_any_method().allow_any_header();
                for origin in origins_raw.split(',') {
                    let o = origin.trim().to_string();
                    if !o.is_empty() {
                        cors = cors.allowed_origin(&o);
                    }
                }
                cors
            } else {
                // Development: permissive for local testing
                Cors::default()
                    .allow_any_origin()
                    .allow_any_method()
                    .allow_any_header()
            }
        };

        // F-006: Rate limiting — 20 requests per 60 seconds per peer IP.
        let governor_conf = GovernorConfigBuilder::default()
            .seconds_per_request(3) // refill 1 token every 3 seconds
            .burst_size(20) // max 20 tokens in bucket
            .finish()
            .unwrap();

        App::new()
            .wrap(cors)
            .wrap(Governor::new(&governor_conf)) // F-006: rate limiting
            .wrap(middleware::Logger::default())
            .app_data(app_state.clone())
            .service(routes::health)
            .service(routes::create_match)
            .service(routes::list_open_matches)
            .service(routes::get_match_status)
            .service(routes::join_match)
            .service(routes::cancel_match)
            .service(routes::get_escrow_status)
            .service(routes::resolve_match)
            .service(routes::dispute_match)
            .service(routes::node_status)
            .service(routes::create_challenge_escrow)
            .service(routes::get_challenge_deposits)
            .service(routes::cancel_challenge)
            .configure(routes::auth::auth_routes)
            .configure(routes::faceit::faceit_routes)
            .service(routes::match_result::trigger_payout)
            .configure(routes::oracle::oracle_routes)
    })
    .bind(bind_addr)?
    .run()
    .await?;

    Ok(())
}

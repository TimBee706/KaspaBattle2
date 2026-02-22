use actix_cors::Cors;
use actix_web::{middleware, web, App, HttpServer};
use std::sync::Arc;
use std::time::Duration;

use battle_api::db;
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
    env_logger::init_from_env(env_logger::Env::default().default_filter_or("info"));

    log::info!("🎮 KaspaBattle Backend v0.3.0 (Real Testnet-10) startet...");

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
            .unwrap_or_else(|_| "http://localhost:3000/api/v1/faceit/callback".to_string()),
        auth_url: "https://accounts.faceit.com/authorize".to_string(),
        token_url: "https://api.faceit.com/auth/v1/oauth/token".to_string(),
        userinfo_url: "https://api.faceit.com/auth/v1/resources/userinfo".to_string(),
    };
    let faceit_oauth = Arc::new(battle_core::faceit_oauth::FaceitOAuthService::new(
        faceit_config,
        auth_db.clone(),
    ));
    log::info!("✅ FaceitOAuthService bereit");

    // === Oracle Service (V2) ===
    let oracle_signing_key = ed25519_dalek::SigningKey::generate(&mut rand::rngs::OsRng);
    let oracle = Arc::new(battle_core::oracle::faceit::FaceitOracleService::new(
        std::env::var("FACEIT_API_KEY").unwrap_or_else(|_| "faceit_api_key".to_string()),
        oracle_signing_key,
    ));
    log::info!("✅ FaceitOracleService bereit");

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

    // === Payout Manager ===
    let payout = Arc::new(PayoutService::new(
        kaspa.clone(),
        std::env::var("TREASURY_ADDRESS")
            .unwrap_or_else(|_| "kaspatest:qztreasurydummy123456789".to_string()),
        std::collections::HashMap::new(),
    ));

    // === Start Watcher Background Task ===
    let _watcher_handle =
        watcher_task::start_watcher_task(db.clone(), watcher.clone(), poll_interval);

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
        oracle,
    });

    let bind_addr = "0.0.0.0:3000";
    log::info!("🚀 HTTP-Server startet auf {}", bind_addr);

    HttpServer::new(move || {
        let cors = Cors::default()
            .allow_any_origin()
            .allow_any_method()
            .allow_any_header();

        App::new()
            .wrap(cors)
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

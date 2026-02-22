use actix_cors::Cors;
use actix_web::{middleware, web, App, HttpServer};
use std::sync::Arc;
use std::time::Duration;

mod db;
mod routes;
mod watcher_task;

use battle_kaspa::escrow::EscrowService;
use battle_kaspa::mock::MockKaspaClient;
use battle_kaspa::payout::PayoutManager;
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
    let payout = Arc::new(PayoutManager::new(escrow.clone()));

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
            // Step 2: New endpoints
            .service(routes::get_escrow_status)
            .service(routes::resolve_match)
            .service(routes::node_status)
            // Step 3: V1 endpoints
            .service(routes::create_challenge_escrow)
            .service(routes::get_challenge_deposits)
            .service(routes::cancel_challenge)
    })
    .bind(bind_addr)?
    .run()
    .await?;

    Ok(())
}

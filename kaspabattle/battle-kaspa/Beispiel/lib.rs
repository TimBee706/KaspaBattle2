// ===== MODULE & IMPORTS =====
pub mod wallet;
pub mod models;

pub use models::*;           // enthält jetzt auch KaspaWallet
pub use models::KaspaWallet; // optional explizit
use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use std::sync::Arc;
use tower_http::cors::CorsLayer;
use tracing::{info, error};
use tracing_subscriber;
use kaspa_consensus_core::network::{NetworkId, NetworkType};

// ===== CUSTOM ERROR TYPE =====
struct AppError(anyhow::Error);

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        error!("Application error: {}", self.0);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Error: {}", self.0)
        ).into_response()
    }
}

impl<E> From<E> for AppError
where
    E: Into<anyhow::Error>,
{
    fn from(err: E) -> Self {
        Self(err.into())
    }
}

// ===== MAIN FUNKTION =====
#[tokio::main]
async fn main() {
    // Logging initialisieren
    tracing_subscriber::fmt::init();
    
    // Option 1: Mainnet (Production)
    // let network_id = NetworkId::mainnet();
    
    // Option 2: Testnet-11 (Aktuelles Testnet)
    let network_id = NetworkId::with_suffix(NetworkType::Testnet, 10);
    
    // Option 3: Simnet (Lokale Entwicklung)
    // let network_id = NetworkId::with_suffix(NetworkType::Simnet, 0);
    
    info!("🔧 Initializing Kaspa Wallet...");
    
    // Wallet initialisieren
    let wallet = match KaspaWallet::new_with_url(network_id, "wss://photon-10.kaspa.red/kaspa/testnet-10/wrpc/borsh").await {
        Ok(w) => {
            info!("✅ Wallet initialized successfully");
            w
        },
        Err(e) => {
            error!("❌ Failed to initialize wallet: {}", e);
            panic!("Cannot start without wallet connection");
        }
    };
    
    // Router bauen
    let app = Router::new()
        .route("/health", get(health_check))
        .route("/balance", get(get_balance))
        .route("/create-account", post(create_account))
        .route("/send_tx", post(create_timelock_tx))
        .route("/mnemonic", get(get_mnemonic))
        .with_state(Arc::new(wallet))
        .layer(CorsLayer::permissive());
    
    // Server starten
    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080")
        .await
        .expect("Failed to bind to port 8080");
    
    info!("🚀 Kaspa Scheduler Service running on http://localhost:8080");
    info!("📖 Health check: http://localhost:8080/health");
    info!("🔑 Create account: POST http://localhost:8080/create-account");
    info!("🔑 Get mnemonic: GET http://localhost:8080/mnemonic");
    info!("🔑 Send transaction: POST http://localhost:8080/send_tx");
    
    axum::serve(listener, app)
        .await
        .expect("Server failed");
}

// ===== HANDLER FUNKTIONEN =====

/// GET /health - Server Health Check
async fn health_check(
    State(_wallet): State<Arc<KaspaWallet>>
) -> Json<HealthResponse> {
    info!("Health check requested");
    Json(HealthResponse {
        status: "ok".to_string(),
        node_connected: true,  // TODO: Prüfe tatsächliche Node-Verbindung
    })
}

/// GET /balance - Account Balance
async fn get_balance(
    State(wallet): State<Arc<KaspaWallet>>
) -> Result<Json<BalanceResponse>, AppError> {
    info!("Balance check requested");
    
    match wallet.get_balance().await {
        Ok(balance) => {
            info!("Balance retrieved: {} KAS", balance);
            Ok(Json(BalanceResponse {
                available: balance,
                total: balance,
            }))
        },
        Err(e) => {
            error!("Balance check failed: {}", e);
            // Gebe hilfreichen Fehler zurück
            Err(AppError(anyhow::anyhow!(
                "Balance check not available. Use Kaspa CLI: kaspa-cli wallet balance"
            )))
        }
    }
}

/// POST /create-account - Create New Account
async fn create_account(
    State(wallet): State<Arc<KaspaWallet>>,
    Json(payload): Json<CreateAccountRequest>,
) -> Result<Json<CreateAccountResponse>, AppError> {
    info!("Create account requested");
    
    let result = wallet.create_account(payload.mnemonic).await?;
    let mnemonic = wallet.get_mnemonic().await?;
    
    info!("Account created successfully");
    
    Ok(Json(CreateAccountResponse {
        status: result,
        mnemonic,
    }))
}


/// GET /mnemonic - Get Stored Mnemonic
async fn get_mnemonic(
    State(wallet): State<Arc<KaspaWallet>>
) -> Result<Json<MnemonicResponse>, AppError> {
    info!("Mnemonic retrieval requested");
    
    let mnemonic = wallet.get_mnemonic().await?;
    
    Ok(Json(MnemonicResponse {
        mnemonic,
    }))
}

/// POST /timelock-tx - Create Time-Locked Transaction
async fn create_timelock_tx(
    State(wallet): State<Arc<KaspaWallet>>,
    Json(payload): Json<CreateTimelockRequest>,
) -> Result<Json<TimelockResponse>, AppError> {
    info!(
        "Timelock TX requested: {} -> {} sompi @ {}",
        payload.destination, payload.amount, payload.locktime
    );
    
    match wallet.create_timelock_tx(
        payload.destination.clone(),
        payload.amount,
        payload.locktime
    ).await {
        Ok(tx) => {
            info!("Transaction created: {}", tx.tx_id);
            Ok(Json(tx))
        },
        Err(e) => {
            error!("Transaction creation failed: {}", e);
            Err(AppError(anyhow::anyhow!(
                "Transaction creation not available. Use Kaspa CLI: \
                 kaspa-cli wallet send --to {} --amount {}",
                payload.destination, payload.amount
            )))
        }
    }
}

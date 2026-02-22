use crate::db::Database;
use actix_web::{get, post, web, HttpResponse, Responder};
use battle_core::match_state::{transition, MatchAction, MatchState};
use battle_core::types::{
    GameType, DEPOSIT_TIMEOUT_MINUTES, MAX_WAGER_KAS, MIN_WAGER_KAS, SOMPI_PER_KAS,
};
use battle_kaspa::escrow::EscrowService;
use battle_kaspa::payout::PayoutManager;
use battle_kaspa::rpc::KaspaRpc;
use battle_kaspa::wallet::EscrowWallet;
use battle_kaspa::watcher::BlockchainWatcher;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

pub struct AppState {
    pub db: Arc<Database>,
    pub kaspa: Arc<dyn KaspaRpc>,
    pub wallet: Arc<EscrowWallet>,
    pub watcher: Arc<BlockchainWatcher>,
    pub payout: Arc<PayoutManager>,
    pub escrow: Arc<EscrowService>,
}

// === Request / Response DTOs ===

#[derive(Debug, Deserialize)]
pub struct CreateMatchRequest {
    pub player_id: String,
    pub kaspa_address: String,
    pub display_name: Option<String>,
    pub game_type: String,
    pub wager_kas: u64,
}

#[derive(Debug, Serialize)]
pub struct CreateMatchResponse {
    pub match_id: String,
    pub escrow_address: String,
    pub wager_kas: f64,
    pub wager_sompi: u64,
    pub game_type: String,
    pub state: String,
    pub timeout_at: String,
}

#[derive(Debug, Serialize)]
pub struct LobbyMatchEntry {
    pub match_id: String,
    pub game_type: String,
    pub wager_kas: f64,
    pub player_a_name: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Serialize)]
pub struct LobbyResponse {
    pub matches: Vec<LobbyMatchEntry>,
    pub count: usize,
}

#[derive(Debug, Serialize)]
pub struct PlayerResponse {
    pub id: String,
    pub name: Option<String>,
    pub address: String,
}

#[derive(Debug, Serialize)]
pub struct MatchStatusResponse {
    pub match_id: String,
    pub state: String,
    pub game_type: String,
    pub player_a: PlayerResponse,
    pub player_b: Option<PlayerResponse>,
    pub wager_kas: f64,
    pub wager_sompi: u64,
    pub escrow_address: String,
    pub created_at: String,
}

#[derive(Debug, Deserialize)]
pub struct JoinMatchRequest {
    pub player_id: String,
    pub kaspa_address: String,
    pub display_name: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct JoinMatchResponse {
    pub match_id: String,
    pub state: String,
    pub escrow_address: String,
    pub wager_kas: f64,
    pub message: String,
}

#[derive(Debug, Deserialize)]
pub struct CancelMatchRequest {
    pub player_id: String,
    pub reason: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct CancelMatchResponse {
    pub match_id: String,
    pub state: String,
    pub message: String,
}

#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub error: String,
}

#[derive(Debug, Serialize)]
pub struct HealthResponse {
    pub status: String,
    pub version: String,
}

// === Step 2: New DTOs ===

#[derive(Debug, Serialize)]
pub struct EscrowStatusResponse {
    pub match_id: String,
    pub escrow_address: String,
    pub expected_total: u64,
    pub expected_total_kas: f64,
    pub current_balance: u64,
    pub current_balance_kas: f64,
    pub player_a_deposited: bool,
    pub player_b_deposited: bool,
    pub utxo_count: u32,
    pub status: String,
}

#[derive(Debug, Deserialize)]
pub struct ResolveMatchRequest {
    pub winner_id: String,
    pub reported_by: String,
}

#[derive(Debug, Serialize)]
pub struct PayoutResponse {
    pub winner_amount_kas: f64,
    pub protocol_fee_kas: f64,
    pub oracle_fee_kas: f64,
    pub payout_tx_hash: String,
}

#[derive(Debug, Serialize)]
pub struct ResolveMatchResponse {
    pub match_id: String,
    pub state: String,
    pub winner_id: String,
    pub payout: Option<PayoutResponse>,
}

#[derive(Debug, Serialize)]
pub struct NodeStatusResponse {
    pub server_version: String,
    pub is_synced: bool,
    pub is_utxo_indexed: bool,
    pub connected: bool,
    pub platform_address: String,
}

// === API V1 DTOs ===

#[derive(Debug, Serialize)]
pub struct EscrowCreateResponse {
    pub challenge_id: String,
    pub escrow_address: String,
    pub wager_amount_sompi: u64,
    pub wager_amount_kas: f64,
}

#[derive(Debug, Serialize)]
pub struct DepositStatusResponse {
    pub challenge_id: String,
    pub status: String,
    pub deposited_sompi: u64,
    pub required_sompi: u64,
    pub player_a_deposited: bool,
    pub player_b_deposited: bool,
    pub utxo_count: u32,
}

#[derive(Debug, Deserialize)]
pub struct CancelChallengeRequest {
    pub reason: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct CancelChallengeResponse {
    pub challenge_id: String,
    pub message: String,
    pub refund_tx_a: Option<String>,
    pub refund_tx_b: Option<String>,
}

// === Endpoints ===

#[get("/api/health")]
pub async fn health() -> impl Responder {
    HttpResponse::Ok().json(HealthResponse {
        status: "ok".to_string(),
        version: "0.2.0".to_string(),
    })
}

#[post("/api/matches")]
pub async fn create_match(
    state: web::Data<AppState>,
    body: web::Json<CreateMatchRequest>,
) -> impl Responder {
    // Validate game type
    let game_type = match GameType::from_str(&body.game_type) {
        Some(gt) => gt,
        None => {
            return HttpResponse::BadRequest().json(ErrorResponse {
                error: format!("Unknown game type: {}", body.game_type),
            });
        }
    };

    // Validate wager
    if body.wager_kas < MIN_WAGER_KAS || body.wager_kas > MAX_WAGER_KAS {
        return HttpResponse::BadRequest().json(ErrorResponse {
            error: format!(
                "Wager {} KAS not in range [{}, {}]",
                body.wager_kas, MIN_WAGER_KAS, MAX_WAGER_KAS
            ),
        });
    }

    // Generate match ID
    let match_id = uuid::Uuid::new_v4().to_string();

    // Generate real escrow address via EscrowWallet (BIP44)
    let escrow_address = match state.wallet.get_escrow_address_for_challenge(&match_id) {
        Ok(addr) => addr.to_string(),
        Err(e) => {
            log::error!("Failed to generate escrow address: {}", e);
            return HttpResponse::InternalServerError().json(ErrorResponse {
                error: "Failed to generate escrow address".to_string(),
            });
        }
    };

    // Calculate timeout
    let timeout_at = chrono::Utc::now() + chrono::Duration::minutes(DEPOSIT_TIMEOUT_MINUTES);
    let timeout_str = timeout_at.format("%Y-%m-%dT%H:%M:%SZ").to_string();

    // Convert wager to sompi
    let wager_sompi = body.wager_kas * SOMPI_PER_KAS;

    // Initial state
    let match_state = MatchState::WaitingForOpponent;

    // Save to database
    if let Err(e) = state
        .db
        .create_match(
            &match_id,
            &body.player_id,
            &body.kaspa_address,
            body.display_name.as_deref(),
            wager_sompi as i64,
            &escrow_address,
            game_type.as_str(),
            &match_state,
            &timeout_str,
        )
        .await
    {
        log::error!("Failed to create match: {}", e);
        return HttpResponse::InternalServerError().json(ErrorResponse {
            error: "Failed to create match".to_string(),
        });
    }

    log::info!(
        "Match created: {} by {} ({} KAS, {})",
        match_id,
        body.player_id,
        body.wager_kas,
        game_type.as_str()
    );

    HttpResponse::Created().json(CreateMatchResponse {
        match_id,
        escrow_address,
        wager_kas: body.wager_kas as f64,
        wager_sompi,
        game_type: game_type.as_str().to_string(),
        state: "WaitingForOpponent".to_string(),
        timeout_at: timeout_str,
    })
}

#[get("/api/matches")]
pub async fn list_open_matches(state: web::Data<AppState>) -> impl Responder {
    match state.db.get_open_matches().await {
        Ok(rows) => {
            let count = rows.len();
            let matches: Vec<LobbyMatchEntry> = rows
                .into_iter()
                .map(|row| LobbyMatchEntry {
                    match_id: row.match_id,
                    game_type: row.game_type,
                    wager_kas: row.wager_sompi as f64 / SOMPI_PER_KAS as f64,
                    player_a_name: row.player_a_name,
                    created_at: row.created_at,
                })
                .collect();

            HttpResponse::Ok().json(LobbyResponse { matches, count })
        }
        Err(e) => {
            log::error!("Failed to list matches: {}", e);
            HttpResponse::InternalServerError().json(ErrorResponse {
                error: "Failed to list matches".to_string(),
            })
        }
    }
}

#[get("/api/matches/{match_id}")]
pub async fn get_match_status(
    state: web::Data<AppState>,
    path: web::Path<String>,
) -> impl Responder {
    let match_id = path.into_inner();

    match state.db.get_match(&match_id).await {
        Ok(Some(row)) => {
            let wager_sompi = row.wager_sompi as u64;
            let player_b = match (row.player_b_id, row.player_b_addr) {
                (Some(id), Some(addr)) => Some(PlayerResponse {
                    id,
                    name: row.player_b_name,
                    address: addr,
                }),
                _ => None,
            };

            HttpResponse::Ok().json(MatchStatusResponse {
                match_id: row.match_id,
                state: row.state,
                game_type: row.game_type,
                player_a: PlayerResponse {
                    id: row.player_a_id,
                    name: row.player_a_name,
                    address: row.player_a_addr,
                },
                player_b,
                wager_kas: wager_sompi as f64 / SOMPI_PER_KAS as f64,
                wager_sompi,
                escrow_address: row.escrow_address,
                created_at: row.created_at,
            })
        }
        Ok(None) => HttpResponse::NotFound().json(ErrorResponse {
            error: "Match not found".to_string(),
        }),
        Err(e) => {
            log::error!("Failed to get match {}: {}", match_id, e);
            HttpResponse::InternalServerError().json(ErrorResponse {
                error: "Failed to retrieve match".to_string(),
            })
        }
    }
}

#[post("/api/matches/{match_id}/join")]
pub async fn join_match(
    state: web::Data<AppState>,
    path: web::Path<String>,
    body: web::Json<JoinMatchRequest>,
) -> impl Responder {
    let match_id = path.into_inner();

    // Load match from DB
    let row = match state.db.get_match(&match_id).await {
        Ok(Some(row)) => row,
        Ok(None) => {
            return HttpResponse::NotFound().json(ErrorResponse {
                error: "Match not found".to_string(),
            });
        }
        Err(e) => {
            log::error!("Failed to load match {}: {}", match_id, e);
            return HttpResponse::InternalServerError().json(ErrorResponse {
                error: "Failed to load match".to_string(),
            });
        }
    };

    // Deserialize current state
    let current_state: MatchState = match serde_json::from_str(&row.state_json) {
        Ok(s) => s,
        Err(e) => {
            log::error!("Failed to deserialize match state: {}", e);
            return HttpResponse::InternalServerError().json(ErrorResponse {
                error: "Invalid match state".to_string(),
            });
        }
    };

    // Apply state machine transition
    let action = MatchAction::Join {
        player_id: body.player_id.clone(),
        kaspa_address: body.kaspa_address.clone(),
    };

    let new_state = match transition(
        &current_state,
        &action,
        &row.player_a_id,
        row.player_b_id.as_deref(),
    ) {
        Ok(s) => s,
        Err(e) => {
            return HttpResponse::BadRequest().json(ErrorResponse {
                error: e.to_string(),
            });
        }
    };

    // Update DB
    if let Err(e) = state
        .db
        .join_match(
            &match_id,
            &body.player_id,
            &body.kaspa_address,
            body.display_name.as_deref(),
            &new_state,
        )
        .await
    {
        log::error!("Failed to update match {} after join: {}", match_id, e);
        return HttpResponse::InternalServerError().json(ErrorResponse {
            error: "Failed to update match".to_string(),
        });
    }

    let wager_kas = row.wager_sompi as f64 / SOMPI_PER_KAS as f64;

    log::info!("Player {} joined match {}", body.player_id, match_id);

    HttpResponse::Ok().json(JoinMatchResponse {
        match_id,
        state: new_state.state_name().to_string(),
        escrow_address: row.escrow_address.clone(),
        wager_kas,
        message: format!(
            "Beigetreten! Sende {} KAS an: {}",
            wager_kas, row.escrow_address
        ),
    })
}

#[post("/api/matches/{match_id}/cancel")]
pub async fn cancel_match(
    state: web::Data<AppState>,
    path: web::Path<String>,
    body: web::Json<CancelMatchRequest>,
) -> impl Responder {
    let match_id = path.into_inner();

    // Load match from DB
    let row = match state.db.get_match(&match_id).await {
        Ok(Some(row)) => row,
        Ok(None) => {
            return HttpResponse::NotFound().json(ErrorResponse {
                error: "Match not found".to_string(),
            });
        }
        Err(e) => {
            log::error!("Failed to load match {}: {}", match_id, e);
            return HttpResponse::InternalServerError().json(ErrorResponse {
                error: "Failed to load match".to_string(),
            });
        }
    };

    // Deserialize current state
    let current_state: MatchState = match serde_json::from_str(&row.state_json) {
        Ok(s) => s,
        Err(e) => {
            log::error!("Failed to deserialize match state: {}", e);
            return HttpResponse::InternalServerError().json(ErrorResponse {
                error: "Invalid match state".to_string(),
            });
        }
    };

    let reason = body
        .reason
        .clone()
        .unwrap_or_else(|| "No reason given".to_string());

    // Apply state machine transition
    let action = MatchAction::Cancel {
        player_id: body.player_id.clone(),
        reason: reason.clone(),
    };

    let new_state = match transition(
        &current_state,
        &action,
        &row.player_a_id,
        row.player_b_id.as_deref(),
    ) {
        Ok(s) => s,
        Err(e) => {
            return HttpResponse::BadRequest().json(ErrorResponse {
                error: e.to_string(),
            });
        }
    };

    // Update DB
    if let Err(e) = state.db.update_match_state(&match_id, &new_state).await {
        log::error!("Failed to update match {} after cancel: {}", match_id, e);
        return HttpResponse::InternalServerError().json(ErrorResponse {
            error: "Failed to update match".to_string(),
        });
    }

    log::info!(
        "Match {} cancelled by {}: {}",
        match_id,
        body.player_id,
        reason
    );

    HttpResponse::Ok().json(CancelMatchResponse {
        match_id,
        state: new_state.state_name().to_string(),
        message: format!("Match abgebrochen: {}", reason),
    })
}

// === Step 2: New Endpoints ===

#[get("/api/matches/{match_id}/escrow")]
pub async fn get_escrow_status(
    state: web::Data<AppState>,
    path: web::Path<String>,
) -> impl Responder {
    let match_id = path.into_inner();

    // Load match from DB
    let row = match state.db.get_match(&match_id).await {
        Ok(Some(row)) => row,
        Ok(None) => {
            return HttpResponse::NotFound().json(ErrorResponse {
                error: "Match not found".to_string(),
            });
        }
        Err(e) => {
            log::error!("Failed to load match {}: {}", match_id, e);
            return HttpResponse::InternalServerError().json(ErrorResponse {
                error: "Failed to load match".to_string(),
            });
        }
    };

    // Check on-chain balance via watcher
    let escrow_status = match state
        .watcher
        .check_escrow_balance(&row.escrow_address)
        .await
    {
        Ok(s) => s,
        Err(e) => {
            log::error!("Failed to check escrow balance for {}: {}", match_id, e);
            return HttpResponse::InternalServerError().json(ErrorResponse {
                error: format!("Failed to check escrow balance: {}", e),
            });
        }
    };

    let expected_total = (row.wager_sompi as u64) * 2;
    let player_a_deposited = row.player_a_deposited.unwrap_or(0) != 0;
    let player_b_deposited = row.player_b_deposited.unwrap_or(0) != 0;

    let status_text = if player_a_deposited && player_b_deposited {
        "both_deposited".to_string()
    } else if player_a_deposited {
        "waiting_for_player_b".to_string()
    } else if player_b_deposited {
        "waiting_for_player_a".to_string()
    } else {
        "waiting_for_deposits".to_string()
    };

    HttpResponse::Ok().json(EscrowStatusResponse {
        match_id: row.match_id,
        escrow_address: row.escrow_address,
        expected_total,
        expected_total_kas: expected_total as f64 / SOMPI_PER_KAS as f64,
        current_balance: escrow_status.total_balance,
        current_balance_kas: escrow_status.total_balance_kas,
        player_a_deposited,
        player_b_deposited,
        utxo_count: escrow_status.utxo_count,
        status: status_text,
    })
}

#[post("/api/matches/{match_id}/resolve")]
pub async fn resolve_match(
    state: web::Data<AppState>,
    path: web::Path<String>,
    body: web::Json<ResolveMatchRequest>,
) -> impl Responder {
    let match_id = path.into_inner();

    // Load match from DB
    let row = match state.db.get_match(&match_id).await {
        Ok(Some(row)) => row,
        Ok(None) => {
            return HttpResponse::NotFound().json(ErrorResponse {
                error: "Match not found".to_string(),
            });
        }
        Err(e) => {
            log::error!("Failed to load match {}: {}", match_id, e);
            return HttpResponse::InternalServerError().json(ErrorResponse {
                error: "Failed to load match".to_string(),
            });
        }
    };

    // Deserialize current state
    let current_state: MatchState = match serde_json::from_str(&row.state_json) {
        Ok(s) => s,
        Err(e) => {
            log::error!("Failed to deserialize match state: {}", e);
            return HttpResponse::InternalServerError().json(ErrorResponse {
                error: "Invalid match state".to_string(),
            });
        }
    };

    // Validate winner is a participant
    let is_player_a = body.winner_id == row.player_a_id;
    let is_player_b = row
        .player_b_id
        .as_ref()
        .map(|b| body.winner_id == *b)
        .unwrap_or(false);

    if !is_player_a && !is_player_b {
        return HttpResponse::BadRequest().json(ErrorResponse {
            error: format!(
                "Winner '{}' is not a participant of this match",
                body.winner_id
            ),
        });
    }

    // Apply state machine transition: ResolveWinner
    let action = MatchAction::ResolveWinner {
        winner_id: body.winner_id.clone(),
    };

    let new_state = match transition(
        &current_state,
        &action,
        &row.player_a_id,
        row.player_b_id.as_deref(),
    ) {
        Ok(s) => s,
        Err(e) => {
            return HttpResponse::BadRequest().json(ErrorResponse {
                error: e.to_string(),
            });
        }
    };

    // Update match state to Resolved
    if let Err(e) = state.db.update_match_state(&match_id, &new_state).await {
        log::error!("Failed to update match {} to Resolved: {}", match_id, e);
        return HttpResponse::InternalServerError().json(ErrorResponse {
            error: "Failed to update match state".to_string(),
        });
    }

    log::info!(
        "Match {} resolved: winner = {}, reported by {}",
        match_id,
        body.winner_id,
        body.reported_by
    );

    // Determine winner address
    let winner_address = if is_player_a {
        row.player_a_addr.clone()
    } else {
        row.player_b_addr.clone().unwrap_or_default()
    };

    // Attempt payout
    let total_pot = (row.wager_sompi as u64) * 2;
    let treasury_address = "kaspatest:qtreasury_kaspabattle";
    let escrow_address = row.escrow_address.clone();

    let payout_response = match state
        .payout
        .execute_payout(
            &match_id,
            &winner_address,
            treasury_address,
            &escrow_address,
            total_pot,
        )
        .await
    {
        Ok(result) => {
            // Record payout in DB
            if let Err(e) = state
                .db
                .update_match_payout(
                    &match_id,
                    &body.winner_id,
                    &winner_address,
                    &result.tx_id,
                    result.winner_amount as i64,
                )
                .await
            {
                log::error!("Failed to record payout for {}: {}", match_id, e);
            }

            Some(PayoutResponse {
                winner_amount_kas: result.winner_amount as f64 / SOMPI_PER_KAS as f64,
                protocol_fee_kas: result.platform_fee as f64 / SOMPI_PER_KAS as f64,
                oracle_fee_kas: 0.0,
                payout_tx_hash: result.tx_id,
            })
        }
        Err(e) => {
            log::warn!(
                "Payout for match {} failed (will retry later): {}",
                match_id,
                e
            );
            None
        }
    };

    HttpResponse::Ok().json(ResolveMatchResponse {
        match_id,
        state: new_state.state_name().to_string(),
        winner_id: body.winner_id.clone(),
        payout: payout_response,
    })
}

#[get("/api/node/status")]
pub async fn node_status(state: web::Data<AppState>) -> impl Responder {
    let connected = state.kaspa.is_connected().await;
    let platform_address = match state.db.get_platform_address().await {
        Ok(addr) => addr,
        Err(_) => "unknown".to_string(),
    };

    let (server_version, is_synced, is_utxo_indexed) = match state.kaspa.get_node_info().await {
        Ok(info) => (info.server_version, info.is_synced, info.is_utxo_indexed),
        Err(_) => ("unknown".to_string(), false, false),
    };

    HttpResponse::Ok().json(NodeStatusResponse {
        server_version,
        is_synced,
        is_utxo_indexed,
        connected,
        platform_address,
    })
}

// === API V1 Endpoints ===

#[post("/api/v1/challenges/{id}/escrow")]
pub async fn create_challenge_escrow(
    state: web::Data<AppState>,
    path: web::Path<String>,
) -> impl Responder {
    let challenge_id = path.into_inner();

    // Check if match exists and its wager
    let row = match state.db.get_match(&challenge_id).await {
        Ok(Some(row)) => row,
        Ok(None) => {
            return HttpResponse::NotFound().json(ErrorResponse {
                error: "Challenge not found".to_string(),
            })
        }
        Err(e) => {
            return HttpResponse::InternalServerError().json(ErrorResponse {
                error: e.to_string(),
            })
        }
    };

    // Create escrow via service
    match state
        .escrow
        .create_escrow(&challenge_id, row.wager_sompi as u64)
    {
        Ok(info) => {
            // Save escrow to DB
            if let Err(e) = state
                .db
                .create_escrow(
                    &challenge_id,
                    &info.escrow_address,
                    info.derivation_index as i32,
                    info.wager_amount_sompi as i64,
                )
                .await
            {
                log::error!("Failed to save escrow to DB: {}", e);
            }
            HttpResponse::Created().json(EscrowCreateResponse {
                challenge_id: info.challenge_id,
                escrow_address: info.escrow_address,
                wager_amount_sompi: info.wager_amount_sompi,
                wager_amount_kas: info.wager_amount_kas,
            })
        }
        Err(e) => HttpResponse::InternalServerError().json(ErrorResponse {
            error: e.to_string(),
        }),
    }
}

#[get("/api/v1/challenges/{id}/deposits")]
pub async fn get_challenge_deposits(
    state: web::Data<AppState>,
    path: web::Path<String>,
) -> impl Responder {
    let challenge_id = path.into_inner();

    let escrow = match state.db.get_escrow(&challenge_id).await {
        Ok(Some(e)) => e,
        Ok(None) => {
            return HttpResponse::NotFound().json(ErrorResponse {
                error: "Escrow for challenge not found".to_string(),
            })
        }
        Err(e) => {
            return HttpResponse::InternalServerError().json(ErrorResponse {
                error: e.to_string(),
            })
        }
    };

    match state
        .escrow
        .check_deposits(&escrow.escrow_address, escrow.wager_amount_sompi as u64)
        .await
    {
        Ok(status) => {
            // Update status in DB if changed
            if status.status.to_string() != escrow.status {
                if let Err(e) = state
                    .db
                    .update_escrow_status(&challenge_id, &status.status.to_string())
                    .await
                {
                    log::error!("Failed to update escrow status: {}", e);
                }
            }
            HttpResponse::Ok().json(DepositStatusResponse {
                challenge_id,
                status: status.status.to_string(),
                deposited_sompi: status.deposited_sompi,
                required_sompi: status.required_sompi,
                player_a_deposited: status.player_a_deposited,
                player_b_deposited: status.player_b_deposited,
                utxo_count: status.utxo_count,
            })
        }
        Err(e) => HttpResponse::InternalServerError().json(ErrorResponse {
            error: e.to_string(),
        }),
    }
}

#[post("/api/v1/challenges/{id}/cancel")]
pub async fn cancel_challenge(
    state: web::Data<AppState>,
    path: web::Path<String>,
    body: web::Json<CancelChallengeRequest>,
) -> impl Responder {
    let challenge_id = path.into_inner();

    // Verify challenge exists
    let row = match state.db.get_match(&challenge_id).await {
        Ok(Some(r)) => r,
        Ok(None) => {
            return HttpResponse::NotFound().json(ErrorResponse {
                error: "Challenge not found".to_string(),
            })
        }
        Err(e) => {
            return HttpResponse::InternalServerError().json(ErrorResponse {
                error: e.to_string(),
            })
        }
    };

    // Calculate refund
    let player_a_addr = row.player_a_addr;
    let player_b_addr = row.player_b_addr.unwrap_or_default();
    let escrow_address = row.escrow_address;

    match state
        .escrow
        .refund(
            &challenge_id,
            &player_a_addr,
            &player_b_addr,
            &escrow_address,
            row.wager_sompi as u64,
        )
        .await
    {
        Ok(result) => {
            // Update DB
            if let Err(e) = state
                .db
                .update_escrow_refund(
                    &challenge_id,
                    result.refund_tx_a.as_deref(),
                    result.refund_tx_b.as_deref(),
                )
                .await
            {
                log::error!("Failed to update escrow refund: {}", e);
            }
            // Also update match state to Cancelled
            let cancel_state = MatchState::Cancelled {
                reason: body
                    .reason
                    .clone()
                    .unwrap_or_else(|| "User requested cancel".to_string()),
            };
            let _ = state
                .db
                .update_match_state(&challenge_id, &cancel_state)
                .await;

            HttpResponse::Ok().json(CancelChallengeResponse {
                challenge_id,
                message: "Challenge cancelled and refund processed".to_string(),
                refund_tx_a: result.refund_tx_a,
                refund_tx_b: result.refund_tx_b,
            })
        }
        Err(e) => HttpResponse::InternalServerError().json(ErrorResponse {
            error: e.to_string(),
        }),
    }
}

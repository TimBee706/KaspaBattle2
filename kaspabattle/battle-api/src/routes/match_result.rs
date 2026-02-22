use actix_web::{post, web, HttpResponse, Responder};
use battle_core::models::match_::{BattleMatch, MatchStatus};
use serde::{Deserialize, Serialize};
use sqlx::Row;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Deserialize, Serialize)]
pub struct PayoutRequest {
    pub match_id: Uuid,
    pub admin_token: String,
}

#[post("/match/payout")]
pub async fn trigger_payout(
    req: web::Json<PayoutRequest>,
    state: web::Data<Arc<crate::routes::AppState>>,
) -> impl Responder {
    if req.admin_token != std::env::var("ADMIN_TOKEN").unwrap_or_else(|_| "secret".to_string()) {
        return HttpResponse::Unauthorized().json("Invalid admin token");
    }

    let match_id_str = req.match_id.to_string();
    let record = match sqlx::query("SELECT * FROM matches WHERE match_id = ?")
        .bind(&match_id_str)
        .fetch_optional(state.db.pool())
        .await
    {
        Ok(Some(r)) => r,
        Ok(None) => return HttpResponse::NotFound().json("Match not found"),
        Err(e) => return HttpResponse::InternalServerError().json(e.to_string()),
    };

    let status_str: String = record.get("state");
    if status_str != "InProgress" && status_str != "Locked" {
        return HttpResponse::BadRequest().json(format!(
            "Match is in status '{}', expected 'InProgress' or 'Locked'",
            status_str
        ));
    }

    // Map SqliteRow to BattleMatch
    let battle_match = BattleMatch {
        id: Uuid::parse_str(record.get::<&str, _>("match_id")).unwrap_or_default(),
        player_a_kas_address: record.get("player_a_addr"),
        player_b_kas_address: record
            .get::<Option<String>, _>("player_b_addr")
            .unwrap_or_default(),
        player_a_faceit_id: record.get("player_a_id"),
        player_b_faceit_id: record
            .get::<Option<String>, _>("player_b_id")
            .unwrap_or_default(),
        faceit_match_id: Some(match_id_str.clone()),
        wager_amount_sompi: record.get::<i64, _>("wager_sompi") as u64,
        escrow_address: record.get("escrow_address"),
        status: MatchStatus::Locked,
        winner_kas_address: record.get::<Option<String>, _>("winner_address"),
        payout_tx_hash: record.get::<Option<String>, _>("payout_tx_hash"),
        oracle_result_signature: None,
        created_at: chrono::Utc::now(), // Placeholder mapping needed
        locked_at: None,
        resolved_at: None,
        timeout_at: chrono::Utc::now(),
    };

    let faceit_match_id = battle_match.faceit_match_id.as_deref().unwrap_or("");
    let winner_result = match state
        .oracle
        .fetch_with_double_confirmation(faceit_match_id)
        .await
    {
        Ok(Some(res)) => res,
        Ok(None) => return HttpResponse::Conflict().json("Match not yet finished on FACEIT"),
        Err(e) => return HttpResponse::InternalServerError().json(format!("Oracle error: {}", e)),
    };

    // Determine winner address (simplified for MVP)
    let winner_address = if winner_result.winner_faceit_id == battle_match.player_a_faceit_id {
        battle_match.player_a_kas_address.clone()
    } else {
        battle_match.player_b_kas_address.clone()
    };

    if winner_address.is_empty() {
        return HttpResponse::BadRequest().json("Winner address could not be determined");
    }

    // Atomares DB-Lock (Sqlite version)
    let rows_affected = match sqlx::query(
        "UPDATE matches SET state = 'Locked' WHERE match_id = ? AND (state = 'InProgress')",
    )
    .bind(&match_id_str)
    .execute(state.db.pool())
    .await
    {
        Ok(res) => res.rows_affected(),
        Err(e) => return HttpResponse::InternalServerError().json(e.to_string()),
    };

    if rows_affected == 0 && status_str != "Locked" {
        return HttpResponse::Conflict().json("Match already locked or processed");
    }

    match state
        .payout
        .execute_payout(&battle_match, &winner_address)
        .await
    {
        Ok(payout) => {
            let _ = sqlx::query(
                "UPDATE matches SET state = 'Resolved', payout_tx_hash = ? WHERE match_id = ?",
            )
            .bind(&payout.winner_tx_hash)
            .bind(&match_id_str)
            .execute(state.db.pool())
            .await;

            HttpResponse::Ok().json(payout.winner_tx_hash)
        }
        Err(e) => HttpResponse::InternalServerError().json(format!("Payout failed: {}", e)),
    }
}

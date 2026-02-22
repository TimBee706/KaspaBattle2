use actix_web::{web, HttpResponse, Responder};
use serde_json::json;

use battle_core::models::oracle::{OracleCreateRequest, OracleJob, OracleJobStatus};
use crate::routes::AppState;

/// POST /api/v1/oracle/jobs
async fn create_oracle_job(
    state: web::Data<AppState>,
    body: web::Json<OracleCreateRequest>,
) -> impl Responder {
    let job_id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();

    let job = OracleJob {
        job_id: job_id.clone(),
        match_id: body.match_id.clone(),
        faceit_match_id: body.faceit_match_id.clone(),
        status: OracleJobStatus::Pending,
        reported_winner: None,
        error_message: None,
        created_at: now.clone(),
        updated_at: now,
        resolved_at: None,
    };

    // Im echten Code: Job speichern (direkt über state.db)
    // Wir rufen state.db.create_oracle_job(...) auf
    match state.db.create_oracle_job(&job).await {
        Ok(_) => HttpResponse::Created().json(job),
        Err(e) => HttpResponse::InternalServerError().json(json!({
            "error": format!("Fehler beim Erstellen des Oracle Jobs: {}", e)
        })),
    }
}

/// GET /api/v1/oracle/jobs/{job_id}
async fn get_oracle_job(
    state: web::Data<AppState>,
    path: web::Path<String>,
) -> impl Responder {
    let job_id = path.into_inner();
    
    match state.db.get_oracle_job(&job_id).await {
        Ok(Some(job)) => HttpResponse::Ok().json(job),
        Ok(None) => HttpResponse::NotFound().json(json!({"error": "Job not found"})),
        Err(e) => HttpResponse::InternalServerError().json(json!({
            "error": format!("Fehler beim Laden des Oracle Jobs: {}", e)
        })),
    }
}

pub fn oracle_routes(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/api/v1/oracle")
            .route("/jobs", web::post().to(create_oracle_job))
            .route("/jobs/{job_id}", web::get().to(get_oracle_job)),
    );
}

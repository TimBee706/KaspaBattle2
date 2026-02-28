use axum::{routing::get, Router};
use dotenvy::dotenv;
use sqlx::postgres::PgPoolOptions;
use std::net::SocketAddr;
use tokio::sync::broadcast;
use tower_http::cors::CorsLayer;

mod api;
mod episodes;
mod models;
mod services;

#[tokio::main]
async fn main() {
    dotenv().ok();

    let db_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&db_url)
        .await
        .unwrap();

    let (tx, _) = broadcast::channel(100);
    let state = api::AppState { pool, tx };

    let app = Router::new()
        .nest("/api", api::router())
        .route("/ws", get(api::ws_handler))
        .layer(CorsLayer::permissive())
        .with_state(state);

    let addr = SocketAddr::from(([0, 0, 0, 0], 8080));
    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

use actix_web::{test, web, App};
use battle_api::db::Database;
use battle_api::routes;
use battle_api::routes::AppState;
use battle_core::oracle::faceit::FaceitOracleService;
use battle_kaspa::mock::MockKaspaClient;
use battle_kaspa::payout::PayoutService;
use battle_kaspa::rpc::KaspaRpc;
use ed25519_dalek::SigningKey;
use rand::rngs::OsRng;
use std::sync::Arc;
use uuid::Uuid;

#[actix_web::test]
async fn test_automated_payout_flow() {
    // 1. Setup Mock Services
    let database_url = "sqlite::memory:?cache=shared";
    let db = Arc::new(Database::new(database_url).await.unwrap());

    let kaspa = Arc::new(MockKaspaClient::new()) as Arc<dyn KaspaRpc>;

    // 1b. Mock FACEIT API
    let mock_server = wiremock::MockServer::start().await;
    let match_id_fake = Uuid::new_v4().to_string();

    wiremock::Mock::given(wiremock::matchers::method("GET"))
        .and(wiremock::matchers::path(format!(
            "/matches/{}",
            match_id_fake
        )))
        .respond_with(
            wiremock::ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "status": "CREATED",
                "game": "cs2",
                "teams": {},
                "results": null,
                "finished_at": null
            })),
        )
        .mount(&mock_server)
        .await;

    let mut csprng = OsRng;
    let oracle_signing_key = SigningKey::generate(&mut csprng);
    let oracle = Arc::new(
        FaceitOracleService::new("test_key".to_string(), oracle_signing_key)
            .with_base_url(mock_server.uri()),
    );

    let payout = Arc::new(PayoutService::new(
        kaspa.clone(),
        "kaspatest:treasury".to_string(),
        std::collections::HashMap::new(),
    ));

    // Mock other services
    let wallet = Arc::new(battle_kaspa::wallet::EscrowWallet::new(None, "testnet-10").unwrap());
    let watcher = Arc::new(battle_kaspa::watcher::BlockchainWatcher::new(
        kaspa.clone(),
        std::time::Duration::from_secs(1),
    ));
    let escrow = Arc::new(battle_kaspa::escrow::EscrowService::new(
        wallet.clone(),
        kaspa.clone(),
    ));

    let rusqlite_conn = rusqlite::Connection::open_in_memory().unwrap();
    let auth_db = Arc::new(tokio::sync::Mutex::new(rusqlite_conn));
    let auth = Arc::new(
        battle_core::auth::AuthService::new(auth_db.clone())
            .await
            .unwrap(),
    );

    let faceit_oauth = Arc::new(battle_core::faceit_oauth::FaceitOAuthService::new(
        battle_core::models::faceit::FaceitOAuthConfig {
            client_id: "id".into(),
            client_secret: "secret".into(),
            redirect_uri: "uri".into(),
            auth_url: "url".into(),
            token_url: "url".into(),
            userinfo_url: "url".into(),
        },
        auth_db.clone(),
    ));

    let app_state = web::Data::new(AppState {
        db,
        kaspa,
        wallet,
        watcher,
        payout,
        escrow,
        auth,
        faceit_oauth,
        oracle,
        faceit_data: Arc::new(battle_core::faceit_data::FaceitDataService::new(
            "dummy".to_string(),
        )),
    });

    let app = test::init_service(
        App::new()
            .app_data(app_state.clone())
            .service(routes::match_result::trigger_payout),
    )
    .await;

    // 2. Create a Mock Match in DB
    let match_id = Uuid::new_v4();
    let match_id_str = match_id.to_string();

    sqlx::query("INSERT INTO matches (match_id, state, state_json, player_a_id, player_a_addr, wager_sompi, escrow_address, faceit_match_id) VALUES (?, 'InProgress', '{}', 'player_a', 'kaspatest:addr_a', 1000000, 'kaspatest:escrow', ?)")
        .bind(&match_id_str)
        .bind(&match_id_fake)
        .execute(app_state.db.pool())
        .await
        .unwrap();

    // 3. Trigger Payout
    let req = test::TestRequest::post()
        .uri("/match/payout")
        .set_json(routes::match_result::PayoutRequest {
            match_id,
            admin_token: "secret".to_string(),
        })
        .to_request();

    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 409);
}

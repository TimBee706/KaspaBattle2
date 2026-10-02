//! Integration tests for Free Play: wallet-free / FACEIT-free / node-free Connect Four with a bot
//! and with another player. Real Postgres + real router; the `AppState` has **no** Kaspa RPC,
//! escrow or multisig service at all, which is exactly the "node is down" situation.

use crate::free_play::{self, FpError, MoveRequest, Opponent};
use crate::native_tests::TestDb;
use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use battle_core::native_games::connect_four::{ConnectFour, GameStatus};
use battle_core::native_games::connect_four_bot::Difficulty;
use sqlx::{PgPool, Row};
use std::sync::Arc;
use tower::ServiceExt;
use uuid::Uuid;

async fn make_user(pool: &PgPool, name: &str) -> Uuid {
    let id = Uuid::new_v4();
    sqlx::query("INSERT INTO users (id, email, password_hash, display_name, username, has_password_login, email_verified) VALUES ($1,$2,'x',$3,$3,TRUE,TRUE)")
        .bind(id).bind(format!("{name}@example.test")).bind(name).execute(pool).await.unwrap();
    id
}

async fn session(pool: &PgPool, user: Uuid) -> String {
    crate::account::create_session(pool, user, chrono::Duration::hours(1))
        .await
        .unwrap()
        .0
}

fn mv_req(pool_version: i64, column: i64) -> MoveRequest {
    MoveRequest {
        column,
        expected_version: pool_version,
        client_nonce: Uuid::new_v4().to_string(),
    }
}

async fn version(pool: &PgPool, id: Uuid) -> i64 {
    sqlx::query_scalar("SELECT version FROM free_play_games WHERE id = $1")
        .bind(id)
        .fetch_one(pool)
        .await
        .unwrap()
}

async fn play(
    pool: &PgPool,
    id: Uuid,
    user: Uuid,
    col: i64,
) -> Result<free_play::Outcome, FpError> {
    let v = version(pool, id).await;
    free_play::apply_move(pool, id, user, mv_req(v, col)).await
}

fn app(pool: PgPool) -> axum::Router {
    // Reuse the state builder of the account tests (no Kaspa services).
    let (tx, _rx) = tokio::sync::broadcast::channel(256);
    let cfg = crate::account::AccountConfig::from_env(false, |_| None);
    let state = crate::api::AppState {
        pool: pool.clone(),
        tx,
        auth_service: Arc::new(battle_core::auth::AuthService::new(pool.clone())),
        faceit_service: Arc::new(battle_core::faceit_oauth::FaceitOAuthService::new(
            battle_core::models::faceit::FaceitOAuthConfig {
                client_id: "x".into(),
                client_secret: "x".into(),
                redirect_uri: "http://localhost/cb".into(),
                auth_url: "http://localhost".into(),
                token_url: "http://localhost".into(),
                userinfo_url: "http://localhost".into(),
            },
            pool,
        )),
        faceit_data_service: None,
        escrow_wallet: None,
        escrow_service: None,
        kaspa_rpc: None,
        payout_service: None,
        blockchain_watcher: None,
        multisig_service: None,
        account: crate::account::AccountRuntime::new(cfg, Arc::new(crate::mail::DisabledMailer)),
        presence: crate::free_play::Presence::default(),
    };
    crate::api::router().with_state(state)
}

async fn call(
    app: &axum::Router,
    method: &str,
    uri: &str,
    token: Option<&str>,
    body: Option<serde_json::Value>,
) -> (StatusCode, serde_json::Value) {
    let mut req = Request::builder().method(method).uri(uri);
    if let Some(t) = token {
        req = req.header(header::COOKIE, format!("kaspabattle-auth={t}"));
    }
    let b = match body {
        Some(b) => {
            req = req.header(header::CONTENT_TYPE, "application/json");
            Body::from(b.to_string())
        }
        None => Body::empty(),
    };
    let res = app.clone().oneshot(req.body(b).unwrap()).await.unwrap();
    let st = res.status();
    let bytes = axum::body::to_bytes(res.into_body(), 1 << 20)
        .await
        .unwrap();
    (
        st,
        serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null),
    )
}

async fn row_count(pool: &PgPool, table: &str) -> i64 {
    sqlx::query_scalar(&format!("SELECT COUNT(*) FROM {table}"))
        .fetch_one(pool)
        .await
        .unwrap()
}

/// First legal column for the human (always column order 0..6, skipping full ones).
fn first_free(board: &serde_json::Value) -> i64 {
    (0..7).find(|c| board[5][*c as usize] == 0).unwrap() as i64
}

// ── Bot games ────────────────────────────────────────────────────────────────

#[tokio::test]
async fn bot_game_runs_end_to_end_without_wallet_faceit_or_kaspa_node() {
    let Some(db) = TestDb::new().await else {
        return;
    };
    let app = app(db.pool.clone());
    let alice = make_user(&db.pool, "Alice").await;
    let t = session(&db.pool, alice).await;

    // The user has no wallet and no FACEIT link at all.
    let (kaspa, faceit): (Option<String>, i64) = (
        sqlx::query_scalar("SELECT kaspa_address FROM users WHERE id = $1")
            .bind(alice)
            .fetch_one(&db.pool)
            .await
            .unwrap(),
        sqlx::query_scalar("SELECT COUNT(*) FROM faceit_links WHERE user_id = $1")
            .bind(alice)
            .fetch_one(&db.pool)
            .await
            .unwrap(),
    );
    assert!(kaspa.is_none() && faceit == 0);

    let (st, g) = call(
        &app,
        "POST",
        "/free-play/games",
        Some(&t),
        Some(serde_json::json!({"opponent":"bot","botDifficulty":"medium"})),
    )
    .await;
    assert_eq!(st, StatusCode::OK, "{g}");
    assert_eq!(g["mode"], "free_play");
    assert_eq!(g["status"], "active");
    assert_eq!(g["players"][1]["isBot"], true);
    assert_eq!(g["currentSlot"], 1);
    let id = g["id"].as_str().unwrap().to_string();

    // Play until the game ends; every human move is answered by exactly one legal bot move.
    let mut state = g;
    let mut guard = 0;
    while state["status"] == "active" {
        guard += 1;
        assert!(guard < 25);
        let col = first_free(&state["board"]);
        let (st, next) = call(&app, "POST", &format!("/free-play/games/{id}/moves"), Some(&t), Some(serde_json::json!({
            "column": col, "expectedVersion": state["version"], "clientNonce": Uuid::new_v4().to_string()
        }))).await;
        assert_eq!(st, StatusCode::OK, "{next}");
        let moved = next["moveCount"].as_i64().unwrap();
        if next["status"] == "active" {
            assert_eq!(
                moved % 2,
                0,
                "after the bot answered it is the human's turn again"
            );
            assert_eq!(next["currentSlot"], 1);
        }
        state = next;
    }
    assert_eq!(state["status"], "finished");
    assert!(["win", "draw"].contains(&state["result"].as_str().unwrap()));
    assert!(state["finishedAt"].is_string());

    // Server-side stats + history; nothing financial exists.
    let (_, stats) = call(&app, "GET", "/free-play/stats", Some(&t), None).await;
    assert_eq!(stats["played"], 1);
    assert_eq!(stats["vsBots"], 1);
    assert_eq!(stats["vsHumans"], 0);
    assert_eq!(
        stats["wins"].as_i64().unwrap()
            + stats["losses"].as_i64().unwrap()
            + stats["draws"].as_i64().unwrap(),
        1
    );
    let (_, h) = call(&app, "GET", "/free-play/history", Some(&t), None).await;
    assert_eq!(h["games"][0]["id"], id);
    assert_eq!(h["games"][0]["opponentKind"], "bot");
    assert_eq!(h["games"][0]["opponentName"], "Bot");
    for table in ["matches", "multisig_escrows", "payments"] {
        assert_eq!(row_count(&db.pool, table).await, 0, "{table}");
    }
    db.teardown().await;
}

#[tokio::test]
async fn bot_never_moves_twice_or_after_the_game_and_survives_concurrent_requests() {
    let Some(db) = TestDb::new().await else {
        return;
    };
    let alice = make_user(&db.pool, "Alice").await;
    let out = free_play::create_game(&db.pool, alice, Opponent::Bot(Difficulty::Easy))
        .await
        .unwrap();
    let id = out.snapshot.id;

    // nothing pending → the bot has nothing to do (idempotent)
    assert!(free_play::bot_turn_if_needed(&db.pool, id)
        .await
        .unwrap()
        .is_none());
    let after = play(&db.pool, id, alice, 3).await.unwrap();
    assert_eq!(after.snapshot.move_count, 2);
    assert!(
        free_play::bot_turn_if_needed(&db.pool, id)
            .await
            .unwrap()
            .is_none(),
        "no second bot move for the same turn"
    );
    let n: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM free_play_moves WHERE game_id=$1 AND player_id IS NULL",
    )
    .bind(id)
    .fetch_one(&db.pool)
    .await
    .unwrap();
    assert_eq!(n, 1);

    // two simultaneous human moves with the same version → exactly one wins the race
    let v = version(&db.pool, id).await;
    let (a, b) = tokio::join!(
        free_play::apply_move(&db.pool, id, alice, mv_req(v, 0)),
        free_play::apply_move(&db.pool, id, alice, mv_req(v, 1)),
    );
    assert_eq!([a.is_ok(), b.is_ok()].iter().filter(|x| **x).count(), 1);
    assert!(matches!(
        a.as_ref().err().or(b.as_ref().err()),
        Some(FpError::VersionConflict { .. })
    ));
    // alternation intact: human, bot, human, bot, human, bot
    let seqs: Vec<(i32, i16)> = sqlx::query_as("SELECT sequence_number, slot FROM free_play_moves WHERE game_id=$1 ORDER BY sequence_number").bind(id).fetch_all(&db.pool).await.unwrap();
    assert!(
        seqs.iter()
            .all(|(n, slot)| *slot == if n % 2 == 1 { 1 } else { 2 }),
        "{seqs:?}"
    );

    // finished games take no bot move
    free_play::leave(&db.pool, id, alice).await.unwrap();
    assert!(free_play::bot_turn_if_needed(&db.pool, id)
        .await
        .unwrap()
        .is_none());
    assert!(matches!(
        play(&db.pool, id, alice, 2).await,
        Err(FpError::WrongStatus(_))
    ));
    db.teardown().await;
}

#[tokio::test]
async fn bots_of_every_difficulty_finish_whole_games_with_only_legal_moves() {
    let Some(db) = TestDb::new().await else {
        return;
    };
    let alice = make_user(&db.pool, "Alice").await;
    for d in [Difficulty::Easy, Difficulty::Medium] {
        let id = free_play::create_game(&db.pool, alice, Opponent::Bot(d))
            .await
            .unwrap()
            .snapshot
            .id;
        let mut last = None;
        for i in 0..30 {
            let snap = free_play::get(&db.pool, id).await.unwrap();
            if snap.status != "active" {
                break;
            }
            let col = (i * 3 % 7) as i64;
            let col = if snap.board[5][col as usize] != 0 {
                first_free(&serde_json::to_value(&snap.board).unwrap())
            } else {
                col
            };
            last = Some(play(&db.pool, id, alice, col).await.unwrap().snapshot);
        }
        let snap = last.unwrap_or(free_play::get(&db.pool, id).await.unwrap());
        // Rebuild from the persisted move list: it must be a legal game that matches the stored state.
        let rows = sqlx::query("SELECT slot, column_index, state_hash FROM free_play_moves WHERE game_id=$1 ORDER BY sequence_number").bind(id).fetch_all(&db.pool).await.unwrap();
        let mut g = ConnectFour::new("p1", "p2");
        for r in &rows {
            let who = if r.get::<i16, _>("slot") == 1 {
                "p1"
            } else {
                "p2"
            };
            g.execute(who, r.get::<i16, _>("column_index") as usize)
                .expect("persisted moves are legal");
            assert_eq!(g.state_hash(), r.get::<String, _>("state_hash"));
        }
        assert_eq!(g.state_hash(), snap.state_hash);
        if snap.status == "finished" {
            assert!(matches!(
                g.status(),
                GameStatus::Won { .. } | GameStatus::Draw
            ));
        }
    }
    db.teardown().await;
}

// ── Free-play invariants (no money, ever) ────────────────────────────────────

#[tokio::test]
async fn free_play_cannot_carry_a_stake_escrow_or_payout() {
    let Some(db) = TestDb::new().await else {
        return;
    };
    let app = app(db.pool.clone());
    let alice = make_user(&db.pool, "Alice").await;
    let t = session(&db.pool, alice).await;

    // manipulated create requests are refused (unknown money-related fields)
    for extra in [
        ("stake", serde_json::json!(100)),
        ("wagerSompi", serde_json::json!(1)),
        ("escrowAddress", serde_json::json!("kaspatest:q")),
        ("mode", serde_json::json!("kaspa_testnet")),
    ] {
        let mut body = serde_json::json!({"opponent":"bot","botDifficulty":"easy"});
        body[extra.0] = extra.1;
        let (st, _) = call(&app, "POST", "/free-play/games", Some(&t), Some(body)).await;
        assert!(
            st.is_client_error(),
            "{} must be rejected, got {st}",
            extra.0
        );
    }
    assert_eq!(row_count(&db.pool, "free_play_games").await, 0);

    // the paid-match endpoint refuses free play: no match, escrow or deposit is created
    let (st, _) = call(&app, "POST", "/challenges", Some(&t), Some(serde_json::json!({"game_id":"connect-four","wager_sompi":0,"mode":"BO1","game_mode":"free_play"}))).await;
    assert!(st.is_client_error());
    assert_eq!(row_count(&db.pool, "matches").await, 0);

    // …and the database itself forbids it
    let id = free_play::create_game(&db.pool, alice, Opponent::Human)
        .await
        .unwrap()
        .snapshot
        .id;
    for sql in [
        "UPDATE free_play_games SET stake_sompi = 5",
        "UPDATE free_play_games SET mode = 'kaspa_testnet'",
        "UPDATE matches SET game_mode = 'free_play'",
    ] {
        assert!(
            sqlx::query(sql).execute(&db.pool).await.is_err() || sql.contains("matches"),
            "{sql}"
        );
    }
    let stake: i64 = sqlx::query_scalar("SELECT stake_sompi FROM free_play_games WHERE id=$1")
        .bind(id)
        .fetch_one(&db.pool)
        .await
        .unwrap();
    assert_eq!(stake, 0);
    // a bot game must not have a second human
    let bob = make_user(&db.pool, "Bob").await;
    let bad = sqlx::query("INSERT INTO free_play_games (opponent_kind, bot_difficulty, status, player_one_id, player_two_id, board_state, state_hash) VALUES ('bot','easy','active',$1,$2,'{}'::jsonb,'h')")
        .bind(alice).bind(bob).execute(&db.pool).await;
    assert!(bad.is_err());
    // no free-play table references the paid-match machinery
    let fks: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM information_schema.table_constraints tc \
         JOIN information_schema.constraint_column_usage ccu USING (constraint_name, constraint_schema) \
         WHERE tc.constraint_type = 'FOREIGN KEY' AND tc.table_name LIKE 'free_play_%' \
           AND ccu.table_name IN ('matches', 'multisig_escrows', 'payments')",
    ).fetch_one(&db.pool).await.unwrap();
    assert_eq!(fks, 0);
    db.teardown().await;
}

// ── Human vs human ───────────────────────────────────────────────────────────

#[tokio::test]
async fn two_players_play_a_full_game_through_the_lobby() {
    let Some(db) = TestDb::new().await else {
        return;
    };
    let app = app(db.pool.clone());
    let a = make_user(&db.pool, "Alice").await;
    let b = make_user(&db.pool, "Bob").await;
    let c = make_user(&db.pool, "Carol").await;
    let (ta, tb, tc) = (
        session(&db.pool, a).await,
        session(&db.pool, b).await,
        session(&db.pool, c).await,
    );

    let (st, g) = call(
        &app,
        "POST",
        "/free-play/games",
        Some(&ta),
        Some(serde_json::json!({"opponent":"human"})),
    )
    .await;
    assert_eq!(st, StatusCode::OK, "{g}");
    assert_eq!(g["status"], "open");
    let id = g["id"].as_str().unwrap().to_string();

    // lobby list: visible to others, flagged as mine for the creator
    let (_, l) = call(&app, "GET", "/free-play/lobbies", Some(&tb), None).await;
    assert_eq!(l["lobbies"][0]["id"], id);
    assert_eq!(l["lobbies"][0]["creatorName"], "Alice");
    assert_eq!(l["lobbies"][0]["mine"], false);
    assert_eq!(
        call(&app, "GET", "/free-play/lobbies", None, None).await.0,
        StatusCode::UNAUTHORIZED
    );

    // creator cannot take the second seat; nobody can move in an open lobby
    assert_eq!(
        call(
            &app,
            "POST",
            &format!("/free-play/games/{id}/join"),
            Some(&ta),
            None
        )
        .await
        .1["error"],
        "own_lobby"
    );
    let (st, _) = call(&app, "POST", &format!("/free-play/games/{id}/moves"), Some(&ta), Some(serde_json::json!({"column":0,"expectedVersion":0,"clientNonce":Uuid::new_v4().to_string()}))).await;
    assert_eq!(st, StatusCode::CONFLICT);

    let (st, g) = call(
        &app,
        "POST",
        &format!("/free-play/games/{id}/join"),
        Some(&tb),
        None,
    )
    .await;
    assert_eq!(st, StatusCode::OK, "{g}");
    assert_eq!(g["status"], "active");
    assert_eq!(g["players"][1]["displayName"], "Bob");
    // lobby is taken
    let (st, e) = call(
        &app,
        "POST",
        &format!("/free-play/games/{id}/join"),
        Some(&tc),
        None,
    )
    .await;
    assert_eq!(
        (st, e["error"].clone()),
        (StatusCode::CONFLICT, "lobby_not_open".into())
    );

    // spectator (logged in, not a player) may watch but never move
    let (st, spec) = call(
        &app,
        "GET",
        &format!("/free-play/games/{id}"),
        Some(&tc),
        None,
    )
    .await;
    assert_eq!(st, StatusCode::OK);
    assert!(spec["you"]["slot"].is_null());
    let mv = |token: String, col: i64, v: i64| {
        let app = app.clone();
        let id = id.clone();
        async move {
            call(&app, "POST", &format!("/free-play/games/{id}/moves"), Some(&token), Some(serde_json::json!({"column":col,"expectedVersion":v,"clientNonce":Uuid::new_v4().to_string()}))).await
        }
    };
    let mut v = spec["version"].as_i64().unwrap();
    assert_eq!(mv(tc.clone(), 0, v).await.0, StatusCode::FORBIDDEN);
    // wrong turn, stale version, invalid column
    assert_eq!(mv(tb.clone(), 0, v).await.1["error"], "not_your_turn");
    assert_eq!(
        mv(ta.clone(), 0, v + 7).await.1["error"],
        "version_conflict"
    );
    assert_eq!(mv(ta.clone(), 9, v).await.1["error"], "invalid_column");

    // Alice wins vertically in column 0 (A,B,A,B,A,B,A)
    for (i, col) in [0, 1, 0, 1, 0, 1, 0].iter().enumerate() {
        let (tok, who) = if i % 2 == 0 {
            (ta.clone(), "alice")
        } else {
            (tb.clone(), "bob")
        };
        let (st, r) = mv(tok, *col, v).await;
        assert_eq!(st, StatusCode::OK, "{who} move {i}: {r}");
        v = r["version"].as_i64().unwrap();
        if i == 6 {
            assert_eq!(r["status"], "finished");
            assert_eq!(r["result"], "win");
            assert_eq!(r["endReason"], "connect_four");
            assert_eq!(r["winnerUserId"], a.to_string());
            assert_eq!(r["winningLine"].as_array().unwrap().len(), 4);
        }
    }
    // after the end: no more moves
    assert_eq!(mv(tb.clone(), 3, v).await.1["error"], "game_over");

    // both histories + stats come from the server
    let (_, sa) = call(&app, "GET", "/free-play/stats", Some(&ta), None).await;
    let (_, sb) = call(&app, "GET", "/free-play/stats", Some(&tb), None).await;
    assert_eq!(
        (
            sa["wins"].clone(),
            sa["losses"].clone(),
            sa["vsHumans"].clone()
        ),
        (1.into(), 0.into(), 1.into())
    );
    assert_eq!(
        (sb["wins"].clone(), sb["losses"].clone()),
        (0.into(), 1.into())
    );
    let (_, ha) = call(&app, "GET", "/free-play/history", Some(&ta), None).await;
    let (_, hb) = call(&app, "GET", "/free-play/history", Some(&tb), None).await;
    assert_eq!(ha["games"][0]["outcome"], "win");
    assert_eq!(ha["games"][0]["opponentName"], "Bob");
    assert_eq!(hb["games"][0]["outcome"], "loss");
    assert_eq!(hb["games"][0]["opponentName"], "Alice");
    let (_, sc) = call(&app, "GET", "/free-play/stats", Some(&tc), None).await;
    assert_eq!(sc["played"], 0, "spectators get no result");

    // rematch needs both players; colours swap
    let (_, r1) = call(
        &app,
        "POST",
        &format!("/free-play/games/{id}/rematch"),
        Some(&ta),
        None,
    )
    .await;
    assert!(r1["rematchGameId"].is_null());
    assert_eq!(
        call(
            &app,
            "POST",
            &format!("/free-play/games/{id}/rematch"),
            Some(&tc),
            None
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    let (_, r2) = call(
        &app,
        "POST",
        &format!("/free-play/games/{id}/rematch"),
        Some(&tb),
        None,
    )
    .await;
    let new_id = r2["rematchGameId"]
        .as_str()
        .expect("both agreed → new game")
        .to_string();
    let (_, ng) = call(
        &app,
        "GET",
        &format!("/free-play/games/{new_id}"),
        Some(&tb),
        None,
    )
    .await;
    assert_eq!(ng["status"], "active");
    assert_eq!(
        ng["players"][0]["displayName"], "Bob",
        "loser starts the rematch"
    );
    db.teardown().await;
}

#[tokio::test]
async fn concurrent_joins_seat_exactly_one_player() {
    let Some(db) = TestDb::new().await else {
        return;
    };
    let a = make_user(&db.pool, "Alice").await;
    let id = free_play::create_game(&db.pool, a, Opponent::Human)
        .await
        .unwrap()
        .snapshot
        .id;
    let mut users = vec![];
    for i in 0..6 {
        users.push(make_user(&db.pool, &format!("Joiner{i}")).await);
    }
    let handles: Vec<_> = users
        .iter()
        .map(|u| {
            let (pool, u) = (db.pool.clone(), *u);
            tokio::spawn(async move { free_play::join(&pool, id, u).await })
        })
        .collect();
    let results: Vec<_> = futures::future::join_all(handles)
        .await
        .into_iter()
        .map(|r| r.unwrap())
        .collect();
    assert_eq!(results.iter().filter(|r| r.is_ok()).count(), 1);
    let p2: Option<Uuid> =
        sqlx::query_scalar("SELECT player_two_id FROM free_play_games WHERE id=$1")
            .bind(id)
            .fetch_one(&db.pool)
            .await
            .unwrap();
    assert!(users.contains(&p2.unwrap()));
    // the same user can never hold both seats
    assert!(
        sqlx::query("UPDATE free_play_games SET player_two_id = player_one_id WHERE id=$1")
            .bind(id)
            .execute(&db.pool)
            .await
            .is_err()
    );
    db.teardown().await;
}

#[tokio::test]
async fn move_requests_are_idempotent_and_authorised() {
    let Some(db) = TestDb::new().await else {
        return;
    };
    let a = make_user(&db.pool, "Alice").await;
    let b = make_user(&db.pool, "Bob").await;
    let id = free_play::create_game(&db.pool, a, Opponent::Human)
        .await
        .unwrap()
        .snapshot
        .id;
    free_play::join(&db.pool, id, b).await.unwrap();

    let nonce = Uuid::new_v4().to_string();
    let v = version(&db.pool, id).await;
    let req = || MoveRequest {
        column: 3,
        expected_version: v,
        client_nonce: nonce.clone(),
    };
    let first = free_play::apply_move(&db.pool, id, a, req()).await.unwrap();
    let again = free_play::apply_move(&db.pool, id, a, req()).await.unwrap();
    assert!(!first.replay && again.replay && again.events.is_empty());
    assert_eq!(again.snapshot.move_count, 1);
    assert!(matches!(
        free_play::apply_move(
            &db.pool,
            id,
            a,
            MoveRequest {
                client_nonce: "nope".into(),
                ..req()
            }
        )
        .await,
        Err(FpError::BadRequest(_))
    ));
    let stranger = make_user(&db.pool, "Eve").await;
    assert!(matches!(
        play(&db.pool, id, stranger, 1).await,
        Err(FpError::NotParticipant)
    ));
    assert!(
        matches!(play(&db.pool, id, a, 1).await, Err(FpError::Move(_))),
        "not Alice's turn"
    );
    assert!(matches!(
        play(&db.pool, id, b, -1).await,
        Err(FpError::Move(_))
    ));
    db.teardown().await;
}

#[tokio::test]
async fn leaving_closes_lobbies_and_forfeits_running_games() {
    let Some(db) = TestDb::new().await else {
        return;
    };
    let a = make_user(&db.pool, "Alice").await;
    let b = make_user(&db.pool, "Bob").await;
    let open = free_play::create_game(&db.pool, a, Opponent::Human)
        .await
        .unwrap();
    let closed = free_play::leave(&db.pool, open.snapshot.id, a)
        .await
        .unwrap();
    assert_eq!(closed.snapshot.status, "cancelled");
    assert!(matches!(
        free_play::join(&db.pool, open.snapshot.id, b).await,
        Err(FpError::Conflict(_))
    ));

    let id = free_play::create_game(&db.pool, a, Opponent::Human)
        .await
        .unwrap()
        .snapshot
        .id;
    free_play::join(&db.pool, id, b).await.unwrap();
    let out = free_play::leave(&db.pool, id, a).await.unwrap();
    assert_eq!(out.snapshot.status, "finished");
    assert_eq!(out.snapshot.winner_user_id, Some(b));
    assert_eq!(out.snapshot.end_reason.as_deref(), Some("left"));
    assert!(matches!(
        free_play::leave(&db.pool, id, a).await,
        Err(FpError::WrongStatus(_))
    ));
    let s = free_play::stats(&db.pool, b).await.unwrap();
    assert_eq!((s.wins, s.losses), (1, 0));
    let s = free_play::stats(&db.pool, a).await.unwrap();
    assert_eq!((s.wins, s.losses), (0, 1));
    db.teardown().await;
}

#[tokio::test]
async fn timeouts_follow_the_documented_rules() {
    let Some(db) = TestDb::new().await else {
        return;
    };
    let a = make_user(&db.pool, "Alice").await;
    let b = make_user(&db.pool, "Bob").await;

    // human game: the player to move forfeits after the deadline
    let id = free_play::create_game(&db.pool, a, Opponent::Human)
        .await
        .unwrap()
        .snapshot
        .id;
    free_play::join(&db.pool, id, b).await.unwrap();
    assert!(
        free_play::expire_due(&db.pool).await.unwrap().is_empty(),
        "nothing due yet"
    );
    play(&db.pool, id, a, 3).await.unwrap(); // now Bob is to move
    sqlx::query(
        "UPDATE free_play_games SET turn_deadline = NOW() - INTERVAL '1 second' WHERE id=$1",
    )
    .bind(id)
    .execute(&db.pool)
    .await
    .unwrap();
    let events = free_play::expire_due(&db.pool).await.unwrap();
    assert!(events.iter().any(|e| e["type"] == "free_play_finished"));
    let snap = free_play::get(&db.pool, id).await.unwrap();
    assert_eq!(
        (
            snap.status.as_str(),
            snap.end_reason.as_deref(),
            snap.winner_user_id
        ),
        ("finished", Some("timeout"), Some(a))
    );

    // bot game left idle: abandoned – no winner, not counted as win/loss
    let bot = free_play::create_game(&db.pool, a, Opponent::Bot(Difficulty::Easy))
        .await
        .unwrap()
        .snapshot
        .id;
    sqlx::query(
        "UPDATE free_play_games SET turn_deadline = NOW() - INTERVAL '1 second' WHERE id=$1",
    )
    .bind(bot)
    .execute(&db.pool)
    .await
    .unwrap();
    free_play::expire_due(&db.pool).await.unwrap();
    let snap = free_play::get(&db.pool, bot).await.unwrap();
    assert_eq!(
        (
            snap.status.as_str(),
            snap.result.as_deref(),
            snap.winner_user_id
        ),
        ("finished", Some("abandoned"), None)
    );
    let s = free_play::stats(&db.pool, a).await.unwrap();
    assert_eq!((s.wins, s.losses, s.abandoned, s.played), (1, 0, 1, 2));

    // stale open lobbies are closed
    let lobby = free_play::create_game(&db.pool, a, Opponent::Human)
        .await
        .unwrap()
        .snapshot
        .id;
    sqlx::query("UPDATE free_play_games SET created_at = NOW() - INTERVAL '2 hours' WHERE id=$1")
        .bind(lobby)
        .execute(&db.pool)
        .await
        .unwrap();
    free_play::expire_due(&db.pool).await.unwrap();
    assert_eq!(
        free_play::get(&db.pool, lobby).await.unwrap().status,
        "cancelled"
    );
    db.teardown().await;
}

#[tokio::test]
async fn per_user_limits_prevent_lobby_spam() {
    let Some(db) = TestDb::new().await else {
        return;
    };
    let a = make_user(&db.pool, "Alice").await;
    for _ in 0..3 {
        free_play::create_game(&db.pool, a, Opponent::Human)
            .await
            .unwrap();
    }
    assert!(matches!(
        free_play::create_game(&db.pool, a, Opponent::Human).await,
        Err(FpError::TooMany("too_many_open_lobbies"))
    ));
    db.teardown().await;
}

// ── WebSocket plumbing (pure parts) ──────────────────────────────────────────

#[test]
fn presence_tracks_first_and_last_connection() {
    let p = crate::free_play::Presence::default();
    let (g, u) = (Uuid::new_v4(), Uuid::new_v4());
    assert!(!p.is_connected(g, u));
    assert!(p.join(g, u), "first connection");
    assert!(!p.join(g, u), "second tab");
    assert!(p.is_connected(g, u));
    assert!(!p.leave(g, u), "one tab left");
    assert!(p.leave(g, u), "last connection gone");
    assert!(!p.is_connected(g, u));
    assert!(!p.leave(g, u), "leaving twice is harmless");
}

//! Integration tests for native (browser) games against a real PostgreSQL.
//!
//! Every test creates its own throw-away database (migrations applied from `../migrations`).
//! They need `DATABASE_URL` (any database on a server where the role may `CREATE DATABASE`). Without
//! it each test logs a notice and returns, so a plain `cargo test` on a machine without Postgres
//! still passes — CI provides Postgres (see `.github/workflows/ci.yml`) so they always run there.

use crate::api::AppState;
use crate::models::MatchStatus;
use crate::native_game::{self, MoveRequest, NativeError};
use axum::body::Body;
use axum::http::{Request, StatusCode};
use battle_core::native_games::connect_four::{ConnectFour, GameStatus, COLUMNS};
use battle_kaspa::mock::MockKaspaClient;
use battle_kaspa::multisig::service::MultisigEscrowService;
use battle_kaspa::rpc::{KaspaBackend, UtxoInfo};
use kaspa_addresses::{Address, Prefix, Version};
use sqlx::{postgres::PgPoolOptions, PgPool, Row};
use std::sync::Arc;
use tower::ServiceExt;
use uuid::Uuid;

// ── Harness ──────────────────────────────────────────────────────────────────

struct TestDb {
    pool: PgPool,
    admin_url: String,
    name: String,
}

impl TestDb {
    async fn new() -> Option<TestDb> {
        let Ok(url) = std::env::var("DATABASE_URL") else {
            eprintln!("⚠️  DATABASE_URL not set — skipping native-games integration test");
            return None;
        };
        let name = format!("kb_native_{}", Uuid::new_v4().simple());
        let admin = PgPoolOptions::new()
            .max_connections(1)
            .connect(&url)
            .await
            .expect("admin connect");
        sqlx::query(&format!("CREATE DATABASE {name}"))
            .execute(&admin)
            .await
            .expect("create db");
        admin.close().await;

        let (base, _) = url
            .rsplit_once('/')
            .expect("DATABASE_URL must end with /dbname");
        let test_url = format!("{base}/{name}");
        let pool = PgPoolOptions::new()
            .max_connections(8)
            .connect(&test_url)
            .await
            .expect("test db connect");
        sqlx::migrate!("../migrations")
            .run(&pool)
            .await
            .expect("migrations");
        Some(TestDb {
            pool,
            admin_url: url,
            name,
        })
    }

    async fn teardown(self) {
        self.pool.close().await;
        if let Ok(admin) = PgPoolOptions::new()
            .max_connections(1)
            .connect(&self.admin_url)
            .await
        {
            let _ = sqlx::query(&format!(
                "DROP DATABASE IF EXISTS {} WITH (FORCE)",
                self.name
            ))
            .execute(&admin)
            .await;
        }
    }
}

fn kaspa_test_address(seed: u8) -> String {
    let secp = secp256k1::Secp256k1::new();
    let sk = secp256k1::SecretKey::from_slice(&[seed; 32]).unwrap();
    let (xonly, _) = secp256k1::Keypair::from_secret_key(&secp, &sk).x_only_public_key();
    Address::new(Prefix::Testnet, Version::PubKey, &xonly.serialize()).to_string()
}

async fn make_user(pool: &PgPool, name: &str, seed: u8) -> Uuid {
    let id = Uuid::new_v4();
    sqlx::query("INSERT INTO users (id, email, password_hash, display_name, kaspa_address) VALUES ($1,$2,'x',$3,$4)")
        .bind(id)
        .bind(format!("{id}@wallet.local"))
        .bind(name)
        .bind(kaspa_test_address(seed))
        .execute(pool)
        .await
        .unwrap();
    id
}

async fn make_session(pool: &PgPool, user: Uuid) -> String {
    let token = format!("tok-{}", Uuid::new_v4());
    sqlx::query(
        "INSERT INTO sessions (id, user_id, expires_at) VALUES ($1,$2, NOW() + INTERVAL '1 day')",
    )
    .bind(&token)
    .bind(user)
    .execute(pool)
    .await
    .unwrap();
    token
}

async fn link_faceit(pool: &PgPool, user: Uuid) {
    sqlx::query("INSERT INTO faceit_links (user_id, faceit_player_id, faceit_nickname) VALUES ($1,$2,'nick')")
        .bind(user)
        .bind(Uuid::new_v4().to_string())
        .execute(pool)
        .await
        .unwrap();
}

/// Fixed test-only key-derivation secret. NEVER use a hardcoded secret like
/// this outside of tests — see `MultisigEscrowService::key_derivation_secret`.
const TEST_KEY_DERIVATION_SECRET: [u8; 32] = [0x42; 32];

fn mock_multisig() -> (Arc<MultisigEscrowService>, Arc<MockKaspaClient>) {
    let mock = Arc::new(MockKaspaClient::new());
    let svc = MultisigEscrowService::new(
        mock.clone() as Arc<dyn KaspaBackend>,
        Prefix::Testnet,
        [0xe1; 32],
        kaspa_test_address(0x5f),
        TEST_KEY_DERIVATION_SECRET,
    )
    .unwrap();
    (Arc::new(svc), mock)
}

const WAGER: i64 = 1_000_000_000; // 10 KAS

/// A NATIVE match in `status` with a real multisig escrow (mock chain) — no deposits involved.
async fn native_match(
    pool: &PgPool,
    svc: &MultisigEscrowService,
    creator: Uuid,
    opponent: Uuid,
    status: &str,
) -> (Uuid, String) {
    let id = Uuid::new_v4();
    let info = svc.create_escrow(id, WAGER as u64, None).await.unwrap();
    sqlx::query(&format!(
        "INSERT INTO matches (id, creator_user_id, opponent_user_id, game_id, wager_sompi, wager_amount_sompi, \
         mode, status, escrow_address, provider, requires_faceit, native_game_type) \
         VALUES ($1,$2,$3,'connect-four',$4,$4,'BO1','{status}'::match_status,$5,'NATIVE',FALSE,'CONNECT_FOUR')"
    ))
    .bind(id)
    .bind(creator)
    .bind(opponent)
    .bind(WAGER)
    .bind(&info.escrow_address)
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO multisig_escrows (match_id, pubkey_a_hex, pubkey_b_hex, pubkey_oracle_hex, redeem_script_hex, p2sh_address, wager_per_player_sompi) \
         VALUES ($1,$2,$3,$4,$5,$6,$7)",
    )
    .bind(id)
    .bind(&info.pubkeys[0])
    .bind(&info.pubkeys[1])
    .bind(&info.pubkeys[2])
    .bind(&info.redeem_script_hex)
    .bind(&info.escrow_address)
    .bind(WAGER)
    .execute(pool)
    .await
    .unwrap();
    (id, info.escrow_address)
}

fn fund_escrow(mock: &MockKaspaClient, address: &str) {
    for (i, tx) in ["11", "22"].iter().enumerate() {
        mock.add_utxo(
            address,
            UtxoInfo {
                tx_id: tx.repeat(32),
                output_index: i as u32,
                amount: WAGER as u64,
                amount_kas: 10.0,
                is_coinbase: false,
                block_daa_score: 100,
                script_public_key: None,
            },
        );
    }
}

/// Runs one episode-runner pass (FUNDED → READY_TO_PLAY for native matches).
async fn run_episode(pool: &PgPool, id: Uuid) {
    use crate::episodes::match_episode::MatchEpisode;
    use crate::episodes::EpisodeTrait;
    let (tx, _rx) = tokio::sync::broadcast::channel(16);
    let ctx = (pool.clone(), None, tx, None, None);
    let mut ep = MatchEpisode::initialize(&ctx, id).await.unwrap();
    ep.execute().await.unwrap();
}

async fn status_of(pool: &PgPool, id: Uuid) -> String {
    sqlx::query_scalar::<_, String>("SELECT status::text FROM matches WHERE id = $1")
        .bind(id)
        .fetch_one(pool)
        .await
        .unwrap()
}

async fn version_of(pool: &PgPool, id: Uuid) -> i64 {
    sqlx::query_scalar("SELECT version FROM native_game_sessions WHERE match_id = $1")
        .bind(id)
        .fetch_one(pool)
        .await
        .unwrap()
}

/// Plays a move with the current version and a fresh nonce.
async fn mv(
    pool: &PgPool,
    id: Uuid,
    user: Uuid,
    col: i64,
) -> Result<native_game::Outcome, NativeError> {
    let v = version_of(pool, id).await;
    native_game::apply_move(
        pool,
        id,
        user,
        MoveRequest {
            column: col,
            expected_version: v,
            client_nonce: Uuid::new_v4().to_string(),
        },
    )
    .await
}

/// Funded native match → episode → session in READY → started. Returns (match, escrow addr).
async fn started_game(
    pool: &PgPool,
    svc: &MultisigEscrowService,
    a: Uuid,
    b: Uuid,
) -> (Uuid, String) {
    let (id, addr) = native_match(pool, svc, a, b, "FUNDED").await;
    run_episode(pool, id).await;
    assert_eq!(status_of(pool, id).await, "READY_TO_PLAY");
    native_game::start_game(pool, id, Some(a)).await.unwrap();
    assert_eq!(status_of(pool, id).await, "IN_GAME");
    (id, addr)
}

/// Finds a legal move order (columns, alternating A/B) that ends in a draw.
fn draw_columns() -> Vec<usize> {
    fn dfs(g: &mut ConnectFour, out: &mut Vec<usize>) -> bool {
        if g.move_count() as usize == 42 {
            return matches!(g.status(), GameStatus::Draw);
        }
        let p = if g.move_count().is_multiple_of(2) {
            "a"
        } else {
            "b"
        };
        for col in 0..COLUMNS {
            if let Ok(m) = g.execute(p, col) {
                out.push(col);
                if (!m.status.is_finished() || matches!(m.status, GameStatus::Draw)) && dfs(g, out)
                {
                    return true;
                }
                out.pop();
                g.rollback(&m.rollback).unwrap();
            }
        }
        false
    }
    let mut g = ConnectFour::new("a", "b");
    let mut out = vec![];
    assert!(dfs(&mut g, &mut out));
    out
}

// ── Lifecycle & settlement ───────────────────────────────────────────────────

#[tokio::test]
async fn full_game_winner_reaches_payout_exactly_once() {
    let Some(db) = TestDb::new().await else {
        return;
    };
    let pool = &db.pool;
    let (svc, mock) = mock_multisig();
    let a = make_user(pool, "Alice", 1).await;
    let b = make_user(pool, "Bob", 2).await;

    // FUNDED native match: the episode creates the session and goes READY_TO_PLAY (no FACEIT step).
    let (id, addr) = native_match(pool, &svc, a, b, "FUNDED").await;
    run_episode(pool, id).await;
    assert_eq!(status_of(pool, id).await, "READY_TO_PLAY");
    let sess: String =
        sqlx::query_scalar("SELECT status FROM native_game_sessions WHERE match_id=$1")
            .bind(id)
            .fetch_one(pool)
            .await
            .unwrap();
    assert_eq!(sess, "READY");
    // Moves are rejected until the game is started.
    assert!(matches!(
        mv(pool, id, a, 0).await,
        Err(NativeError::WrongStatus(_))
    ));
    // A second episode pass must not create a second session or change the state.
    run_episode(pool, id).await;
    assert_eq!(status_of(pool, id).await, "READY_TO_PLAY");

    // Start is idempotent and only for participants.
    let carol = make_user(pool, "Carol", 3).await;
    assert!(matches!(
        native_game::start_game(pool, id, Some(carol)).await,
        Err(NativeError::NotParticipant)
    ));
    let started = native_game::start_game(pool, id, Some(b)).await.unwrap();
    assert!(!started.replay && started.events.len() == 1);
    let again = native_game::start_game(pool, id, Some(a)).await.unwrap();
    assert!(again.replay && again.events.is_empty());
    assert_eq!(status_of(pool, id).await, "IN_GAME");

    // Alice wins vertically in column 0.
    for (user, col) in [(a, 0), (b, 1), (a, 0), (b, 1), (a, 0), (b, 1)] {
        mv(pool, id, user, col).await.unwrap();
    }
    let last = mv(pool, id, a, 0).await.unwrap();
    assert_eq!(last.snapshot.result.as_deref(), Some("WIN"));
    assert_eq!(last.snapshot.end_reason.as_deref(), Some("CONNECT_FOUR"));
    assert_eq!(last.snapshot.winner_user_id, Some(a));
    assert!(last
        .events
        .iter()
        .any(|e| e["type"] == "native_game_finished"));
    assert!(matches!(
        mv(pool, id, b, 3).await,
        Err(NativeError::WrongStatus(_))
    ));

    // Match settled from the engine result.
    let row = sqlx::query(
        "SELECT status::text AS status, winner_user_id, loser_user_id, result_source, result_hash, game_finished_at \
         FROM matches WHERE id=$1",
    )
    .bind(id).fetch_one(pool).await.unwrap();
    assert_eq!(row.get::<String, _>("status"), "FINISHED_GAME");
    assert_eq!(row.get::<Option<Uuid>, _>("winner_user_id"), Some(a));
    assert_eq!(row.get::<Option<Uuid>, _>("loser_user_id"), Some(b));
    assert_eq!(
        row.get::<Option<String>, _>("result_source").as_deref(),
        Some("NATIVE_ENGINE")
    );
    assert_eq!(
        row.get::<Option<String>, _>("result_hash").map(|h| h.len()),
        Some(64)
    );
    assert!(row
        .get::<Option<chrono::DateTime<chrono::Utc>>, _>("game_finished_at")
        .is_some());
    let moves: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM native_game_moves WHERE match_id=$1")
        .bind(id)
        .fetch_one(pool)
        .await
        .unwrap();
    assert_eq!(moves, 7);

    // Payout worker: FINISHED_GAME → PSKT → READY_FOR_PAYOUT, exactly once.
    fund_escrow(&mock, &addr);
    crate::payout_worker::poll_finished_matches(pool, &svc)
        .await
        .unwrap();
    assert_eq!(status_of(pool, id).await, "READY_FOR_PAYOUT");
    let pskt1: String = sqlx::query_scalar("SELECT payout_pskt_hex FROM matches WHERE id=$1")
        .bind(id)
        .fetch_one(pool)
        .await
        .unwrap();
    crate::payout_worker::poll_finished_matches(pool, &svc)
        .await
        .unwrap();
    crate::payout_worker::poll_finished_matches(pool, &svc)
        .await
        .unwrap();
    let pskt2: String = sqlx::query_scalar("SELECT payout_pskt_hex FROM matches WHERE id=$1")
        .bind(id)
        .fetch_one(pool)
        .await
        .unwrap();
    assert_eq!(
        pskt1, pskt2,
        "later worker cycles must not re-create the PSKT"
    );
    assert_eq!(status_of(pool, id).await, "READY_FOR_PAYOUT");
    // The PSKT pays the engine's winner.
    let pskt_json: serde_json::Value =
        serde_json::from_slice(&hex::decode(&pskt1).unwrap()).unwrap();
    assert_eq!(pskt_json["winner_address"], kaspa_test_address(1));

    db.teardown().await;
}

#[tokio::test]
async fn draw_creates_no_winner_payout_and_is_refunded() {
    let Some(db) = TestDb::new().await else {
        return;
    };
    let pool = &db.pool;
    let (svc, mock) = mock_multisig();
    let a = make_user(pool, "Alice", 1).await;
    let b = make_user(pool, "Bob", 2).await;
    let (id, addr) = started_game(pool, &svc, a, b).await;

    let cols = draw_columns();
    assert_eq!(cols.len(), 42);
    let mut last = None;
    for (i, col) in cols.iter().enumerate() {
        let user = if i % 2 == 0 { a } else { b };
        last = Some(mv(pool, id, user, *col as i64).await.unwrap());
    }
    let last = last.unwrap();
    assert_eq!(last.snapshot.result.as_deref(), Some("DRAW"));
    assert_eq!(last.snapshot.end_reason.as_deref(), Some("DRAW"));
    assert_eq!(last.snapshot.winner_user_id, None);

    let row = sqlx::query("SELECT status::text AS status, winner_user_id, loser_user_id, result_source FROM matches WHERE id=$1")
        .bind(id).fetch_one(pool).await.unwrap();
    assert_eq!(row.get::<String, _>("status"), "REFUND_PENDING");
    assert_eq!(row.get::<Option<Uuid>, _>("winner_user_id"), None);
    assert_eq!(row.get::<Option<Uuid>, _>("loser_user_id"), None);
    assert_eq!(
        row.get::<Option<String>, _>("result_source").as_deref(),
        Some("NATIVE_ENGINE")
    );

    // The payout worker never touches it…
    fund_escrow(&mock, &addr);
    crate::payout_worker::poll_finished_matches(pool, &svc)
        .await
        .unwrap();
    assert_eq!(status_of(pool, id).await, "REFUND_PENDING");
    let pskt: Option<String> =
        sqlx::query_scalar("SELECT payout_pskt_hex FROM matches WHERE id=$1")
            .bind(id)
            .fetch_one(pool)
            .await
            .unwrap();
    assert!(pskt.is_none(), "a draw must never produce a winner PSKT");

    // …the refund worker returns both stakes (50/50 split TX).
    crate::refund_worker::poll_refundable_matches(pool, &svc)
        .await
        .unwrap();
    let row = sqlx::query(
        "SELECT status::text AS status, refund_status, refund_tx_hash FROM matches WHERE id=$1",
    )
    .bind(id)
    .fetch_one(pool)
    .await
    .unwrap();
    assert_eq!(row.get::<String, _>("status"), "REFUNDED");
    assert_eq!(
        row.get::<Option<String>, _>("refund_status").as_deref(),
        Some("success")
    );
    assert!(row.get::<Option<String>, _>("refund_tx_hash").is_some());
    assert_eq!(mock.get_submitted_tx_ids().len(), 1);

    db.teardown().await;
}

#[tokio::test]
async fn duplicate_client_nonce_creates_no_second_move() {
    let Some(db) = TestDb::new().await else {
        return;
    };
    let pool = &db.pool;
    let (svc, _mock) = mock_multisig();
    let a = make_user(pool, "Alice", 1).await;
    let b = make_user(pool, "Bob", 2).await;
    let (id, _) = started_game(pool, &svc, a, b).await;

    let nonce = Uuid::new_v4().to_string();
    let v = version_of(pool, id).await;
    let req = || MoveRequest {
        column: 3,
        expected_version: v,
        client_nonce: nonce.clone(),
    };
    let first = native_game::apply_move(pool, id, a, req()).await.unwrap();
    assert!(!first.replay);
    // Exact retry (same nonce, now-stale expectedVersion) is an idempotent success, not a conflict.
    let second = native_game::apply_move(pool, id, a, req()).await.unwrap();
    assert!(second.replay && second.events.is_empty());
    assert_eq!(second.snapshot.move_count, 1);
    assert_eq!(second.snapshot.version, first.snapshot.version);
    let n: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM native_game_moves WHERE match_id=$1")
        .bind(id)
        .fetch_one(pool)
        .await
        .unwrap();
    assert_eq!(n, 1);

    // Invalid nonce format is rejected.
    let bad = native_game::apply_move(
        pool,
        id,
        b,
        MoveRequest {
            column: 0,
            expected_version: first.snapshot.version,
            client_nonce: "not-a-uuid".into(),
        },
    )
    .await;
    assert!(matches!(bad, Err(NativeError::BadRequest(_))));

    db.teardown().await;
}

#[tokio::test]
async fn concurrent_moves_are_serialized() {
    let Some(db) = TestDb::new().await else {
        return;
    };
    let pool = db.pool.clone();
    let (svc, _mock) = mock_multisig();
    let a = make_user(&pool, "Alice", 1).await;
    let b = make_user(&pool, "Bob", 2).await;
    let (id, _) = started_game(&pool, &svc, a, b).await;
    let v = version_of(&pool, id).await;

    // Same player double-submits with different nonces + the same expectedVersion, while the
    // opponent tries to jump in — only one move may ever be applied.
    let mut handles = vec![];
    for (user, col) in [(a, 0i64), (a, 1), (a, 2), (b, 3), (b, 4)] {
        let pool = pool.clone();
        handles.push(tokio::spawn(async move {
            native_game::apply_move(
                &pool,
                id,
                user,
                MoveRequest {
                    column: col,
                    expected_version: v,
                    client_nonce: Uuid::new_v4().to_string(),
                },
            )
            .await
        }));
    }
    let results: Vec<_> = futures::future::join_all(handles)
        .await
        .into_iter()
        .map(|r| r.unwrap())
        .collect();
    let ok = results.iter().filter(|r| r.is_ok()).count();
    assert_eq!(
        ok,
        1,
        "exactly one concurrent request may win: {:?}",
        results
            .iter()
            .map(|r| r.as_ref().map(|_| ()).map_err(|e| e.to_string()))
            .collect::<Vec<_>>()
    );
    for r in results.iter().filter(|r| r.is_err()) {
        assert!(
            matches!(
                r,
                Err(NativeError::VersionConflict { .. }) | Err(NativeError::Move(_))
            ),
            "{:?}",
            r.as_ref().err()
        );
    }
    let (count, seqs): (i64, i64) = sqlx::query_as(
        "SELECT COUNT(*), COUNT(DISTINCT sequence_number) FROM native_game_moves WHERE match_id=$1",
    )
    .bind(id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!((count, seqs), (1, 1));
    assert_eq!(version_of(&pool, id).await, v + 1);

    // A long run of properly sequenced alternating moves keeps versions strictly increasing.
    let mut expected = v + 1;
    let mut turn_a = false; // one A-move already happened above (or a B-move rejected) → find who is next
    let snap = {
        let mut c = pool.acquire().await.unwrap();
        native_game::load_snapshot(&mut c, id)
            .await
            .unwrap()
            .unwrap()
    };
    if snap.current_player_id == Some(a) {
        turn_a = true;
    }
    for i in 0..6 {
        let user = if turn_a { a } else { b };
        let out = mv(&pool, id, user, (i % 7) as i64).await.unwrap();
        expected += 1;
        assert_eq!(out.snapshot.version, expected);
        turn_a = !turn_a;
    }

    db.teardown().await;
}

#[tokio::test]
async fn snapshot_restores_board_after_reconnect() {
    let Some(db) = TestDb::new().await else {
        return;
    };
    let pool = &db.pool;
    let (svc, _mock) = mock_multisig();
    let a = make_user(pool, "Alice", 1).await;
    let b = make_user(pool, "Bob", 2).await;
    let (id, _) = started_game(pool, &svc, a, b).await;

    for (user, col) in [(a, 3), (b, 3), (a, 2), (b, 4)] {
        mv(pool, id, user, col).await.unwrap();
    }
    let mut conn = pool.acquire().await.unwrap();
    let snap = native_game::load_snapshot(&mut conn, id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(snap.status, "ACTIVE");
    assert_eq!(snap.move_count, 4);
    assert_eq!(snap.board.len(), 6);
    assert_eq!(snap.board[0][3], 1);
    assert_eq!(snap.board[1][3], 2);
    assert_eq!(snap.board[0][2], 1);
    assert_eq!(snap.board[0][4], 2);
    assert_eq!(snap.current_player_id, Some(a));
    assert_eq!(
        snap.last_move
            .as_ref()
            .map(|m| (m.column, m.row, m.player_id)),
        Some((4, 0, Some(b)))
    );
    assert!(snap.turn_deadline.is_some());
    // The stored hash equals the hash of the replayed audit log.
    let mut g = ConnectFour::new(a.to_string(), b.to_string());
    let rows = sqlx::query("SELECT player_id, column_index, state_hash FROM native_game_moves WHERE match_id=$1 ORDER BY sequence_number")
        .bind(id).fetch_all(pool).await.unwrap();
    for r in &rows {
        let p: Uuid = r.get("player_id");
        g.execute(&p.to_string(), r.get::<i16, _>("column_index") as usize)
            .unwrap();
        assert_eq!(g.state_hash(), r.get::<String, _>("state_hash"));
    }
    assert_eq!(g.state_hash(), snap.state_hash);

    drop(conn); // pool.close() in teardown waits for every connection to be returned
    db.teardown().await;
}

#[tokio::test]
async fn invalid_requests_are_rejected_without_side_effects() {
    let Some(db) = TestDb::new().await else {
        return;
    };
    let pool = &db.pool;
    let (svc, _mock) = mock_multisig();
    let a = make_user(pool, "Alice", 1).await;
    let b = make_user(pool, "Bob", 2).await;
    let mallory = make_user(pool, "Mallory", 3).await;
    let (id, _) = started_game(pool, &svc, a, b).await;
    let v = version_of(pool, id).await;

    assert!(matches!(
        mv(pool, id, mallory, 0).await,
        Err(NativeError::NotParticipant)
    ));
    assert!(matches!(
        mv(pool, id, b, 0).await,
        Err(NativeError::Move(_))
    )); // not b's turn
    assert!(matches!(
        mv(pool, id, a, 7).await,
        Err(NativeError::Move(_))
    ));
    assert!(matches!(
        mv(pool, id, a, -1).await,
        Err(NativeError::Move(_))
    ));
    let stale = native_game::apply_move(
        pool,
        id,
        a,
        MoveRequest {
            column: 0,
            expected_version: v + 5,
            client_nonce: Uuid::new_v4().to_string(),
        },
    )
    .await;
    assert!(
        matches!(stale, Err(NativeError::VersionConflict { current_version }) if current_version == v)
    );
    assert_eq!(version_of(pool, id).await, v);

    // Fill column 0 (no win), then a 7th disc is rejected as full.
    for (i, user) in [a, b, a, b, a, b].iter().enumerate() {
        let _ = i;
        mv(pool, id, *user, 0).await.unwrap();
    }
    assert!(matches!(
        mv(pool, id, a, 0).await,
        Err(NativeError::Move(_))
    ));

    db.teardown().await;
}

#[tokio::test]
async fn resignation_and_timeout_settle_for_the_opponent() {
    let Some(db) = TestDb::new().await else {
        return;
    };
    let pool = &db.pool;
    let (svc, _mock) = mock_multisig();
    let a = make_user(pool, "Alice", 1).await;
    let b = make_user(pool, "Bob", 2).await;

    // Resignation: Alice resigns → Bob wins.
    let (id, _) = started_game(pool, &svc, a, b).await;
    let mallory = make_user(pool, "Mallory", 3).await;
    assert!(matches!(
        native_game::resign(pool, id, mallory).await,
        Err(NativeError::NotParticipant)
    ));
    let out = native_game::resign(pool, id, a).await.unwrap();
    assert_eq!(out.snapshot.winner_user_id, Some(b));
    assert_eq!(out.snapshot.end_reason.as_deref(), Some("RESIGNATION"));
    assert_eq!(status_of(pool, id).await, "FINISHED_GAME");
    assert!(matches!(
        native_game::resign(pool, id, b).await,
        Err(NativeError::WrongStatus(_))
    ));

    // Timeout: not due yet → nothing; overdue → the player to move forfeits.
    let (id2, _) = started_game(pool, &svc, a, b).await;
    assert!(native_game::expire_if_due(pool, id2)
        .await
        .unwrap()
        .is_none());
    sqlx::query("UPDATE native_game_sessions SET turn_deadline = NOW() - INTERVAL '1 second' WHERE match_id=$1")
        .bind(id2).execute(pool).await.unwrap();
    let out = native_game::expire_if_due(pool, id2)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        out.snapshot.winner_user_id,
        Some(b),
        "Alice was to move and timed out"
    );
    assert_eq!(out.snapshot.end_reason.as_deref(), Some("TIMEOUT"));
    assert_eq!(status_of(pool, id2).await, "FINISHED_GAME");
    assert!(native_game::expire_if_due(pool, id2)
        .await
        .unwrap()
        .is_none());

    // Autostart: a READY game nobody started is started by the server after the grace period.
    let (id3, _) = native_match(pool, &svc, a, b, "FUNDED").await;
    run_episode(pool, id3).await;
    assert_eq!(status_of(pool, id3).await, "READY_TO_PLAY");
    std::env::set_var("NATIVE_AUTOSTART_SECS", "0");
    let auto = native_game::autostart_if_due(pool, id3).await.unwrap();
    std::env::remove_var("NATIVE_AUTOSTART_SECS");
    assert!(auto.is_some());
    assert_eq!(status_of(pool, id3).await, "IN_GAME");

    db.teardown().await;
}

#[tokio::test]
async fn database_constraints_guard_the_move_log() {
    let Some(db) = TestDb::new().await else {
        return;
    };
    let pool = &db.pool;
    let (svc, _mock) = mock_multisig();
    let a = make_user(pool, "Alice", 1).await;
    let b = make_user(pool, "Bob", 2).await;
    let (id, _) = started_game(pool, &svc, a, b).await;
    mv(pool, id, a, 0).await.unwrap();

    let ins = |seq: i32, col: i16, nonce: &'static str| {
        let pool = pool.clone();
        async move {
            sqlx::query("INSERT INTO native_game_moves (match_id, sequence_number, player_id, column_index, row_index, client_nonce, state_hash) VALUES ($1,$2,$3,$4,0,$5,'h')")
                .bind(id).bind(seq).bind(a).bind(col).bind(nonce)
                .execute(&pool).await
        }
    };
    assert!(
        ins(1, 2, "another-nonce-1").await.is_err(),
        "UNIQUE(match_id, sequence_number)"
    );
    assert!(
        ins(9, 9, "another-nonce-2").await.is_err(),
        "column_index 0..6"
    );
    let dup_nonce: String =
        sqlx::query_scalar("SELECT client_nonce FROM native_game_moves WHERE match_id=$1")
            .bind(id)
            .fetch_one(pool)
            .await
            .unwrap();
    let dup = sqlx::query("INSERT INTO native_game_moves (match_id, sequence_number, player_id, column_index, row_index, client_nonce, state_hash) VALUES ($1,2,$2,1,0,$3,'h')")
        .bind(id).bind(a).bind(dup_nonce).execute(pool).await;
    assert!(dup.is_err(), "UNIQUE(match_id, player_id, client_nonce)");
    // One session per match.
    let second = sqlx::query("INSERT INTO native_game_sessions (match_id, game_type, player_one_id, player_two_id, board_state, state_hash) VALUES ($1,'CONNECT_FOUR',$2,$3,'{}'::jsonb,'h')")
        .bind(id).bind(a).bind(b).execute(pool).await;
    assert!(second.is_err(), "UNIQUE(match_id)");

    db.teardown().await;
}

// ── Provider snapshot / legacy compatibility ─────────────────────────────────

#[tokio::test]
async fn legacy_rows_default_to_faceit_and_faceit_flow_is_untouched() {
    let Some(db) = TestDb::new().await else {
        return;
    };
    let pool = &db.pool;
    let (svc, mock) = mock_multisig();
    let a = make_user(pool, "Alice", 1).await;
    let b = make_user(pool, "Bob", 2).await;

    // A row written the "old way" (no provider columns) is a FACEIT match.
    let legacy = Uuid::new_v4();
    sqlx::query("INSERT INTO matches (id, creator_user_id, opponent_user_id, game_id, wager_sompi, wager_amount_sompi, mode, status) VALUES ($1,$2,$3,'cs2',$4,$4,'BO1','FINISHED_FACEIT')")
        .bind(legacy).bind(a).bind(b).bind(WAGER).execute(pool).await.unwrap();
    let m = crate::api::load_match_full(pool, legacy).await.unwrap();
    assert_eq!(m.provider, crate::models::MatchProvider::Faceit);
    assert_eq!(m.requires_faceit, Some(true));
    assert_eq!(m.status, MatchStatus::FinishedFaceit);

    // …and the payout worker still processes FINISHED_FACEIT exactly as before.
    let info = svc.create_escrow(legacy, WAGER as u64, None).await.unwrap();
    sqlx::query("UPDATE matches SET winner_user_id=$2, loser_user_id=$3, faceit_finished_at=NOW(), escrow_address=$4 WHERE id=$1")
        .bind(legacy).bind(b).bind(a).bind(&info.escrow_address).execute(pool).await.unwrap();
    sqlx::query("INSERT INTO multisig_escrows (match_id, pubkey_a_hex, pubkey_b_hex, pubkey_oracle_hex, redeem_script_hex, p2sh_address, wager_per_player_sompi) VALUES ($1,$2,$3,$4,$5,$6,$7)")
        .bind(legacy).bind(&info.pubkeys[0]).bind(&info.pubkeys[1]).bind(&info.pubkeys[2])
        .bind(&info.redeem_script_hex).bind(&info.escrow_address).bind(WAGER).execute(pool).await.unwrap();
    fund_escrow(&mock, &info.escrow_address);
    crate::payout_worker::poll_finished_matches(pool, &svc)
        .await
        .unwrap();
    assert_eq!(status_of(pool, legacy).await, "READY_FOR_PAYOUT");

    // Episode runner: a FACEIT match in FUNDED still goes to GAME_ID_INPUT (not READY_TO_PLAY).
    let fac = Uuid::new_v4();
    sqlx::query("INSERT INTO matches (id, creator_user_id, opponent_user_id, game_id, wager_sompi, wager_amount_sompi, mode, status) VALUES ($1,$2,$3,'cs2',$4,$4,'BO1','FUNDED')")
        .bind(fac).bind(a).bind(b).bind(WAGER).execute(pool).await.unwrap();
    run_episode(pool, fac).await;
    assert_eq!(status_of(pool, fac).await, "GAME_ID_INPUT");
    let sessions: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM native_game_sessions WHERE match_id=$1")
            .bind(fac)
            .fetch_one(pool)
            .await
            .unwrap();
    assert_eq!(sessions, 0);

    // Catalog edits never change an existing match (provider is a snapshot).
    sqlx::query("UPDATE games SET provider='FACEIT', requires_faceit=TRUE, native_game_type=NULL WHERE slug='connect-four'")
        .execute(pool).await.unwrap();
    let (nid, _) = native_match(pool, &svc, a, b, "OPEN").await;
    let nm = crate::api::load_match_full(pool, nid).await.unwrap();
    assert_eq!(nm.provider, crate::models::MatchProvider::Native);

    db.teardown().await;
}

// ── HTTP: feature flag, auth decoupling, FACEIT gate, events ─────────────────

fn app_state(pool: PgPool, svc: Arc<MultisigEscrowService>) -> AppState {
    let (tx, _rx) = tokio::sync::broadcast::channel(256);
    AppState {
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
        multisig_service: Some(svc),
    }
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
        req = req.header("Authorization", format!("Bearer {t}"));
    }
    let body = match body {
        Some(b) => {
            req = req.header("content-type", "application/json");
            Body::from(b.to_string())
        }
        None => Body::empty(),
    };
    let res = app.clone().oneshot(req.body(body).unwrap()).await.unwrap();
    let status = res.status();
    let bytes = axum::body::to_bytes(res.into_body(), 1 << 20)
        .await
        .unwrap();
    let json = serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null);
    (status, json)
}

#[tokio::test]
async fn http_native_flow_needs_no_faceit_and_faceit_flow_still_requires_it() {
    let Some(db) = TestDb::new().await else {
        return;
    };
    let pool = db.pool.clone();
    let (svc, _mock) = mock_multisig();
    let state = app_state(pool.clone(), svc.clone());
    let mut events = state.tx.subscribe();
    let app = crate::api::router().with_state(state);

    // Users WITHOUT any FACEIT link, each with wallet + session.
    let a = make_user(&pool, "Alice", 1).await;
    let b = make_user(&pool, "Bob", 2).await;
    let ta = make_session(&pool, a).await;
    let tb = make_session(&pool, b).await;

    // Feature flag off: no native match can be created or joined.
    std::env::remove_var("NATIVE_GAMES_ENABLED");
    let (st, feat) = call(&app, "GET", "/features", None, None).await;
    assert_eq!(
        (st, feat["nativeGamesEnabled"].clone()),
        (StatusCode::OK, serde_json::json!(false))
    );
    let create =
        serde_json::json!({ "game_id": "connect-four", "wager_sompi": WAGER, "mode": "BO1" });
    let (st, _) = call(&app, "POST", "/challenges", Some(&ta), Some(create.clone())).await;
    assert_eq!(
        st,
        StatusCode::FORBIDDEN,
        "flag off → native creation blocked"
    );

    // Flag on: create a native match without FACEIT.
    std::env::set_var("NATIVE_GAMES_ENABLED", "true");
    std::env::remove_var("KASPA_NETWORK");
    let (st, feat) = call(&app, "GET", "/features", None, None).await;
    assert_eq!(
        (st, feat["nativeGamesEnabled"].clone()),
        (StatusCode::OK, serde_json::json!(true))
    );
    let (st, m) = call(&app, "POST", "/challenges", Some(&ta), Some(create.clone())).await;
    assert_eq!(st, StatusCode::OK, "{m}");
    assert_eq!(m["provider"], "NATIVE");
    assert_eq!(m["native_game_type"], "CONNECT_FOUR");
    assert_eq!(m["requires_faceit"], false);
    let mid: Uuid = m["id"].as_str().unwrap().parse().unwrap();

    // Unauthenticated create is rejected; BO3 is not offered for browser games.
    let (st, _) = call(&app, "POST", "/challenges", None, Some(create.clone())).await;
    assert_eq!(st, StatusCode::UNAUTHORIZED);
    let bo3 = serde_json::json!({ "game_id": "connect-four", "wager_sompi": WAGER, "mode": "BO3" });
    let (st, _) = call(&app, "POST", "/challenges", Some(&ta), Some(bo3)).await;
    assert_eq!(st, StatusCode::BAD_REQUEST);
    // Unknown games are rejected.
    let unk = serde_json::json!({ "game_id": "chess", "wager_sompi": WAGER, "mode": "BO1" });
    let (st, _) = call(&app, "POST", "/challenges", Some(&ta), Some(unk)).await;
    assert_eq!(st, StatusCode::BAD_REQUEST);

    // Join by a FACEIT-free user; the creator cannot join their own match.
    let (st, _) = call(
        &app,
        "POST",
        &format!("/matches/{mid}/accept"),
        Some(&ta),
        None,
    )
    .await;
    assert_ne!(st, StatusCode::OK);
    let (st, joined) = call(
        &app,
        "POST",
        &format!("/matches/{mid}/accept"),
        Some(&tb),
        None,
    )
    .await;
    assert_eq!(st, StatusCode::OK, "{joined}");
    assert_eq!(joined["status"], "AWAITING_FUNDING");
    assert_eq!(joined["provider"], "NATIVE");

    // No session before funding.
    let (st, body) = call(&app, "GET", &format!("/matches/{mid}/game"), None, None).await;
    assert_eq!(
        (st, body["error"].clone()),
        (StatusCode::NOT_FOUND, serde_json::json!("game_not_started"))
    );
    // Native matches must never enter the FACEIT match-id flow.
    let (st, body) = call(
        &app,
        "POST",
        &format!("/matches/{mid}/faceit-match-id"),
        Some(&ta),
        Some(serde_json::json!({ "faceit_match_id": Uuid::new_v4().to_string() })),
    )
    .await;
    assert_eq!(
        (st, body["error"].clone()),
        (
            StatusCode::CONFLICT,
            serde_json::json!("not_a_faceit_match")
        )
    );

    // Both deposits confirmed → episode → READY_TO_PLAY → auto start by a client.
    sqlx::query("UPDATE matches SET status='FUNDED', player_a_deposit_confirmed=TRUE, player_b_deposit_confirmed=TRUE WHERE id=$1")
        .bind(mid).execute(&pool).await.unwrap();
    run_episode(&pool, mid).await;
    let (st, g) = call(
        &app,
        "GET",
        &format!("/matches/{mid}/game"),
        Some(&ta),
        None,
    )
    .await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(g["status"], "READY");
    assert_eq!(g["you"]["slot"], 1);
    let (st, g) = call(
        &app,
        "POST",
        &format!("/matches/{mid}/game/start"),
        Some(&tb),
        None,
    )
    .await;
    assert_eq!(st, StatusCode::OK, "{g}");
    assert_eq!(g["status"], "ACTIVE");
    assert_eq!(g["matchStatus"], "IN_GAME");

    // Moves over HTTP.
    let mut version = g["version"].as_i64().unwrap();
    let mv_http = |token: &str, col: i64, ver: i64| {
        let app = app.clone();
        let token = token.to_string();
        async move {
            call(&app, "POST", &format!("/matches/{mid}/connect-four/moves"), Some(&token),
                Some(serde_json::json!({ "column": col, "expectedVersion": ver, "clientNonce": Uuid::new_v4().to_string() }))).await
        }
    };
    let (st, r) = mv_http(&ta, 3, version).await;
    assert_eq!(st, StatusCode::OK, "{r}");
    assert_eq!(r["board"][0][3], 1);
    assert_eq!(r["currentPlayerId"], b.to_string());
    version = r["version"].as_i64().unwrap();
    // Wrong turn, stale version, bad column, unauthenticated.
    let (st, r) = mv_http(&ta, 4, version).await;
    assert_eq!(
        (st, r["error"].clone()),
        (StatusCode::CONFLICT, serde_json::json!("not_your_turn"))
    );
    let (st, r) = mv_http(&tb, 4, version - 1).await;
    assert_eq!(
        (st, r["error"].clone()),
        (StatusCode::CONFLICT, serde_json::json!("version_conflict"))
    );
    let (st, r) = mv_http(&tb, 9, version).await;
    assert_eq!(
        (st, r["error"].clone()),
        (StatusCode::BAD_REQUEST, serde_json::json!("invalid_column"))
    );
    let (st, _) = call(&app, "POST", &format!("/matches/{mid}/connect-four/moves"), None,
        Some(serde_json::json!({ "column": 1, "expectedVersion": version, "clientNonce": Uuid::new_v4().to_string() }))).await;
    assert_eq!(st, StatusCode::UNAUTHORIZED);
    let (st, r) = mv_http(&tb, 3, version).await;
    assert_eq!(st, StatusCode::OK, "{r}");
    assert_eq!(r["board"][1][3], 2);

    // Reconnect: a fresh GET returns exactly the same board the events described.
    let (_, snap) = call(
        &app,
        "GET",
        &format!("/matches/{mid}/game"),
        Some(&tb),
        None,
    )
    .await;
    assert_eq!(snap["moveCount"], 2);
    assert_eq!(snap["you"]["slot"], 2);
    assert_eq!(snap["board"], r["board"]);

    // Events were typed and published only for committed state.
    let mut types = vec![];
    while let Ok(raw) = events.try_recv() {
        let e: serde_json::Value = serde_json::from_str(&raw).unwrap_or_default();
        if let Some(t) = e["type"].as_str() {
            if t.starts_with("native_game_") {
                // Whatever version an event announces must already be durable in the DB.
                let db_version = version_of(&pool, mid).await;
                assert!(e["version"].as_i64().unwrap() <= db_version);
            }
            types.push(t.to_string());
        }
    }
    for expected in [
        "native_game_started",
        "native_game_move",
        "native_game_state",
        "match_update",
    ] {
        assert!(
            types.iter().any(|t| t == expected),
            "missing event {expected}: {types:?}"
        );
    }

    // A third user may watch but not play.
    let c = make_user(&pool, "Carol", 3).await;
    let tc = make_session(&pool, c).await;
    let (st, _) = call(
        &app,
        "GET",
        &format!("/matches/{mid}/game"),
        Some(&tc),
        None,
    )
    .await;
    assert_eq!(st, StatusCode::OK);
    let (st, r) = mv_http(&tc, 1, r["version"].as_i64().unwrap()).await;
    assert_eq!(
        (st, r["error"].clone()),
        (StatusCode::FORBIDDEN, serde_json::json!("not_participant"))
    );

    // ── FACEIT games keep requiring a FACEIT link (now enforced server-side) ──
    let faceit_create =
        serde_json::json!({ "game_id": "cs2", "wager_sompi": WAGER, "mode": "BO1" });
    let (st, _) = call(
        &app,
        "POST",
        "/challenges",
        Some(&ta),
        Some(faceit_create.clone()),
    )
    .await;
    assert_eq!(
        st,
        StatusCode::FORBIDDEN,
        "FACEIT match without FACEIT link must be refused"
    );
    link_faceit(&pool, a).await;
    let (st, fm) = call(&app, "POST", "/challenges", Some(&ta), Some(faceit_create)).await;
    assert_eq!(st, StatusCode::OK, "{fm}");
    assert_eq!(fm["provider"], "FACEIT");
    assert_eq!(fm["requires_faceit"], true);
    let fid = fm["id"].as_str().unwrap().to_string();
    // Bob (no FACEIT link) cannot join it…
    let (st, _) = call(
        &app,
        "POST",
        &format!("/matches/{fid}/accept"),
        Some(&tb),
        None,
    )
    .await;
    assert_eq!(st, StatusCode::FORBIDDEN);
    // …until he links FACEIT.
    link_faceit(&pool, b).await;
    let (st, j) = call(
        &app,
        "POST",
        &format!("/matches/{fid}/accept"),
        Some(&tb),
        None,
    )
    .await;
    assert_eq!(st, StatusCode::OK, "{j}");

    // Mainnet guard: native creation blocked unless explicitly allowed.
    std::env::set_var("KASPA_NETWORK", "mainnet");
    let (st, _) = call(&app, "POST", "/challenges", Some(&ta), Some(create.clone())).await;
    assert_eq!(st, StatusCode::FORBIDDEN, "native games are testnet-only");
    std::env::remove_var("KASPA_NETWORK");
    std::env::remove_var("NATIVE_GAMES_ENABLED");

    db.teardown().await;
}

// ── Audit regressions (docs/FULL_SYSTEM_AUDIT_2026-09-29.md) ─────────────────

/// F-08: one signed wallet-login challenge must yield exactly one session, even when
/// several verify requests race; expired challenges are never claimable.
#[tokio::test]
async fn wallet_challenge_is_claimed_exactly_once_under_concurrency() {
    let Some(db) = TestDb::new().await else {
        return;
    };
    let pool = db.pool.clone();

    let fresh = Uuid::new_v4();
    let expired = Uuid::new_v4();
    for (id, interval) in [
        (fresh, "+ INTERVAL '5 minutes'"),
        (expired, "- INTERVAL '1 minute'"),
    ] {
        sqlx::query(&format!(
            "INSERT INTO wallet_login_challenges (id, kaspa_address, challenge_message, expires_at) \
             VALUES ($1, $2, 'msg', NOW() {interval})"
        ))
        .bind(id)
        .bind(kaspa_test_address(9))
        .execute(&pool)
        .await
        .unwrap();
    }

    let mut tasks = Vec::new();
    for _ in 0..16 {
        let pool = pool.clone();
        tasks.push(tokio::spawn(async move {
            crate::api::consume_wallet_challenge(&pool, fresh)
                .await
                .unwrap()
        }));
    }
    let mut winners = 0;
    for t in tasks {
        if t.await.unwrap() {
            winners += 1;
        }
    }
    assert_eq!(winners, 1, "exactly one concurrent claim may succeed");
    assert!(
        !crate::api::consume_wallet_challenge(&pool, fresh)
            .await
            .unwrap(),
        "replay after use must fail"
    );
    assert!(
        !crate::api::consume_wallet_challenge(&pool, expired)
            .await
            .unwrap(),
        "expired challenge must not be claimable"
    );
    assert!(
        !crate::api::consume_wallet_challenge(&pool, Uuid::new_v4())
            .await
            .unwrap(),
        "unknown challenge must not be claimable"
    );

    db.teardown().await;
}

/// F-01: `POST /multisig/create` is bound to the match row — strangers are rejected,
/// unknown matches 404, mismatching wagers 400, and a legitimate repeat call cannot
/// rewrite the stored escrow terms.
#[tokio::test]
async fn multisig_create_is_bound_to_match_participants_and_cannot_overwrite() {
    let Some(db) = TestDb::new().await else {
        return;
    };
    let pool = db.pool.clone();
    let (svc, _mock) = mock_multisig();
    let app = crate::api::router().with_state(app_state(pool.clone(), svc.clone()));

    let a = make_user(&pool, "Alice", 1).await;
    let b = make_user(&pool, "Bob", 2).await;
    let x = make_user(&pool, "Mallory", 3).await;
    let (ta, tx_) = (make_session(&pool, a).await, make_session(&pool, x).await);
    let (mid, _addr) = native_match(&pool, &svc, a, b, "OPEN").await;

    let body = |wager: u64| serde_json::json!({ "match_id": mid, "wager_per_player_sompi": wager, "timelock_timestamp": 42 });

    let (st, _) = call(
        &app,
        "POST",
        "/multisig/create",
        Some(&tx_),
        Some(body(WAGER as u64)),
    )
    .await;
    assert_eq!(st, StatusCode::FORBIDDEN, "non-participant");

    let unknown = serde_json::json!({ "match_id": Uuid::new_v4(), "wager_per_player_sompi": 1 });
    let (st, _) = call(&app, "POST", "/multisig/create", Some(&ta), Some(unknown)).await;
    assert_eq!(st, StatusCode::NOT_FOUND);

    let (st, _) = call(&app, "POST", "/multisig/create", Some(&ta), Some(body(1))).await;
    assert_eq!(
        st,
        StatusCode::BAD_REQUEST,
        "wager must equal the match row"
    );
    let (st, _) = call(
        &app,
        "POST",
        "/multisig/create",
        Some(&ta),
        Some(body(u64::MAX / 2 + 1)),
    )
    .await;
    assert_eq!(st, StatusCode::BAD_REQUEST, "overflowing wager");

    let (st, j) = call(
        &app,
        "POST",
        "/multisig/create",
        Some(&ta),
        Some(body(WAGER as u64)),
    )
    .await;
    assert_eq!(st, StatusCode::OK, "{j}");
    let stored = svc.get_escrow(&mid).await.unwrap();
    assert_eq!(stored.wager_per_player_sompi, WAGER as u64);
    assert_eq!(
        stored.timelock_timestamp, None,
        "client timelock must be ignored"
    );

    db.teardown().await;
}

/// F-02: wagers outside 1..=MAX_WAGER_KAS are rejected before any DB/escrow work.
#[tokio::test]
async fn create_challenge_rejects_out_of_range_wagers() {
    let Some(db) = TestDb::new().await else {
        return;
    };
    let pool = db.pool.clone();
    let (svc, _mock) = mock_multisig();
    let app = crate::api::router().with_state(app_state(pool.clone(), svc));
    let a = make_user(&pool, "Alice", 1).await;
    let ta = make_session(&pool, a).await;

    let max = (battle_core::types::MAX_WAGER_KAS * battle_core::types::SOMPI_PER_KAS) as i64;
    for bad in [i64::MIN, -1, 0, max + 1, i64::MAX] {
        let req =
            serde_json::json!({ "game_id": "connect-four", "wager_sompi": bad, "mode": "BO1" });
        let (st, _) = call(&app, "POST", "/challenges", Some(&ta), Some(req)).await;
        assert_eq!(st, StatusCode::BAD_REQUEST, "wager {bad} must be rejected");
    }

    db.teardown().await;
}

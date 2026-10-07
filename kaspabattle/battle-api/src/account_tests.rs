//! Integration tests: e-mail accounts (register / verify / login / reset / change / newsletter),
//! session security and the Free Play lifecycle. Real Postgres, in-memory mailer, real router.
//! Needs `DATABASE_URL`; without it every test logs a notice and returns (CI provides Postgres).

use crate::account::{AccountConfig, AccountRuntime};
use crate::api::AppState;
use crate::mail::{DisabledMailer, Mailer, MemoryMailer};
use crate::native_tests::TestDb;
use axum::body::Body;
use axum::http::{header, HeaderMap, Request, StatusCode};
use sqlx::{PgPool, Row};
use std::sync::Arc;
use tower::ServiceExt;
use uuid::Uuid;

const PW: &str = "correct horse battery staple";

fn state_with(pool: PgPool, mailer: Arc<dyn Mailer>, require_verification: bool) -> AppState {
    let (tx, _rx) = tokio::sync::broadcast::channel(256);
    let mail_available = mailer.is_configured();
    let mut cfg = AccountConfig::from_env(mail_available, |k| match k {
        "PUBLIC_APP_URL" => Some("https://app.example.test".into()),
        _ => None,
    });
    cfg.require_email_verification = require_verification;
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
        // No Kaspa RPC, no escrow, no multisig: Free Play must work exactly like this.
        kaspa_rpc: None,
        payout_service: None,
        blockchain_watcher: None,
        multisig_service: None,
        account: AccountRuntime::new(cfg, mailer),
        presence: crate::free_play::Presence::default(),
    }
}

fn router(state: AppState) -> axum::Router {
    crate::api::router().with_state(state)
}

struct Resp {
    status: StatusCode,
    headers: HeaderMap,
    json: serde_json::Value,
}

impl Resp {
    /// The session token from `Set-Cookie`, if any.
    fn session_cookie(&self) -> Option<String> {
        self.headers
            .get_all(header::SET_COOKIE)
            .iter()
            .filter_map(|v| v.to_str().ok())
            .find_map(|c| {
                c.strip_prefix("kaspabattle-auth=")
                    .map(|r| r.split(';').next().unwrap().to_string())
            })
            .filter(|t| !t.is_empty())
    }
}

async fn send(
    app: &axum::Router,
    method: &str,
    uri: &str,
    token: Option<&str>,
    body: Option<serde_json::Value>,
) -> Resp {
    let mut req = Request::builder().method(method).uri(uri);
    if let Some(t) = token {
        req = req.header(header::COOKIE, format!("kaspabattle-auth={t}"));
    }
    let body = match body {
        Some(b) => {
            req = req.header(header::CONTENT_TYPE, "application/json");
            Body::from(b.to_string())
        }
        None => Body::empty(),
    };
    let res = app.clone().oneshot(req.body(body).unwrap()).await.unwrap();
    let status = res.status();
    let headers = res.headers().clone();
    let bytes = axum::body::to_bytes(res.into_body(), 1 << 20)
        .await
        .unwrap();
    Resp {
        status,
        headers,
        json: serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null),
    }
}

fn reg(username: &str, email: &str, password: &str) -> serde_json::Value {
    serde_json::json!({
        "username": username, "email": email, "password": password, "passwordConfirm": password,
        "acceptTerms": true, "newsletter": false, "website": ""
    })
}

async fn register(app: &axum::Router, username: &str, email: &str) -> Resp {
    send(
        app,
        "POST",
        "/auth/register",
        None,
        Some(reg(username, email, PW)),
    )
    .await
}

async fn login(app: &axum::Router, email: &str, password: &str) -> Resp {
    send(
        app,
        "POST",
        "/auth/login",
        None,
        Some(serde_json::json!({ "email": email, "password": password })),
    )
    .await
}

fn token_from_link(text: &str) -> String {
    let i = text.find("token=").expect("mail contains a token link") + 6;
    text[i..]
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
        .collect()
}

// ── Registration ─────────────────────────────────────────────────────────────

#[tokio::test]
async fn registration_stores_an_argon2id_hash_and_normalised_data() {
    let Some(db) = TestDb::new().await else {
        return;
    };
    let app = router(state_with(db.pool.clone(), Arc::new(DisabledMailer), false));

    let r = register(&app, "Alice_01", "  Alice@Example.TEST ").await;
    assert_eq!(r.status, StatusCode::CREATED);
    assert_eq!(r.json["status"], "registered");
    // the response never contains credentials or ids
    assert!(r.json.get("password").is_none() && r.json.get("userId").is_none());

    let row = sqlx::query("SELECT email, username, display_name, password_hash, has_password_login, email_verified, newsletter_consent_at FROM users WHERE lower(username) = 'alice_01'")
        .fetch_one(&db.pool).await.unwrap();
    assert_eq!(row.get::<String, _>("email"), "alice@example.test");
    assert_eq!(row.get::<String, _>("display_name"), "Alice_01");
    let hash: String = row.get("password_hash");
    assert!(
        hash.starts_with("$argon2id$v=19$m=19456,t=2,p=1$"),
        "{hash}"
    );
    assert!(!hash.contains(PW));
    assert!(row.get::<bool, _>("has_password_login"));
    assert!(!row.get::<bool, _>("email_verified"));
    assert!(
        row.get::<Option<chrono::DateTime<chrono::Utc>>, _>("newsletter_consent_at")
            .is_none(),
        "newsletter is opt-in"
    );
    db.teardown().await;
}

#[tokio::test]
async fn duplicate_email_is_indistinguishable_but_duplicate_username_is_reported() {
    let Some(db) = TestDb::new().await else {
        return;
    };
    let app = router(state_with(db.pool.clone(), Arc::new(DisabledMailer), false));
    let first = register(&app, "Alice", "alice@example.test").await;
    // same e-mail, different case and a new username → identical answer, no second account
    let dup = register(&app, "Mallory", "ALICE@example.TEST").await;
    assert_eq!((dup.status, &dup.json), (first.status, &first.json));
    let n: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM users WHERE lower(email) = 'alice@example.test'")
            .fetch_one(&db.pool)
            .await
            .unwrap();
    assert_eq!(n, 1);
    let m: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users WHERE lower(username) = 'mallory'")
        .fetch_one(&db.pool)
        .await
        .unwrap();
    assert_eq!(m, 0);
    // usernames are public → clear error, case-insensitive
    let taken = register(&app, "aLiCe", "other@example.test").await;
    assert_eq!(taken.status, StatusCode::CONFLICT);
    assert_eq!(taken.json["error"], "username_taken");
    db.teardown().await;
}

#[tokio::test]
async fn registration_validates_everything_server_side() {
    let Some(db) = TestDb::new().await else {
        return;
    };
    let app = router(state_with(db.pool.clone(), Arc::new(DisabledMailer), false));
    let bad = |u: &str, e: &str, p: &str| reg(u, e, p);

    let cases: Vec<(serde_json::Value, &str, &str)> = vec![
        (
            bad("alice", "a@example.test", "short"),
            "password",
            "too_short",
        ),
        (
            bad("alice", "a@example.test", &"x1".repeat(100)),
            "password",
            "too_long",
        ),
        (
            bad("alice", "a@example.test", "Password1234"),
            "password",
            "too_common",
        ),
        (
            bad("alice", "a@example.test", "alice-loves-you"),
            "password",
            "contains_personal_data",
        ),
        (bad("ab", "a@example.test", PW), "username", "too_short"),
        (
            bad("al ice", "a@example.test", PW),
            "username",
            "invalid_characters",
        ),
        (bad("admin", "a@example.test", PW), "username", "reserved"),
        (bad("alice", "not-an-email", PW), "email", "invalid"),
        (bad("alice", "x@wallet.local", PW), "email", "not_allowed"),
    ];
    for (body, field, code) in cases {
        let r = send(&app, "POST", "/auth/register", None, Some(body.clone())).await;
        assert_eq!(r.status, StatusCode::BAD_REQUEST, "{body}");
        assert_eq!(r.json["fields"][field], code, "{body} → {}", r.json);
    }
    // confirmation mismatch + missing terms
    let mut b = reg("alice", "a@example.test", PW);
    b["passwordConfirm"] = "different password!".into();
    b["acceptTerms"] = false.into();
    let r = send(&app, "POST", "/auth/register", None, Some(b)).await;
    assert_eq!(r.json["fields"]["passwordConfirm"], "mismatch");
    assert_eq!(r.json["fields"]["acceptTerms"], "required");
    // unknown fields are refused (no smuggling extra attributes)
    let mut b = reg("alice", "a@example.test", PW);
    b["isAdmin"] = true.into();
    assert!(send(&app, "POST", "/auth/register", None, Some(b))
        .await
        .status
        .is_client_error());
    let n: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users")
        .fetch_one(&db.pool)
        .await
        .unwrap();
    assert_eq!(n, 0, "nothing was created by invalid requests");
    db.teardown().await;
}

#[tokio::test]
async fn honeypot_and_per_ip_rate_limit_stop_mass_registration() {
    let Some(db) = TestDb::new().await else {
        return;
    };
    let app = router(state_with(db.pool.clone(), Arc::new(DisabledMailer), false));
    let mut bot = reg("botuser", "bot@example.test", PW);
    bot["website"] = "http://spam.example".into();
    let r = send(&app, "POST", "/auth/register", None, Some(bot)).await;
    assert_eq!(r.status, StatusCode::CREATED, "bots see a success…");
    let n: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users")
        .fetch_one(&db.pool)
        .await
        .unwrap();
    assert_eq!(n, 0, "…but nothing is created");

    // honeypot hits do not consume the budget; 5 real registrations are allowed per hour and IP
    for i in 0..5 {
        assert_eq!(
            register(&app, &format!("User{i}x"), &format!("u{i}@example.test"))
                .await
                .status,
            StatusCode::CREATED
        );
    }
    let blocked = register(&app, "User9x", "u9@example.test").await;
    assert_eq!(blocked.status, StatusCode::TOO_MANY_REQUESTS);
    assert!(blocked.headers.contains_key(header::RETRY_AFTER));
    db.teardown().await;
}

// ── Login / sessions ─────────────────────────────────────────────────────────

#[tokio::test]
async fn login_sets_a_hardened_cookie_and_me_works_without_wallet_or_faceit() {
    let Some(db) = TestDb::new().await else {
        return;
    };
    let app = router(state_with(db.pool.clone(), Arc::new(DisabledMailer), false));
    register(&app, "Alice", "alice@example.test").await;

    let r = login(&app, "Alice@Example.test", PW).await;
    assert_eq!(r.status, StatusCode::OK);
    assert_eq!(r.json["username"], "Alice");
    let cookie = r
        .headers
        .get(header::SET_COOKIE)
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();
    assert!(
        cookie.contains("HttpOnly") && cookie.contains("Path=/") && cookie.contains("SameSite="),
        "{cookie}"
    );
    assert!(!cookie.contains("Domain="), "host-only cookie");
    assert!(
        !cookie.contains("Max-Age"),
        "no remember-me → browser-session cookie"
    );
    // the token never appears in the body
    let token = r.session_cookie().unwrap();
    assert!(!r.json.to_string().contains(&token));

    let me = send(&app, "GET", "/auth/me", Some(&token), None).await;
    assert_eq!(me.status, StatusCode::OK);
    assert_eq!(me.json["username"], "Alice");
    assert_eq!(me.json["email"], "alice@example.test");
    assert_eq!(me.json["email_verified"], false);
    assert_eq!(me.json["faceit_connected"], false);
    assert!(me.json["kaspa_address"].is_null(), "no wallet needed");
    assert_eq!(me.json["newsletter_subscribed"], false);
    assert!(me.json.to_string().find("password_hash").is_none());

    // remember-me → persistent cookie
    let r = send(
        &app,
        "POST",
        "/auth/login",
        None,
        Some(serde_json::json!({"email":"alice@example.test","password":PW,"remember":true})),
    )
    .await;
    assert!(r
        .headers
        .get(header::SET_COOKIE)
        .unwrap()
        .to_str()
        .unwrap()
        .contains("Max-Age="));
    db.teardown().await;
}

#[tokio::test]
async fn failed_logins_do_not_reveal_whether_the_account_exists() {
    let Some(db) = TestDb::new().await else {
        return;
    };
    let app = router(state_with(db.pool.clone(), Arc::new(DisabledMailer), false));
    register(&app, "Alice", "alice@example.test").await;
    let wrong_pw = login(&app, "alice@example.test", "totally wrong password").await;
    let unknown = login(&app, "nobody@example.test", "totally wrong password").await;
    let garbage = login(&app, "not an email", "x").await;
    for r in [&wrong_pw, &unknown, &garbage] {
        assert_eq!(r.status, StatusCode::UNAUTHORIZED);
    }
    assert_eq!(wrong_pw.json, unknown.json);
    assert_eq!(wrong_pw.json, garbage.json);
    assert_eq!(
        wrong_pw.json["message"],
        "E-Mail-Adresse oder Passwort ist nicht korrekt."
    );
    assert!(wrong_pw.session_cookie().is_none());
    db.teardown().await;
}

#[tokio::test]
async fn repeated_failures_are_throttled_temporarily_not_locked_out_forever() {
    let Some(db) = TestDb::new().await else {
        return;
    };
    let state = state_with(db.pool.clone(), Arc::new(DisabledMailer), false);
    let throttle = state.account.throttle.clone();
    let app = router(state);
    register(&app, "Alice", "alice@example.test").await;
    for _ in 0..crate::account::LOGIN_MAX_PER_ACCOUNT {
        assert_eq!(
            login(&app, "alice@example.test", "wrong password here")
                .await
                .status,
            StatusCode::UNAUTHORIZED
        );
    }
    // now even the right password is refused for the window…
    let r = login(&app, "alice@example.test", PW).await;
    assert_eq!(r.status, StatusCode::TOO_MANY_REQUESTS);
    assert!(r.headers.contains_key(header::RETRY_AFTER));
    // …and the block is temporary state, not a database flag
    throttle.clear(&format!(
        "login:acct:{}",
        crate::account::email_key("alice@example.test")
    ));
    assert_eq!(
        login(&app, "alice@example.test", PW).await.status,
        StatusCode::OK
    );
    db.teardown().await;
}

#[tokio::test]
async fn every_login_rotates_the_session_and_logout_revokes_it() {
    let Some(db) = TestDb::new().await else {
        return;
    };
    let app = router(state_with(db.pool.clone(), Arc::new(DisabledMailer), false));
    register(&app, "Alice", "alice@example.test").await;
    let t1 = login(&app, "alice@example.test", PW)
        .await
        .session_cookie()
        .unwrap();
    let t2 = login(&app, "alice@example.test", PW)
        .await
        .session_cookie()
        .unwrap();
    assert_ne!(t1, t2);
    assert_eq!(t1.len(), 43);
    assert_eq!(
        send(&app, "GET", "/auth/me", Some(&t1), None).await.status,
        StatusCode::OK
    );

    let out = send(&app, "POST", "/auth/logout", Some(&t1), None).await;
    assert_eq!(out.status, StatusCode::NO_CONTENT);
    assert!(out
        .headers
        .get(header::SET_COOKIE)
        .unwrap()
        .to_str()
        .unwrap()
        .contains("Max-Age=0"));
    assert_eq!(
        send(&app, "GET", "/auth/me", Some(&t1), None).await.status,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        send(&app, "GET", "/auth/me", Some(&t2), None).await.status,
        StatusCode::OK,
        "other devices stay signed in"
    );
    // log back in: account and data are still there
    let again = login(&app, "alice@example.test", PW).await;
    assert_eq!(again.status, StatusCode::OK);
    db.teardown().await;
}

#[tokio::test]
async fn csrf_layer_protects_the_credential_endpoints() {
    let Some(db) = TestDb::new().await else {
        return;
    };
    let app = router(state_with(db.pool.clone(), Arc::new(DisabledMailer), false)).layer(
        axum::middleware::from_fn(|req, next| async move {
            crate::api::csrf_guard::csrf_protection_layer_multi(
                vec!["https://app.example.test".into()],
                req,
                next,
            )
            .await
        }),
    );
    let post = |origin: Option<&str>| {
        let mut r = Request::builder()
            .method("POST")
            .uri("/auth/login")
            .header(header::CONTENT_TYPE, "application/json");
        if let Some(o) = origin {
            r = r.header("origin", o);
        }
        r.body(Body::from(r#"{"email":"a@example.test","password":"x"}"#))
            .unwrap()
    };
    assert_eq!(
        app.clone()
            .oneshot(post(Some("https://evil.example")))
            .await
            .unwrap()
            .status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        app.clone().oneshot(post(None)).await.unwrap().status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        app.clone()
            .oneshot(post(Some("https://app.example.test")))
            .await
            .unwrap()
            .status(),
        StatusCode::UNAUTHORIZED
    );
    db.teardown().await;
}

#[tokio::test]
async fn wallet_and_faceit_accounts_cannot_use_password_login_and_keep_working() {
    let Some(db) = TestDb::new().await else {
        return;
    };
    let app = router(state_with(db.pool.clone(), Arc::new(DisabledMailer), false));
    // a legacy wallet user exactly as `wallet-verify` creates it
    let id = Uuid::new_v4();
    let hash = crate::account::hash_password("random-wallet-secret-x").unwrap();
    sqlx::query("INSERT INTO users (id, email, password_hash, display_name, kaspa_address) VALUES ($1,$2,$3,'Player_abc123','kaspatest:qlegacy')")
        .bind(id).bind(format!("{id}@wallet.local")).bind(hash).execute(&db.pool).await.unwrap();
    // even knowing the synthetic address and secret, password login is impossible
    assert_eq!(
        login(
            &app,
            &format!("{id}@wallet.local"),
            "random-wallet-secret-x"
        )
        .await
        .status,
        StatusCode::UNAUTHORIZED
    );
    let (kept,): (bool,) = sqlx::query_as("SELECT NOT has_password_login FROM users WHERE id = $1")
        .bind(id)
        .fetch_one(&db.pool)
        .await
        .unwrap();
    assert!(kept);
    // such a session still resolves; /auth/me hides the synthetic address
    let token = crate::account::create_session(&db.pool, id, chrono::Duration::hours(1))
        .await
        .unwrap()
        .0;
    let me = send(&app, "GET", "/auth/me", Some(&token), None).await;
    assert_eq!(me.status, StatusCode::OK);
    assert!(me.json["email"].is_null());
    assert_eq!(me.json["kaspa_address"], "kaspatest:qlegacy");
    db.teardown().await;
}

// ── E-mail verification ──────────────────────────────────────────────────────

#[tokio::test]
async fn email_verification_flow_with_single_use_expiring_tokens() {
    let Some(db) = TestDb::new().await else {
        return;
    };
    let mailer = Arc::new(MemoryMailer::default());
    let app = router(state_with(db.pool.clone(), mailer.clone(), true));

    let r = register(&app, "Alice", "alice@example.test").await;
    assert_eq!(r.json["emailVerificationRequired"], true);
    let mails = mailer.take();
    assert_eq!(mails.len(), 1);
    assert_eq!(mails[0].to, "alice@example.test");
    assert!(mails[0]
        .text
        .contains("https://app.example.test/verify-email?token="));
    let token = token_from_link(&mails[0].text);

    // only the hash is stored
    let stored: String = sqlx::query_scalar("SELECT token_hash FROM email_verification_tokens")
        .fetch_one(&db.pool)
        .await
        .unwrap();
    assert_ne!(stored, token);
    assert_eq!(stored, crate::account::hash_token(&token));

    // not usable before verification
    let blocked = login(&app, "alice@example.test", PW).await;
    assert_eq!(blocked.status, StatusCode::FORBIDDEN);
    assert_eq!(blocked.json["error"], "email_not_verified");
    assert!(blocked.session_cookie().is_none());
    // a wrong password still gets the generic error (no state leak without the password)
    assert_eq!(
        login(&app, "alice@example.test", "wrong wrong wrong")
            .await
            .json["error"],
        "invalid_credentials"
    );

    // verify, once
    let ok = send(
        &app,
        "POST",
        "/auth/verify-email",
        None,
        Some(serde_json::json!({"token": token})),
    )
    .await;
    assert_eq!(
        (ok.status, ok.json["verified"].clone()),
        (StatusCode::OK, true.into())
    );
    let replay = send(
        &app,
        "POST",
        "/auth/verify-email",
        None,
        Some(serde_json::json!({"token": token})),
    )
    .await;
    assert_eq!(replay.status, StatusCode::BAD_REQUEST);
    let session = login(&app, "alice@example.test", PW)
        .await
        .session_cookie()
        .unwrap();
    let me = send(&app, "GET", "/auth/me", Some(&session), None).await;
    assert_eq!(me.json["email_verified"], true);

    // garbage / oversized tokens are refused
    for t in ["nope", &"a".repeat(500)] {
        assert_eq!(
            send(
                &app,
                "POST",
                "/auth/verify-email",
                None,
                Some(serde_json::json!({"token": t}))
            )
            .await
            .status,
            StatusCode::BAD_REQUEST
        );
    }
    db.teardown().await;
}

#[tokio::test]
async fn expired_verification_token_is_rejected_and_resend_invalidates_the_old_link() {
    let Some(db) = TestDb::new().await else {
        return;
    };
    let mailer = Arc::new(MemoryMailer::default());
    let app = router(state_with(db.pool.clone(), mailer.clone(), true));
    register(&app, "Alice", "alice@example.test").await;
    let t_old = token_from_link(&mailer.take()[0].text);

    // resend: neutral for unknown addresses, real mail for the unverified account
    let unknown = send(
        &app,
        "POST",
        "/auth/resend-verification",
        None,
        Some(serde_json::json!({"email":"nobody@example.test"})),
    )
    .await;
    let known = send(
        &app,
        "POST",
        "/auth/resend-verification",
        None,
        Some(serde_json::json!({"email":"alice@example.test"})),
    )
    .await;
    assert_eq!((unknown.status, &unknown.json), (known.status, &known.json));
    assert_eq!(unknown.status, StatusCode::ACCEPTED);
    let mails = mailer.take();
    assert_eq!(
        mails.len(),
        1,
        "only the existing unverified account gets mail"
    );
    let t_new = token_from_link(&mails[0].text);
    assert_ne!(t_old, t_new);
    assert_eq!(
        send(
            &app,
            "POST",
            "/auth/verify-email",
            None,
            Some(serde_json::json!({"token": t_old}))
        )
        .await
        .status,
        StatusCode::BAD_REQUEST,
        "old link is dead"
    );

    sqlx::query("UPDATE email_verification_tokens SET expires_at = NOW() - INTERVAL '1 second'")
        .execute(&db.pool)
        .await
        .unwrap();
    assert_eq!(
        send(
            &app,
            "POST",
            "/auth/verify-email",
            None,
            Some(serde_json::json!({"token": t_new}))
        )
        .await
        .status,
        StatusCode::BAD_REQUEST,
        "expired"
    );
    db.teardown().await;
}

#[tokio::test]
async fn without_smtp_the_explicit_fallback_lets_people_in_and_never_fakes_reset_mails() {
    let Some(db) = TestDb::new().await else {
        return;
    };
    let app = router(state_with(db.pool.clone(), Arc::new(DisabledMailer), false));
    let r = register(&app, "Alice", "alice@example.test").await;
    assert_eq!(r.json["emailVerificationRequired"], false);
    assert_eq!(
        login(&app, "alice@example.test", PW).await.status,
        StatusCode::OK
    );
    let tokens: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM email_verification_tokens")
        .fetch_one(&db.pool)
        .await
        .unwrap();
    assert_eq!(
        tokens, 0,
        "no token is generated when nothing can be delivered"
    );
    // honest failure instead of a pretend-success
    let f = send(
        &app,
        "POST",
        "/auth/forgot-password",
        None,
        Some(serde_json::json!({"email":"alice@example.test"})),
    )
    .await;
    assert_eq!(f.status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(f.json["error"], "email_delivery_unavailable");
    let resets: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM password_reset_tokens")
        .fetch_one(&db.pool)
        .await
        .unwrap();
    assert_eq!(resets, 0);
    db.teardown().await;
}

// ── Password reset / change / newsletter ─────────────────────────────────────

#[tokio::test]
async fn password_reset_is_single_use_expiring_and_revokes_all_sessions() {
    let Some(db) = TestDb::new().await else {
        return;
    };
    let mailer = Arc::new(MemoryMailer::default());
    let app = router(state_with(db.pool.clone(), mailer.clone(), false));
    register(&app, "Alice", "alice@example.test").await;
    mailer.take();
    let s1 = login(&app, "alice@example.test", PW)
        .await
        .session_cookie()
        .unwrap();
    let s2 = login(&app, "alice@example.test", PW)
        .await
        .session_cookie()
        .unwrap();

    // unknown address: same answer, no mail
    let unknown = send(
        &app,
        "POST",
        "/auth/forgot-password",
        None,
        Some(serde_json::json!({"email":"ghost@example.test"})),
    )
    .await;
    assert_eq!(unknown.status, StatusCode::ACCEPTED);
    assert_eq!(mailer.len(), 0);
    let known = send(
        &app,
        "POST",
        "/auth/forgot-password",
        None,
        Some(serde_json::json!({"email":"alice@example.test"})),
    )
    .await;
    assert_eq!((known.status, &known.json), (unknown.status, &unknown.json));
    let mails = mailer.take();
    assert_eq!(mails.len(), 1);
    assert!(mails[0]
        .text
        .contains("https://app.example.test/reset-password?token="));
    let token = token_from_link(&mails[0].text);
    let stored: String = sqlx::query_scalar("SELECT token_hash FROM password_reset_tokens")
        .fetch_one(&db.pool)
        .await
        .unwrap();
    assert_eq!(stored, crate::account::hash_token(&token));

    // weak new password is refused and does NOT burn the token
    let weak = send(
        &app,
        "POST",
        "/auth/reset-password",
        None,
        Some(serde_json::json!({"token": token, "newPassword": "short"})),
    )
    .await;
    assert_eq!(weak.status, StatusCode::BAD_REQUEST);

    let new_pw = "a brand new passphrase 42";
    let ok = send(
        &app,
        "POST",
        "/auth/reset-password",
        None,
        Some(serde_json::json!({"token": token, "newPassword": new_pw})),
    )
    .await;
    assert_eq!(ok.status, StatusCode::OK);
    // replay fails, old password fails, new works, old sessions are dead
    assert_eq!(
        send(
            &app,
            "POST",
            "/auth/reset-password",
            None,
            Some(serde_json::json!({"token": token, "newPassword": "yet another passphrase 7"}))
        )
        .await
        .status,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        login(&app, "alice@example.test", PW).await.status,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        login(&app, "alice@example.test", new_pw).await.status,
        StatusCode::OK
    );
    for s in [&s1, &s2] {
        assert_eq!(
            send(&app, "GET", "/auth/me", Some(s), None).await.status,
            StatusCode::UNAUTHORIZED
        );
    }

    // expiry
    send(
        &app,
        "POST",
        "/auth/forgot-password",
        None,
        Some(serde_json::json!({"email":"alice@example.test"})),
    )
    .await;
    let t2 = token_from_link(&mailer.take()[0].text);
    sqlx::query("UPDATE password_reset_tokens SET expires_at = NOW() - INTERVAL '1 second' WHERE used_at IS NULL").execute(&db.pool).await.unwrap();
    assert_eq!(
        send(
            &app,
            "POST",
            "/auth/reset-password",
            None,
            Some(serde_json::json!({"token": t2, "newPassword": "and one more passphrase 9"}))
        )
        .await
        .status,
        StatusCode::BAD_REQUEST
    );
    db.teardown().await;
}

#[tokio::test]
async fn change_password_requires_the_current_one_and_keeps_only_this_session() {
    let Some(db) = TestDb::new().await else {
        return;
    };
    let app = router(state_with(db.pool.clone(), Arc::new(DisabledMailer), false));
    register(&app, "Alice", "alice@example.test").await;
    let here = login(&app, "alice@example.test", PW)
        .await
        .session_cookie()
        .unwrap();
    let other = login(&app, "alice@example.test", PW)
        .await
        .session_cookie()
        .unwrap();

    let wrong = send(&app, "POST", "/auth/change-password", Some(&here), Some(serde_json::json!({"currentPassword":"nope nope nope","newPassword":"a brand new passphrase 42"}))).await;
    assert_eq!(wrong.status, StatusCode::FORBIDDEN);
    let unauth = send(
        &app,
        "POST",
        "/auth/change-password",
        None,
        Some(serde_json::json!({"currentPassword":PW,"newPassword":"a brand new passphrase 42"})),
    )
    .await;
    assert_eq!(unauth.status, StatusCode::UNAUTHORIZED);
    let ok = send(
        &app,
        "POST",
        "/auth/change-password",
        Some(&here),
        Some(serde_json::json!({"currentPassword":PW,"newPassword":"a brand new passphrase 42"})),
    )
    .await;
    assert_eq!(ok.status, StatusCode::OK);
    assert_eq!(
        send(&app, "GET", "/auth/me", Some(&here), None)
            .await
            .status,
        StatusCode::OK
    );
    assert_eq!(
        send(&app, "GET", "/auth/me", Some(&other), None)
            .await
            .status,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        login(&app, "alice@example.test", "a brand new passphrase 42")
            .await
            .status,
        StatusCode::OK
    );
    db.teardown().await;
}

#[tokio::test]
async fn newsletter_consent_is_opt_in_recorded_and_revocable() {
    let Some(db) = TestDb::new().await else {
        return;
    };
    let app = router(state_with(db.pool.clone(), Arc::new(DisabledMailer), false));
    // opt-in at registration records version + source + time
    let mut b = reg("Alice", "alice@example.test", PW);
    b["newsletter"] = true.into();
    assert_eq!(
        send(&app, "POST", "/auth/register", None, Some(b))
            .await
            .status,
        StatusCode::CREATED
    );
    let row = sqlx::query("SELECT newsletter_consent_at, newsletter_consent_version, newsletter_consent_source, newsletter_doi_confirmed_at FROM users WHERE username = 'Alice'")
        .fetch_one(&db.pool).await.unwrap();
    assert!(row
        .get::<Option<chrono::DateTime<chrono::Utc>>, _>("newsletter_consent_at")
        .is_some());
    assert_eq!(
        row.get::<Option<String>, _>("newsletter_consent_source")
            .as_deref(),
        Some("registration")
    );
    assert_eq!(
        row.get::<Option<String>, _>("newsletter_consent_version")
            .as_deref(),
        Some(crate::account::NEWSLETTER_CONSENT_VERSION)
    );
    assert!(
        row.get::<Option<chrono::DateTime<chrono::Utc>>, _>("newsletter_doi_confirmed_at")
            .is_none(),
        "no double-opt-in → nothing may be sent"
    );

    let t = login(&app, "alice@example.test", PW)
        .await
        .session_cookie()
        .unwrap();
    assert_eq!(
        send(&app, "GET", "/auth/me", Some(&t), None).await.json["newsletter_subscribed"],
        true
    );
    let off = send(
        &app,
        "POST",
        "/auth/newsletter",
        Some(&t),
        Some(serde_json::json!({"subscribe": false})),
    )
    .await;
    assert_eq!(off.status, StatusCode::OK);
    assert_eq!(
        send(&app, "GET", "/auth/me", Some(&t), None).await.json["newsletter_subscribed"],
        false
    );
    send(
        &app,
        "POST",
        "/auth/newsletter",
        Some(&t),
        Some(serde_json::json!({"subscribe": true})),
    )
    .await;
    assert_eq!(
        send(&app, "GET", "/auth/me", Some(&t), None).await.json["newsletter_subscribed"],
        true
    );
    assert_eq!(
        send(
            &app,
            "POST",
            "/auth/newsletter",
            None,
            Some(serde_json::json!({"subscribe": true}))
        )
        .await
        .status,
        StatusCode::UNAUTHORIZED
    );
    db.teardown().await;
}

#[tokio::test]
async fn audit_log_never_contains_secrets() {
    let Some(db) = TestDb::new().await else {
        return;
    };
    let app = router(state_with(db.pool.clone(), Arc::new(DisabledMailer), false));
    register(&app, "Alice", "alice@example.test").await;
    login(&app, "alice@example.test", PW).await;
    login(&app, "alice@example.test", "wrong password").await;
    let rows = sqlx::query("SELECT event, ip_hash FROM auth_audit_log")
        .fetch_all(&db.pool)
        .await
        .unwrap();
    assert!(rows.len() >= 3);
    for r in rows {
        let line = format!(
            "{} {:?}",
            r.get::<String, _>("event"),
            r.get::<Option<String>, _>("ip_hash")
        );
        assert!(
            !line.contains("alice") && !line.contains("example.test") && !line.contains(PW),
            "{line}"
        );
    }
    db.teardown().await;
}

// ── Migration safety ─────────────────────────────────────────────────────────

#[tokio::test]
async fn new_migrations_leave_existing_users_and_matches_untouched() {
    let Some(db) = TestDb::new().await else {
        return;
    };
    // Re-create the pre-feature situation inside the fully migrated DB: legacy rows that exist
    // before the new columns are filled, then re-run the new migrations (they must be idempotent).
    let wallet = Uuid::new_v4();
    let faceit = Uuid::new_v4();
    for (id, email, name) in [
        (wallet, format!("{wallet}@wallet.local"), "Player_w1"),
        (faceit, "Faceit.Player@Example.com".to_string(), "FaceitGuy"),
    ] {
        sqlx::query("INSERT INTO users (id, email, password_hash, display_name) VALUES ($1,$2,'$argon2id$legacy',$3)")
            .bind(id).bind(email).bind(name).execute(&db.pool).await.unwrap();
    }
    sqlx::query("INSERT INTO matches (id, creator_user_id, game_id, wager_sompi, wager_amount_sompi, mode) VALUES ($1,$2,'cs2',100,100,'BO1')")
        .bind(Uuid::new_v4()).bind(faceit).execute(&db.pool).await.unwrap();
    let before: Vec<(Uuid, String, String)> =
        sqlx::query_as("SELECT id, email, display_name FROM users ORDER BY id")
            .fetch_all(&db.pool)
            .await
            .unwrap();

    for sql in [
        include_str!("../../migrations/202610020001_email_identity.sql"),
        include_str!("../../migrations/202610020002_free_play.sql"),
    ] {
        sqlx::raw_sql(sql)
            .execute(&db.pool)
            .await
            .expect("migrations are re-runnable");
    }
    let after: Vec<(Uuid, String, String)> =
        sqlx::query_as("SELECT id, email, display_name FROM users ORDER BY id")
            .fetch_all(&db.pool)
            .await
            .unwrap();
    assert_eq!(before, after);
    let flags: Vec<bool> = sqlx::query_scalar("SELECT has_password_login FROM users")
        .fetch_all(&db.pool)
        .await
        .unwrap();
    assert!(
        flags.iter().all(|f| !f),
        "legacy accounts are not password accounts"
    );
    let mode: String = sqlx::query_scalar("SELECT game_mode FROM matches")
        .fetch_one(&db.pool)
        .await
        .unwrap();
    assert_eq!(mode, "kaspa_testnet");
    // a paid match can never be marked free_play
    let bad = sqlx::query("UPDATE matches SET game_mode = 'free_play'")
        .execute(&db.pool)
        .await;
    assert!(bad.is_err());
    db.teardown().await;
}

// ── Session token hashing (audit F-07) ───────────────────────────────────────

#[tokio::test]
async fn session_tokens_are_stored_hashed_and_the_database_value_is_not_a_credential() {
    let Some(db) = TestDb::new().await else {
        return;
    };
    let app = router(state_with(db.pool.clone(), Arc::new(DisabledMailer), false));
    register(&app, "Hashy", "hashy@example.test").await;
    let token = login(&app, "hashy@example.test", PW)
        .await
        .session_cookie()
        .unwrap();

    let ids: Vec<String> = sqlx::query_scalar("SELECT id FROM sessions")
        .fetch_all(&db.pool)
        .await
        .unwrap();
    assert_eq!(ids.len(), 1);
    assert_ne!(ids[0], token, "the raw token must never be persisted");
    assert_eq!(
        ids[0],
        battle_core::auth::AuthService::hash_session_token(&token)
    );

    // presenting the stored hash as a cookie must NOT authenticate (a DB leak is not a login)
    let leaked = send(&app, "GET", "/auth/me", Some(&ids[0]), None).await;
    assert_eq!(leaked.status, StatusCode::UNAUTHORIZED);
    // the real token still works, and logout removes exactly that row
    assert_eq!(
        send(&app, "GET", "/auth/me", Some(&token), None)
            .await
            .status,
        StatusCode::OK
    );
    send(&app, "POST", "/auth/logout", Some(&token), None).await;
    let left: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM sessions")
        .fetch_one(&db.pool)
        .await
        .unwrap();
    assert_eq!(left, 0);
}

#[tokio::test]
async fn hash_migration_keeps_existing_sessions_valid_and_is_idempotent() {
    let Some(db) = TestDb::new().await else {
        return;
    };
    let app = router(state_with(db.pool.clone(), Arc::new(DisabledMailer), false));
    register(&app, "Legacy", "legacy@example.test").await;
    let uid: Uuid = sqlx::query_scalar("SELECT id FROM users WHERE email = 'legacy@example.test'")
        .fetch_one(&db.pool)
        .await
        .unwrap();

    // a session created before the migration: the raw token is the primary key
    let raw = battle_core::auth::AuthService::generate_session_token();
    sqlx::query(
        "INSERT INTO sessions (id, user_id, expires_at) VALUES ($1,$2, NOW() + INTERVAL '1 day')",
    )
    .bind(&raw)
    .bind(uid)
    .execute(&db.pool)
    .await
    .unwrap();
    assert_eq!(
        send(&app, "GET", "/auth/me", Some(&raw), None).await.status,
        StatusCode::UNAUTHORIZED,
        "unmigrated raw rows are not accepted by the hashing code"
    );

    let sql = include_str!("../../migrations/202610070001_hash_session_tokens.sql");
    sqlx::raw_sql(sql).execute(&db.pool).await.unwrap();
    let stored: String = sqlx::query_scalar("SELECT id FROM sessions WHERE user_id = $1")
        .bind(uid)
        .fetch_one(&db.pool)
        .await
        .unwrap();
    assert_eq!(
        stored,
        battle_core::auth::AuthService::hash_session_token(&raw),
        "SQL sha256 must equal the Rust hash"
    );
    assert_eq!(
        send(&app, "GET", "/auth/me", Some(&raw), None).await.status,
        StatusCode::OK,
        "users stay logged in across the migration"
    );

    // re-running must not hash the hash
    sqlx::raw_sql(sql).execute(&db.pool).await.unwrap();
    let again: String = sqlx::query_scalar("SELECT id FROM sessions WHERE user_id = $1")
        .bind(uid)
        .fetch_one(&db.pool)
        .await
        .unwrap();
    assert_eq!(again, stored);
}

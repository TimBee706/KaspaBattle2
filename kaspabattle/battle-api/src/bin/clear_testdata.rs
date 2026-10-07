use sqlx::postgres::PgPoolOptions;

#[tokio::main]
async fn main() {
    // Destructive dev tool (deletes ALL matches and payments). It must never run by accident
    // against a database that happens to be configured in the environment, so it needs an explicit
    // DATABASE_URL (no built-in default) and an explicit confirmation.
    let Ok(db_url) = std::env::var("DATABASE_URL") else {
        eprintln!("DATABASE_URL must be set explicitly; refusing to guess a database.");
        std::process::exit(2);
    };
    if std::env::var("CONFIRM_CLEAR_TESTDATA").as_deref() != Ok("yes") {
        eprintln!(
            "This deletes ALL rows of `payments` and `matches`.\n\
             Re-run with CONFIRM_CLEAR_TESTDATA=yes to proceed."
        );
        std::process::exit(2);
    }

    let pool = PgPoolOptions::new()
        .max_connections(2)
        .connect(&db_url)
        .await
        .expect("DB connection failed");

    println!("Verbunden mit DB");

    let r1 = sqlx::query("DELETE FROM payments")
        .execute(&pool)
        .await
        .expect("DELETE payments failed");
    println!("✅ payments gelöscht: {} Zeilen", r1.rows_affected());

    let r2 = sqlx::query("DELETE FROM matches")
        .execute(&pool)
        .await
        .expect("DELETE matches failed");
    println!("✅ matches gelöscht: {} Zeilen", r2.rows_affected());

    println!("🎉 Datenbank bereinigt — alle Testdaten gelöscht.");
}

use sqlx::postgres::PgPoolOptions;

#[tokio::main]
async fn main() {
    let db_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:postgres@localhost:5432/kaspabattle".to_string());

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

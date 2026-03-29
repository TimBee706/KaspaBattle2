use sqlx::postgres::PgPoolOptions;

#[tokio::main]
async fn main() {
    let db_url = "postgres://postgres:postgres@localhost:5432/kaspabattle";
    let pool = PgPoolOptions::new()
        .max_connections(2)
        .connect(db_url)
        .await
        .expect("Failed to connect");

    sqlx::query("DELETE FROM _sqlx_migrations")
        .execute(&pool)
        .await
        .expect("Failed to clear migrations");

    println!("✅ _sqlx_migrations cleared — run cargo run -p battle-api to re-apply all migrations");
}

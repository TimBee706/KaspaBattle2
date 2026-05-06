use sqlx::postgres::PgPoolOptions;
use sqlx::Row;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let db_url = "postgres://postgres:postgres@localhost:5432/kaspabattle";
    let pool = PgPoolOptions::new()
        .max_connections(1)
        .connect(db_url)
        .await?;

    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM _sqlx_migrations")
        .fetch_one(&pool)
        .await?;

    println!("Applied migrations count: {}", count);

    let rows = sqlx::query("SELECT version, description FROM _sqlx_migrations ORDER BY version")
        .fetch_all(&pool)
        .await?;

    for row in rows {
        let version: i64 = row.get(0);
        let desc: String = row.get(1);
        println!(" - {}: {}", version, desc);
    }

    Ok(())
}

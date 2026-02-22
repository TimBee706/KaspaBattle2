use battle_core::match_state::MatchState;
use battle_core::models::oracle::{OracleJob, OracleJobStatus};
use sqlx::sqlite::SqlitePoolOptions;
use sqlx::{Sqlite, SqlitePool};

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct MatchRow {
    pub match_id: String,
    pub state: String,
    pub state_json: String,
    pub player_a_id: String,
    pub player_a_addr: String,
    pub player_a_name: Option<String>,
    pub player_b_id: Option<String>,
    pub player_b_addr: Option<String>,
    pub player_b_name: Option<String>,
    pub wager_sompi: i64,
    pub escrow_address: String,
    pub game_type: String,
    pub timeout_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    // Step 2 columns (may be NULL if migration 002 hasn't run yet)
    pub escrow_private_seed: Option<String>,
    pub player_a_deposited: Option<i32>,
    pub player_b_deposited: Option<i32>,
    pub winner_id: Option<String>,
    pub winner_address: Option<String>,
    pub payout_tx_hash: Option<String>,
    pub payout_amount_sompi: Option<i64>,
}

#[derive(Debug, Clone, sqlx::FromRow, serde::Serialize)]
pub struct EscrowRow {
    pub challenge_id: String,
    pub escrow_address: String,
    pub derivation_index: i32,
    pub wager_amount_sompi: i64,
    pub status: String,
    pub payout_tx_id: Option<String>,
    pub refund_tx_a: Option<String>,
    pub refund_tx_b: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

/// Lightweight struct for active escrow polling
#[derive(Debug, Clone)]
pub struct ActiveEscrow {
    pub match_id: String,
    pub escrow_address: String,
    pub wager_sompi: i64,
    pub player_a_id: String,
    pub player_a_addr: Option<String>,
    pub player_b_id: Option<String>,
    pub player_b_addr: Option<String>,
    pub player_a_deposited: bool,
    pub player_b_deposited: bool,
    pub state_json: String,
    pub timeout_at: Option<String>,
}

pub struct Database {
    pub(crate) pool: SqlitePool,
}

impl Database {
    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    pub async fn new(database_url: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect(database_url)
            .await?;

        // Run migration 001
        let migration_sql = include_str!("../../migrations/001_create_matches.sql");
        for statement in migration_sql.split(';') {
            let trimmed = statement.trim();
            if !trimmed.is_empty() {
                sqlx::query(trimmed).execute(&pool).await?;
            }
        }

        let db = Database { pool };

        // Run migration 002
        db.run_migration_002().await?;

        // Run migration 003
        db.run_migration_003().await?;

        // Run migration 004 (Users)
        db.run_migration_004().await?;

        // Run migration 005 (Oracle)
        db.run_migration_005().await?;

        // Run migration 006 (Faceit Match ID)
        db.run_migration_006().await?;

        Ok(db)
    }

    /// Run migration 002: deposits table + ALTER TABLE columns.
    /// Each ALTER TABLE is executed individually because SQLite
    /// only supports one ALTER TABLE per statement.
    /// Uses "IF NOT EXISTS" / checks to be idempotent.
    async fn run_migration_002(&self) -> Result<(), Box<dyn std::error::Error>> {
        // Create deposits table
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS deposits (
                id              INTEGER PRIMARY KEY AUTOINCREMENT,
                match_id        TEXT NOT NULL REFERENCES matches(match_id),
                player_id       TEXT NOT NULL,
                tx_hash         TEXT,
                amount_sompi    INTEGER NOT NULL,
                detected_at     TEXT NOT NULL DEFAULT (datetime('now')),
                confirmed       INTEGER NOT NULL DEFAULT 0
            )",
        )
        .execute(&self.pool)
        .await?;

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_deposits_match ON deposits(match_id)")
            .execute(&self.pool)
            .await?;

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_deposits_player ON deposits(player_id)")
            .execute(&self.pool)
            .await?;

        // ALTER TABLE columns – each one individually, ignoring errors if column already exists
        let alter_statements = vec![
            "ALTER TABLE matches ADD COLUMN escrow_private_seed TEXT",
            "ALTER TABLE matches ADD COLUMN player_a_deposited INTEGER DEFAULT 0",
            "ALTER TABLE matches ADD COLUMN player_b_deposited INTEGER DEFAULT 0",
            "ALTER TABLE matches ADD COLUMN winner_id TEXT",
            "ALTER TABLE matches ADD COLUMN winner_address TEXT",
            "ALTER TABLE matches ADD COLUMN payout_tx_hash TEXT",
            "ALTER TABLE matches ADD COLUMN payout_amount_sompi INTEGER",
        ];

        for stmt in alter_statements {
            // Ignore "duplicate column" errors (column already exists)
            match sqlx::query(stmt).execute(&self.pool).await {
                Ok(_) => {}
                Err(e) => {
                    let err_str = e.to_string();
                    if err_str.contains("duplicate column") || err_str.contains("already exists") {
                        // Column already exists, skip
                    } else {
                        log::warn!("Migration 002 ALTER TABLE warning: {}", err_str);
                    }
                }
            }
        }

        log::info!("✅ Migration 002 (deposits) completed");
        Ok(())
    }

    /// Run migration 003: escrows table + platform_config.
    async fn run_migration_003(&self) -> Result<(), Box<dyn std::error::Error>> {
        let migration_sql = include_str!("../../migrations/003_escrow_transactions.sql");
        for statement in migration_sql.split(';') {
            let trimmed = statement.trim();
            if !trimmed.is_empty() {
                sqlx::query(trimmed).execute(&self.pool).await?;
            }
        }

        log::info!("✅ Migration 003 (escrows) completed");
        Ok(())
    }

    /// Run migration 004: users table.
    async fn run_migration_004(&self) -> Result<(), Box<dyn std::error::Error>> {
        let migration_sql = include_str!("../../battle-core/migrations/004_users.sql");
        for statement in migration_sql.split(';') {
            let trimmed = statement.trim();
            if !trimmed.is_empty() {
                sqlx::query(trimmed).execute(&self.pool).await?;
            }
        }
        log::info!("✅ Migration 004 (users) completed");
        Ok(())
    }

    /// Run migration 005: oracle_jobs table.
    async fn run_migration_005(&self) -> Result<(), Box<dyn std::error::Error>> {
        let migration_sql = include_str!("../../battle-core/migrations/005_oracle.sql");
        for statement in migration_sql.split(';') {
            let trimmed = statement.trim();
            if !trimmed.is_empty() {
                sqlx::query(trimmed).execute(&self.pool).await?;
            }
        }
        log::info!("✅ Migration 005 (oracle) completed");
        Ok(())
    }

    /// Run migration 006: add faceit_match_id to matches.
    async fn run_migration_006(&self) -> Result<(), Box<dyn std::error::Error>> {
        let migration_sql = include_str!("../../migrations/006_faceit_match_id.sql");
        for statement in migration_sql.split(';') {
            let trimmed = statement.trim();
            if !trimmed.is_empty() {
                // Ignore "duplicate column" errors
                match sqlx::query(trimmed).execute(&self.pool).await {
                    Ok(_) => {}
                    Err(e) => {
                        let err_str = e.to_string();
                        if err_str.contains("duplicate column") || err_str.contains("already exists") {
                            // skip
                        } else {
                            return Err(Box::new(e));
                        }
                    }
                }
            }
        }
        log::info!("✅ Migration 006 (faceit_match_id) completed");
        Ok(())
    }

    pub async fn create_match(
        &self,
        match_id: &str,
        player_a_id: &str,
        player_a_addr: &str,
        player_a_name: Option<&str>,
        wager_sompi: i64,
        escrow_address: &str,
        game_type: &str,
        state: &MatchState,
        timeout_at: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let state_name = state.state_name();
        let state_json =
            serde_json::to_string(state).map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;

        sqlx::query(
            "INSERT INTO matches (match_id, state, state_json, player_a_id, player_a_addr, player_a_name, wager_sompi, escrow_address, game_type, timeout_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"
        )
        .bind(match_id)
        .bind(state_name)
        .bind(&state_json)
        .bind(player_a_id)
        .bind(player_a_addr)
        .bind(player_a_name)
        .bind(wager_sompi)
        .bind(escrow_address)
        .bind(game_type)
        .bind(timeout_at)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn get_match(
        &self,
        match_id: &str,
    ) -> Result<Option<MatchRow>, Box<dyn std::error::Error>> {
        let row = sqlx::query_as::<Sqlite, MatchRow>("SELECT * FROM matches WHERE match_id = ?")
            .bind(match_id)
            .fetch_optional(&self.pool)
            .await?;

        Ok(row)
    }

    pub async fn update_match_state(
        &self,
        match_id: &str,
        new_state: &MatchState,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let state_name = new_state.state_name();
        let state_json = serde_json::to_string(new_state)
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;

        sqlx::query(
            "UPDATE matches SET state = ?, state_json = ?, updated_at = datetime('now') WHERE match_id = ?"
        )
        .bind(state_name)
        .bind(&state_json)
        .bind(match_id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn join_match(
        &self,
        match_id: &str,
        player_b_id: &str,
        player_b_addr: &str,
        player_b_name: Option<&str>,
        new_state: &MatchState,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let state_name = new_state.state_name();
        let state_json = serde_json::to_string(new_state)
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;

        let result = sqlx::query(
            "UPDATE matches SET player_b_id = ?, player_b_addr = ?, player_b_name = ?, state = ?, state_json = ?, updated_at = datetime('now') WHERE match_id = ? AND state = 'WaitingForOpponent'"
        )
        .bind(player_b_id)
        .bind(player_b_addr)
        .bind(player_b_name)
        .bind(state_name)
        .bind(&state_json)
        .bind(match_id)
        .execute(&self.pool)
        .await?;

        if result.rows_affected() == 0 {
            return Err(Box::new(std::io::Error::new(
                std::io::ErrorKind::Other,
                "Match ist nicht mehr im Status WaitingForOpponent oder existiert nicht",
            )));
        }

        Ok(())
    }

    pub async fn get_open_matches(&self) -> Result<Vec<MatchRow>, Box<dyn std::error::Error>> {
        let rows = sqlx::query_as::<_, MatchRow>(
            "SELECT * FROM matches WHERE state = 'WaitingForOpponent' ORDER BY created_at DESC LIMIT 50"
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows)
    }

    // === Step 2: New methods ===

    /// Record a detected deposit in the deposits table
    pub async fn record_deposit(
        &self,
        match_id: &str,
        player_id: &str,
        tx_hash: Option<&str>,
        amount_sompi: i64,
    ) -> Result<(), Box<dyn std::error::Error>> {
        sqlx::query(
            "INSERT INTO deposits (match_id, player_id, tx_hash, amount_sompi)
             VALUES (?, ?, ?, ?)",
        )
        .bind(match_id)
        .bind(player_id)
        .bind(tx_hash)
        .bind(amount_sompi)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Update the deposit status flags on the matches table
    pub async fn update_deposit_status(
        &self,
        match_id: &str,
        player_a_deposited: bool,
        player_b_deposited: bool,
    ) -> Result<(), Box<dyn std::error::Error>> {
        sqlx::query(
            "UPDATE matches SET player_a_deposited = ?, player_b_deposited = ?, updated_at = datetime('now') WHERE match_id = ?"
        )
        .bind(player_a_deposited as i32)
        .bind(player_b_deposited as i32)
        .bind(match_id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Get all matches in WaitingForDeposits state with their escrow addresses.
    /// Used by the blockchain watcher to poll for deposits.
    pub async fn get_active_escrow_addresses(
        &self,
    ) -> Result<Vec<ActiveEscrow>, Box<dyn std::error::Error>> {
        let rows = sqlx::query_as::<_, MatchRow>(
            "SELECT * FROM matches WHERE state = 'WaitingForDeposits'",
        )
        .fetch_all(&self.pool)
        .await?;

        let active: Vec<ActiveEscrow> = rows
            .into_iter()
            .map(|r| ActiveEscrow {
                match_id: r.match_id,
                escrow_address: r.escrow_address,
                wager_sompi: r.wager_sompi,
                player_a_id: r.player_a_id,
                player_a_addr: Some(r.player_a_addr),
                player_b_id: r.player_b_id,
                player_b_addr: r.player_b_addr,
                player_a_deposited: r.player_a_deposited.unwrap_or(0) != 0,
                player_b_deposited: r.player_b_deposited.unwrap_or(0) != 0,
                state_json: r.state_json,
                timeout_at: r.timeout_at,
            })
            .collect();

        Ok(active)
    }

    /// F-006: Get all matches in WaitingForDeposits state where the deposit
    /// timeout has expired. `now_str` should be an ISO 8601 datetime string
    /// (e.g. from `chrono::Utc::now().naive_utc().to_string()`).
    pub async fn get_timed_out_escrows(
        &self,
        now_str: &str,
    ) -> Result<Vec<ActiveEscrow>, Box<dyn std::error::Error>> {
        let rows = sqlx::query_as::<_, MatchRow>(
            "SELECT * FROM matches WHERE state = 'WaitingForDeposits' AND timeout_at IS NOT NULL AND timeout_at < ?",
        )
        .bind(now_str)
        .fetch_all(&self.pool)
        .await?;

        let active: Vec<ActiveEscrow> = rows
            .into_iter()
            .map(|r| ActiveEscrow {
                match_id: r.match_id,
                escrow_address: r.escrow_address,
                wager_sompi: r.wager_sompi,
                player_a_id: r.player_a_id,
                player_a_addr: Some(r.player_a_addr),
                player_b_id: r.player_b_id,
                player_b_addr: r.player_b_addr,
                player_a_deposited: r.player_a_deposited.unwrap_or(0) != 0,
                player_b_deposited: r.player_b_deposited.unwrap_or(0) != 0,
                state_json: r.state_json,
                timeout_at: r.timeout_at,
            })
            .collect();

        Ok(active)
    }

    /// Store payout information after a match is resolved
    pub async fn update_match_payout(
        &self,
        match_id: &str,
        winner_id: &str,
        winner_address: &str,
        payout_tx_hash: &str,
        payout_amount_sompi: i64,
    ) -> Result<(), Box<dyn std::error::Error>> {
        sqlx::query(
            "UPDATE matches SET winner_id = ?, winner_address = ?, payout_tx_hash = ?, payout_amount_sompi = ?, updated_at = datetime('now') WHERE match_id = ?"
        )
        .bind(winner_id)
        .bind(winner_address)
        .bind(payout_tx_hash)
        .bind(payout_amount_sompi)
        .bind(match_id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    // === Step 3: Escrow methods ===

    pub async fn create_escrow(
        &self,
        challenge_id: &str,
        escrow_address: &str,
        derivation_index: i32,
        wager_amount_sompi: i64,
    ) -> Result<(), Box<dyn std::error::Error>> {
        sqlx::query(
            "INSERT INTO escrows (challenge_id, escrow_address, derivation_index, wager_amount_sompi, status)
             VALUES (?, ?, ?, ?, 'NONE')"
        )
        .bind(challenge_id)
        .bind(escrow_address)
        .bind(derivation_index)
        .bind(wager_amount_sompi)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn get_escrow(
        &self,
        challenge_id: &str,
    ) -> Result<Option<EscrowRow>, Box<dyn std::error::Error>> {
        let row =
            sqlx::query_as::<Sqlite, EscrowRow>("SELECT * FROM escrows WHERE challenge_id = ?")
                .bind(challenge_id)
                .fetch_optional(&self.pool)
                .await?;

        Ok(row)
    }

    pub async fn update_escrow_status(
        &self,
        challenge_id: &str,
        status: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        sqlx::query(
            "UPDATE escrows SET status = ?, updated_at = datetime('now') WHERE challenge_id = ?",
        )
        .bind(status)
        .bind(challenge_id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn update_escrow_refund(
        &self,
        challenge_id: &str,
        refund_tx_a: Option<&str>,
        refund_tx_b: Option<&str>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        sqlx::query(
            "UPDATE escrows SET status = 'REFUNDED', refund_tx_a = ?, refund_tx_b = ?, updated_at = datetime('now') WHERE challenge_id = ?"
        )
        .bind(refund_tx_a)
        .bind(refund_tx_b)
        .bind(challenge_id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn get_platform_address(&self) -> Result<String, Box<dyn std::error::Error>> {
        let row: (String,) =
            sqlx::query_as("SELECT value FROM platform_config WHERE key = 'platform_address'")
                .fetch_one(&self.pool)
                .await?;
        Ok(row.0)
    }

    // === Step 4: Oracle methods ===

    pub async fn create_oracle_job(
        &self,
        job: &OracleJob,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let status = job.status.as_str();

        sqlx::query(
            "INSERT INTO oracle_jobs (job_id, match_id, faceit_match_id, status, reported_winner, error_message, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)"
        )
        .bind(&job.job_id)
        .bind(&job.match_id)
        .bind(&job.faceit_match_id)
        .bind(status)
        .bind(&job.reported_winner)
        .bind(&job.error_message)
        .bind(&job.created_at)
        .bind(&job.updated_at)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn get_oracle_job(
        &self,
        job_id: &str,
    ) -> Result<Option<OracleJob>, Box<dyn std::error::Error>> {
        #[derive(sqlx::FromRow)]
        struct DbOracleJob {
            job_id: String,
            match_id: String,
            faceit_match_id: String,
            status: String,
            reported_winner: Option<String>,
            error_message: Option<String>,
            created_at: String,
            updated_at: String,
            resolved_at: Option<String>,
        }

        let row =
            sqlx::query_as::<Sqlite, DbOracleJob>("SELECT * FROM oracle_jobs WHERE job_id = ?")
                .bind(job_id)
                .fetch_optional(&self.pool)
                .await?;

        if let Some(db_job) = row {
            let status =
                db_job.status.parse::<OracleJobStatus>().unwrap_or(OracleJobStatus::Pending);
            Ok(Some(OracleJob {
                job_id: db_job.job_id,
                match_id: db_job.match_id,
                faceit_match_id: db_job.faceit_match_id,
                status,
                reported_winner: db_job.reported_winner,
                error_message: db_job.error_message,
                created_at: db_job.created_at,
                updated_at: db_job.updated_at,
                resolved_at: db_job.resolved_at,
            }))
        } else {
            Ok(None)
        }
    }
}

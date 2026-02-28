use super::EpisodeTrait;
use crate::models::MatchStatus;
use async_trait::async_trait;
use sqlx::{PgPool, Row};
use uuid::Uuid;

#[allow(dead_code)]
pub struct MatchEpisode {
    pub match_id: Uuid,
    pub db_pool: PgPool,
}

#[async_trait]
impl EpisodeTrait for MatchEpisode {
    type Context = PgPool;
    type Error = Box<dyn std::error::Error + Send + Sync>;

    async fn initialize(ctx: &Self::Context, match_id: Uuid) -> Result<Self, Self::Error> {
        Ok(Self {
            match_id,
            db_pool: ctx.clone(),
        })
    }

    async fn execute(&mut self) -> Result<(), Self::Error> {
        let mut tx = self.db_pool.begin().await?;
        let row = sqlx::query("SELECT status, creator_user_id, opponent_user_id FROM matches WHERE id = $1 FOR UPDATE")
            .bind(self.match_id)
            .fetch_one(&mut *tx).await?;

        let status: MatchStatus = row.try_get("status")?;
        let creator_user_id: Uuid = row.try_get("creator_user_id")?;
        let opponent_user_id: Option<Uuid> = row.try_get("opponent_user_id")?;

        if status == MatchStatus::AwaitingFunding {
            let is_funded_onchain = true;
            if is_funded_onchain {
                if let Some(o_id) = opponent_user_id {
                    let external_id = crate::services::faceit::create_faceit_match(
                        &creator_user_id.to_string(),
                        &o_id.to_string(),
                    )
                    .await?;
                    sqlx::query("UPDATE matches SET status = 'LOCKED', external_match_id = $1 WHERE id = $2")
                        .bind(external_id)
                        .bind(self.match_id)
                        .execute(&mut *tx).await?;
                }
            }
        }
        tx.commit().await?;
        Ok(())
    }

    async fn rollback(&mut self) -> Result<(), Self::Error> {
        sqlx::query("UPDATE matches SET status = 'CANCELLED' WHERE id = $1")
            .bind(self.match_id)
            .execute(&self.db_pool)
            .await?;
        Ok(())
    }

    async fn poll(&mut self) -> Result<bool, Self::Error> {
        let row = sqlx::query("SELECT status FROM matches WHERE id = $1")
            .bind(self.match_id)
            .fetch_one(&self.db_pool)
            .await?;
        let status: MatchStatus = row.try_get("status")?;
        Ok(status == MatchStatus::Resolved || status == MatchStatus::Cancelled)
    }
}

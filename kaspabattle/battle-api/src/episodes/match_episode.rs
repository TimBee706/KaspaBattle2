use super::{DepositCheckResult, EpisodeTrait, PlayerRole};
use crate::models::MatchStatus;
use async_trait::async_trait;
use battle_kaspa::escrow::{DepositState, EscrowService};
use sqlx::{PgPool, Row};
use std::sync::Arc;
use tokio::sync::broadcast::Sender;
use uuid::Uuid;

/// Encapsulates one match's lifecycle as a kdapp-style Episode.
///
/// State transitions driven by this episode:
///   Open → AwaitingFunding (on join)
///   AwaitingFunding → Funded (both deposits on-chain confirmed)
///   Funded → Locked (escrow locked, FACEIT match started)
///   Resolving → Resolved (oracle result received)
///   Resolved → PaidOut (PayoutService executed)
pub struct MatchEpisode {
    pub match_id: Uuid,
    pub db_pool: PgPool,
    pub escrow_service: Option<Arc<EscrowService>>,
    pub tx: Sender<String>,
}

#[async_trait]
impl EpisodeTrait for MatchEpisode {
    type Context = (PgPool, Option<Arc<EscrowService>>, Sender<String>);
    type Error = Box<dyn std::error::Error + Send + Sync>;

    async fn initialize(ctx: &Self::Context, match_id: Uuid) -> Result<Self, Self::Error> {
        Ok(Self {
            match_id,
            db_pool: ctx.0.clone(),
            escrow_service: ctx.1.clone(),
            tx: ctx.2.clone(),
        })
    }

    async fn execute(&mut self) -> Result<(), Self::Error> {
        let mut tx = self.db_pool.begin().await?;

        let row = sqlx::query(
            "SELECT status, creator_user_id, opponent_user_id, \
             escrow_address, wager_amount_sompi, created_at \
             FROM matches WHERE id = $1 FOR UPDATE",
        )
        .bind(self.match_id)
        .fetch_one(&mut *tx)
        .await?;

        let status: MatchStatus = row.try_get("status")?;

        match status {
            // AwaitingFunding: check on-chain deposits
            MatchStatus::AwaitingFunding => {
                let created_at: Option<chrono::DateTime<chrono::Utc>> = row.try_get("created_at")?;
                if let Some(created) = created_at {
                    if chrono::Utc::now() > created + chrono::Duration::minutes(15) {
                        eprintln!(
                            "⏳ Episode {}: AWAITING_FUNDING timeout reached",
                            self.match_id
                        );
                        sqlx::query("UPDATE matches SET status = 'CANCELLED' WHERE id = $1")
                            .bind(self.match_id)
                            .execute(&mut *tx)
                            .await?;

                        if let Some(ref svc) = self.escrow_service {
                            let addr: String = row.try_get("escrow_address")?;
                            let wager: i64 = row.try_get("wager_amount_sompi")?;
                            let _ = svc
                                .refund(
                                    &self.match_id.to_string(),
                                    "",
                                    "",
                                    &addr,
                                    wager as u64,
                                )
                                .await;
                        }
                        tx.commit().await?;
                        let _ = self.poll().await; // just to make sure poll can be called or to cleanly exit
                        return Ok(());
                    }
                }

                if let Some(ref svc) = self.escrow_service {
                    let addr: String = row.try_get("escrow_address")?;
                    let wager: i64 = row.try_get("wager_amount_sompi")?;

                    if !addr.is_empty() {
                        match svc.check_deposits(&addr, wager as u64).await {
                            Ok(deposit_status)
                                if deposit_status.status == DepositState::Complete =>
                            {
                                sqlx::query(
                                    "UPDATE matches SET status = 'FUNDED', \
                                     player_a_deposit_confirmed = true, \
                                     player_b_deposit_confirmed = true \
                                     WHERE id = $1",
                                )
                                .bind(self.match_id)
                                .execute(&mut *tx)
                                .await?;
                                
                                // Fetch updated and broadcast
                                if let Ok(updated) = sqlx::query_as::<_, crate::models::Match>("SELECT * FROM matches WHERE id = $1").bind(self.match_id).fetch_one(&self.db_pool).await {
                                    let mut m = updated;
                                    m.calculate_wager();
                                    let _ = self.tx.send(serde_json::to_string(&m).unwrap_or_default());
                                }

                                eprintln!(
                                    "💰 Episode {}: AWAITING_FUNDING → FUNDED",
                                    self.match_id
                                );
                            }
                            Ok(_) => {
                                eprintln!("⏳ Episode {}: deposits still pending", self.match_id);
                            }
                            Err(e) => {
                                eprintln!(
                                    "⚠️ Episode {}: deposit check error: {}",
                                    self.match_id, e
                                );
                            }
                        }
                    }
                }
            }

            // Funded: lock escrow, start FACEIT match
            MatchStatus::Funded => {
                let creator_id: Uuid = row.try_get("creator_user_id")?;
                let opponent_id: Option<Uuid> = row.try_get("opponent_user_id")?;

                if let Some(opp_id) = opponent_id {
                    // Create FACEIT match (gracefully handles missing FACEIT integration)
                    let external_id = crate::services::faceit::create_faceit_match(
                        &creator_id.to_string(),
                        &opp_id.to_string(),
                    )
                    .await
                    .unwrap_or_else(|_| format!("local-{}", self.match_id));

                    sqlx::query(
                        "UPDATE matches SET status = 'LOCKED', \
                         external_match_id = $1 WHERE id = $2",
                    )
                    .bind(&external_id)
                    .bind(self.match_id)
                    .execute(&mut *tx)
                    .await?;

                    // Fetch updated and broadcast
                    if let Ok(updated) = sqlx::query_as::<_, crate::models::Match>("SELECT * FROM matches WHERE id = $1").bind(self.match_id).fetch_one(&self.db_pool).await {
                        let mut m = updated;
                        m.calculate_wager();
                        let _ = self.tx.send(serde_json::to_string(&m).unwrap_or_default());
                    }

                    eprintln!(
                        "🔒 Episode {}: FUNDED → LOCKED (faceit_id: {})",
                        self.match_id, external_id
                    );
                }
            }

            _ => {
                // Terminal or not-yet-active states: no-op
            }
        }

        tx.commit().await?;
        Ok(())
    }

    async fn rollback(&mut self) -> Result<(), Self::Error> {
        sqlx::query(
            "UPDATE matches SET status = 'CANCELLED' WHERE id = $1 \
             AND status IN ('OPEN', 'AWAITING_FUNDING', 'FUNDED')",
        )
        .bind(self.match_id)
        .execute(&self.db_pool)
        .await?;

        // Fetch updated and broadcast
        if let Ok(updated) = sqlx::query_as::<_, crate::models::Match>("SELECT * FROM matches WHERE id = $1").bind(self.match_id).fetch_one(&self.db_pool).await {
            let mut m = updated;
            m.calculate_wager();
            let _ = self.tx.send(serde_json::to_string(&m).unwrap_or_default());
        }

        eprintln!("❌ Episode {}: rolled back → CANCELLED", self.match_id);
        // TODO: trigger EscrowService.refund() if deposits were received
        Ok(())
    }

    async fn poll(&mut self) -> Result<bool, Self::Error> {
        let row = sqlx::query("SELECT status FROM matches WHERE id = $1")
            .bind(self.match_id)
            .fetch_one(&self.db_pool)
            .await?;
        let status: MatchStatus = row.try_get("status")?;
        Ok(matches!(
            status,
            MatchStatus::Resolved | MatchStatus::PaidOut | MatchStatus::Cancelled
        ))
    }

    // ── v0.2: record_deposit ──────────────────────────────────────────────

    async fn record_deposit(
        &mut self,
        player: PlayerRole,
        tx_hash: &str,
    ) -> Result<(), Self::Error> {
        let col = match player {
            PlayerRole::A => "player_a_deposit_tx_hash",
            PlayerRole::B => "player_b_deposit_tx_hash",
        };
        sqlx::query(&format!("UPDATE matches SET {} = $1 WHERE id = $2", col))
            .bind(tx_hash)
            .bind(self.match_id)
            .execute(&self.db_pool)
            .await?;
        Ok(())
    }

    // ── v0.2: confirm_deposits ────────────────────────────────────────────

    async fn confirm_deposits(&mut self) -> Result<DepositCheckResult, Self::Error> {
        let row =
            sqlx::query("SELECT escrow_address, wager_amount_sompi FROM matches WHERE id = $1")
                .bind(self.match_id)
                .fetch_one(&self.db_pool)
                .await?;

        let addr: String = row.try_get("escrow_address")?;
        let wager: i64 = row.try_get("wager_amount_sompi")?;

        if let Some(ref svc) = self.escrow_service {
            let status = svc.check_deposits(&addr, wager as u64).await?;
            match status.status {
                DepositState::Complete => {
                    sqlx::query(
                        "UPDATE matches SET status = 'FUNDED', \
                         player_a_deposit_confirmed = true, \
                         player_b_deposit_confirmed = true \
                         WHERE id = $1",
                    )
                    .bind(self.match_id)
                    .execute(&self.db_pool)
                    .await?;
                    Ok(DepositCheckResult::Complete)
                }
                DepositState::Partial => Ok(DepositCheckResult::Partial {
                    balance_sompi: status.deposited_sompi,
                }),
                DepositState::None => Ok(DepositCheckResult::None),
            }
        } else {
            Ok(DepositCheckResult::None)
        }
    }

    // ── v0.2: submit_faceid ───────────────────────────────────────────────

    async fn submit_faceid(&mut self, player: PlayerRole, hash: &str) -> Result<(), Self::Error> {
        let col = match player {
            PlayerRole::A => "player_a_faceid_hash",
            PlayerRole::B => "player_b_faceid_hash",
        };
        sqlx::query(&format!("UPDATE matches SET {} = $1 WHERE id = $2", col))
            .bind(hash)
            .bind(self.match_id)
            .execute(&self.db_pool)
            .await?;
        Ok(())
    }
}

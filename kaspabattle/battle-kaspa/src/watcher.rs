/// Blockchain watcher — polls escrow addresses for deposits.
///
/// Uses `Arc<dyn KaspaRpc>` for balance checking. The watcher is invoked
/// by the background task in `battle-api/src/watcher_task.rs`.
use std::sync::Arc;
use std::time::Duration;

use serde::Serialize;

use crate::rpc::{KaspaError, KaspaRpc};

/// Balance and UTXO summary for an escrow address.
#[derive(Debug, Clone, Serialize)]
pub struct EscrowStatus {
    pub escrow_address: String,
    pub total_balance: u64,
    pub utxo_count: u32,
    pub total_balance_kas: f64,
}

/// Result of evaluating whether each player has deposited.
#[derive(Debug, Clone, Serialize)]
pub struct DepositEvaluation {
    pub player_a_deposited: bool,
    pub player_b_deposited: bool,
    pub both_deposited: bool,
    pub total_balance: u64,
    pub required_per_player: u64,
}

/// Watches escrow addresses for incoming deposits.
pub struct BlockchainWatcher {
    kaspa: Arc<dyn KaspaRpc>,
    pub poll_interval: Duration,
}

impl BlockchainWatcher {
    /// Create a new watcher.
    pub fn new(kaspa: Arc<dyn KaspaRpc>, poll_interval: Duration) -> Self {
        Self {
            kaspa,
            poll_interval,
        }
    }

    /// Check the balance of an escrow address.
    pub async fn check_escrow_balance(
        &self,
        escrow_address: &str,
    ) -> Result<EscrowStatus, KaspaError> {
        let utxos = self.kaspa.get_utxos(escrow_address).await?;
        let total_balance: u64 = utxos.iter().map(|u| u.amount).sum();

        Ok(EscrowStatus {
            escrow_address: escrow_address.to_string(),
            total_balance,
            utxo_count: utxos.len() as u32,
            total_balance_kas: total_balance as f64 / 100_000.0,
        })
    }

    /// Evaluate whether each player has deposited enough.
    ///
    /// Logic:
    /// - Player A deposited if balance >= 1x wager_per_player
    /// - Player B deposited if balance >= 2x wager_per_player
    pub fn evaluate_deposits(
        &self,
        escrow_status: &EscrowStatus,
        wager_per_player: u64,
    ) -> DepositEvaluation {
        let one_player = wager_per_player;
        let two_players = wager_per_player * 2;

        let player_a_deposited = escrow_status.total_balance >= one_player;
        let player_b_deposited = escrow_status.total_balance >= two_players;

        DepositEvaluation {
            player_a_deposited,
            player_b_deposited,
            both_deposited: player_a_deposited && player_b_deposited,
            total_balance: escrow_status.total_balance,
            required_per_player: wager_per_player,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mock::MockKaspaClient;
    use crate::rpc::UtxoInfo;

    fn make_watcher() -> (BlockchainWatcher, Arc<MockKaspaClient>) {
        let mock = Arc::new(MockKaspaClient::new());
        let watcher =
            BlockchainWatcher::new(mock.clone() as Arc<dyn KaspaRpc>, Duration::from_secs(3));
        (watcher, mock)
    }

    #[tokio::test]
    async fn test_check_balance_empty() {
        let (watcher, _mock) = make_watcher();
        let status = watcher
            .check_escrow_balance("kaspatest:qtest")
            .await
            .unwrap();
        assert_eq!(status.total_balance, 0);
        assert_eq!(status.utxo_count, 0);
    }

    #[tokio::test]
    async fn test_check_balance_with_utxos() {
        let (watcher, mock) = make_watcher();
        mock.add_utxo(
            "kaspatest:qtest",
            UtxoInfo {
                tx_id: "tx1".to_string(),
                output_index: 0,
                amount: 5_000_000,
                amount_kas: 50.0,
                is_coinbase: false,
                block_daa_score: 100,
                script_public_key: None,
            },
        );

        let status = watcher
            .check_escrow_balance("kaspatest:qtest")
            .await
            .unwrap();
        assert_eq!(status.total_balance, 5_000_000);
        assert_eq!(status.utxo_count, 1);
    }

    #[test]
    fn test_evaluate_deposits_none() {
        let (watcher, _) = make_watcher();
        let status = EscrowStatus {
            escrow_address: "kaspatest:qtest".to_string(),
            total_balance: 0,
            utxo_count: 0,
            total_balance_kas: 0.0,
        };
        let eval = watcher.evaluate_deposits(&status, 5_000_000);
        assert!(!eval.player_a_deposited);
        assert!(!eval.player_b_deposited);
        assert!(!eval.both_deposited);
    }

    #[test]
    fn test_evaluate_deposits_one() {
        let (watcher, _) = make_watcher();
        let status = EscrowStatus {
            escrow_address: "kaspatest:qtest".to_string(),
            total_balance: 5_000_000,
            utxo_count: 1,
            total_balance_kas: 50.0,
        };
        let eval = watcher.evaluate_deposits(&status, 5_000_000);
        assert!(eval.player_a_deposited);
        assert!(!eval.player_b_deposited);
        assert!(!eval.both_deposited);
    }

    #[test]
    fn test_evaluate_deposits_both() {
        let (watcher, _) = make_watcher();
        let status = EscrowStatus {
            escrow_address: "kaspatest:qtest".to_string(),
            total_balance: 10_000_000,
            utxo_count: 2,
            total_balance_kas: 100.0,
        };
        let eval = watcher.evaluate_deposits(&status, 5_000_000);
        assert!(eval.player_a_deposited);
        assert!(eval.player_b_deposited);
        assert!(eval.both_deposited);
    }
}

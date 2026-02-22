/// Payout calculation and execution utilities.
///
/// This module is now a thin wrapper around EscrowService's payout logic.
/// Preserved for backward compatibility with existing route handlers.
use std::sync::Arc;

use serde::Serialize;

use crate::escrow::EscrowService;
use crate::rpc::KaspaRpc;
use battle_core::types::SOMPI_PER_KAS;

/// PayoutManager wraps EscrowService for payout operations.
pub struct PayoutManager {
    escrow_service: Arc<EscrowService>,
}

/// Breakdown of a payout distribution.
#[derive(Debug, Clone, Serialize)]
pub struct PayoutBreakdown {
    pub total_pot: u64,
    pub winner_amount: u64,
    pub platform_fee: u64,
    pub estimated_network_fee: u64,
    pub winner_amount_kas: f64,
    pub platform_fee_kas: f64,
}

/// Result of a payout operation.
#[derive(Debug, Clone, Serialize)]
pub struct PayoutResult {
    pub match_id: String,
    pub tx_id: String,
    pub winner_address: String,
    pub winner_amount: u64,
    pub platform_fee: u64,
    pub timestamp: String,
}

/// Errors during payout execution.
#[derive(Debug, Clone)]
pub enum PayoutError {
    InsufficientFunds { required: u64, available: u64 },
    EscrowNotFound(String),
    TxFailed(String),
}

impl std::fmt::Display for PayoutError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            PayoutError::InsufficientFunds {
                required,
                available,
            } => {
                write!(
                    f,
                    "Insufficient funds: need {} sompi, have {}",
                    required, available
                )
            }
            PayoutError::EscrowNotFound(id) => write!(f, "Escrow not found: {}", id),
            PayoutError::TxFailed(e) => write!(f, "Transaction failed: {}", e),
        }
    }
}

impl std::error::Error for PayoutError {}

impl PayoutManager {
    /// Create a new PayoutManager backed by an EscrowService.
    pub fn new(escrow_service: Arc<EscrowService>) -> Self {
        Self { escrow_service }
    }

    /// Calculate the payout breakdown for a given total pot.
    pub fn calculate_payout(&self, total_pot: u64) -> PayoutBreakdown {
        let bd = self.escrow_service.calculate_payout(total_pot);
        PayoutBreakdown {
            total_pot: bd.total_pot,
            winner_amount: bd.winner_amount,
            platform_fee: bd.platform_fee,
            estimated_network_fee: bd.estimated_network_fee,
            winner_amount_kas: bd.winner_amount_kas,
            platform_fee_kas: bd.platform_fee_kas,
        }
    }

    /// Execute the payout for a resolved match.
    pub async fn execute_payout(
        &self,
        match_id: &str,
        winner_address: &str,
        treasury_address: &str,
        escrow_address: &str,
        total_pot: u64,
    ) -> Result<PayoutResult, PayoutError> {
        let result = self
            .escrow_service
            .payout_winner(
                match_id,
                winner_address,
                treasury_address,
                escrow_address,
                total_pot,
            )
            .await
            .map_err(|e| PayoutError::TxFailed(e.to_string()))?;

        Ok(PayoutResult {
            match_id: result.challenge_id,
            tx_id: result.payout_tx_id,
            winner_address: result.winner_address,
            winner_amount: result.amount_sompi,
            platform_fee: result.fee_sompi,
            timestamp: result.timestamp,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mock::MockKaspaClient;
    use crate::wallet::EscrowWallet;

    fn make_payout_manager() -> PayoutManager {
        let wallet = Arc::new(
            EscrowWallet::new(
                Some(
                    "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about".to_string()
                ),
                "testnet",
            )
            .unwrap(),
        );
        let rpc: Arc<dyn KaspaRpc> = Arc::new(MockKaspaClient::new());
        let escrow_service = Arc::new(EscrowService::new(wallet, rpc));
        PayoutManager::new(escrow_service)
    }

    #[test]
    fn test_calculate_payout() {
        let pm = make_payout_manager();
        let breakdown = pm.calculate_payout(10_000_000);
        assert_eq!(breakdown.total_pot, 10_000_000);
        assert_eq!(breakdown.winner_amount, 9_500_000); // 95%
        assert_eq!(breakdown.platform_fee, 500_000); // 5%
    }

    #[test]
    fn test_calculate_payout_small_amount() {
        let pm = make_payout_manager();
        let breakdown = pm.calculate_payout(100);
        assert_eq!(breakdown.total_pot, 100);
        assert_eq!(breakdown.platform_fee, 5);
        assert_eq!(breakdown.winner_amount, 95);
    }

    #[test]
    fn test_payout_error_display() {
        let err = PayoutError::InsufficientFunds {
            required: 1000,
            available: 500,
        };
        assert!(format!("{}", err).contains("Insufficient"));
    }
}

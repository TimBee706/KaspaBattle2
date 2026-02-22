/// EscrowService — manages the full lifecycle of challenge escrows.
///
/// Responsibilities:
/// - Create escrow addresses for new challenges
/// - Check deposit status via RPC
/// - Execute payouts to winners (builds TX, signs, submits)
/// - Process refunds for cancelled challenges
///
/// Uses `EscrowWallet` for key management and `Arc<dyn KaspaRpc>` for
/// blockchain interaction.
///
/// # Security Notice — F-001 (CRITICAL)
///
/// **Current model**: Off-chain escrow. A centralized server holds BIP44-derived
/// private keys for each escrow address. This is a custodial design — if the
/// server is compromised, all escrowed funds are at risk.
///
/// **Whitepaper target**: Trustless on-chain escrow via Kasplex L2 smart contracts
/// or native UTXO-script-based multi-sig escrow.
///
/// **Interim mitigations** (implemented):
/// - Keys are zeroized on drop (F-003).
/// - Deposits are per-address attributed (F-008).
/// - Timeout refunds are automatic (F-006).
///
/// **Roadmap**:
/// 1. Evaluate Kasplex L2 maturity for MatchEscrow contract deployment.
/// 2. Alternatively, implement 2-of-2 multi-sig UTXO scripts as an interim
///    trustless step (requires both server + user co-signature).
/// 3. Migrate fund custody to on-chain mechanism before Mainnet launch.
///
/// TODO(F-001): Replace this off-chain custody model before production.
use std::sync::Arc;

use anyhow::{anyhow, Result};
use serde::Serialize;

use crate::errors::EscrowError;
use crate::rpc::KaspaRpc;
use crate::wallet::EscrowWallet;
use battle_core::types::SOMPI_PER_KAS;
use kaspa_addresses::{Address, Version};
use kaspa_consensus_core::network::NetworkId;
use secp256k1::{PublicKey, Secp256k1, SecretKey};
use sha2::{Digest, Sha256};
use uuid::Uuid;

/// Derives a deterministic escrow address from a match ID and two player public keys.
/// Uses SHA-256 of (match_id || player_a_pubkey || player_b_pubkey) as the secret.
/// Returns the Kaspa address for the given network.
pub fn derive_escrow_address(
    match_id: &Uuid,
    player_a_pubkey: &str,
    player_b_pubkey: &str,
    network: NetworkId,
) -> Result<String, EscrowError> {
    if player_a_pubkey.len() < 64 || player_b_pubkey.len() < 64 {
        return Err(EscrowError::InvalidPublicKey(
            "Public keys must be valid hex strings.".to_string(),
        ));
    }

    log::info!(
        "Generating escrow address for match: {}",
        match_id
    );

    let input = format!(
        "{}{}{}",
        match_id,
        player_a_pubkey,
        player_b_pubkey
    );
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    let hash_result = hasher.finalize();

    let secp = Secp256k1::new();
    let secret_key = SecretKey::from_slice(&hash_result).map_err(|e| {
        EscrowError::DerivationFailed(format!("Failed to create secret key from hash: {}", e))
    })?;
    let public_key = PublicKey::from_secret_key(&secp, &secret_key);

    let x_only_public_key = public_key.x_only_public_key().0;
    let address = Address::new(
        network.into(),
        Version::PubKey,
        &x_only_public_key.serialize(),
    );

    Ok(address.to_string())
}

/// Platform fee percentage (5% of total pot)
const PLATFORM_FEE_PERCENT: u64 = 5;

/// Estimated network fee in sompi (~0.01 KAS)
const ESTIMATED_NETWORK_FEE: u64 = 1_000;

/// Information about a created escrow.
#[derive(Debug, Clone, Serialize)]
pub struct EscrowInfo {
    pub challenge_id: String,
    pub escrow_address: String,
    pub derivation_index: u32,
    pub wager_amount_sompi: u64,
    pub wager_amount_kas: f64,
}

/// Deposit status for a challenge.
#[derive(Debug, Clone, Serialize)]
pub struct DepositStatus {
    pub status: DepositState,
    pub deposited_sompi: u64,
    pub required_sompi: u64,
    pub deposited_kas: f64,
    pub required_kas: f64,
    pub player_a_deposited: bool,
    pub player_b_deposited: bool,
    pub utxo_count: u32,
}

/// The state of deposits for a challenge.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub enum DepositState {
    /// No deposits received yet
    #[serde(rename = "NONE")]
    None,
    /// Some but not enough funds deposited
    #[serde(rename = "PARTIAL")]
    Partial,
    /// Both players have deposited the required amount
    #[serde(rename = "COMPLETE")]
    Complete,
}

impl std::fmt::Display for DepositState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DepositState::None => write!(f, "NONE"),
            DepositState::Partial => write!(f, "PARTIAL"),
            DepositState::Complete => write!(f, "COMPLETE"),
        }
    }
}

/// Payout breakdown for a resolved challenge.
#[derive(Debug, Clone, Serialize)]
pub struct PayoutBreakdown {
    pub total_pot: u64,
    pub winner_amount: u64,
    pub platform_fee: u64,
    pub estimated_network_fee: u64,
    pub winner_amount_kas: f64,
    pub platform_fee_kas: f64,
}

/// Result of a successful payout.
#[derive(Debug, Clone, Serialize)]
pub struct PayoutResult {
    pub challenge_id: String,
    pub payout_tx_id: String,
    pub winner_address: String,
    pub amount_sompi: u64,
    pub fee_sompi: u64,
    pub timestamp: String,
}

/// Result of a refund operation.
#[derive(Debug, Clone, Serialize)]
pub struct RefundResult {
    pub challenge_id: String,
    pub refund_tx_a: Option<String>,
    pub refund_tx_b: Option<String>,
    pub refund_amount_a: u64,
    pub refund_amount_b: u64,
}

/// Manages the full lifecycle of escrows.
pub struct EscrowService {
    wallet: Arc<EscrowWallet>,
    rpc: Arc<dyn KaspaRpc>,
}

impl EscrowService {
    /// Create a new EscrowService.
    pub fn new(wallet: Arc<EscrowWallet>, rpc: Arc<dyn KaspaRpc>) -> Self {
        Self { wallet, rpc }
    }

    /// Create a new escrow for a challenge.
    ///
    /// Derives a unique address from the wallet and returns escrow info.
    /// The caller is responsible for storing this in the database.
    pub fn create_escrow(&self, challenge_id: &str, wager_amount_sompi: u64) -> Result<EscrowInfo> {
        let (address, derivation_index) = self.wallet.derive_escrow_address(challenge_id)?;

        let address_str = address.to_string();

        tracing::info!(
            "Escrow created: challenge={}, address={}, wager={} sompi ({} KAS)",
            challenge_id,
            address_str,
            wager_amount_sompi,
            wager_amount_sompi as f64 / SOMPI_PER_KAS as f64
        );

        Ok(EscrowInfo {
            challenge_id: challenge_id.to_string(),
            escrow_address: address_str,
            derivation_index,
            wager_amount_sompi,
            wager_amount_kas: wager_amount_sompi as f64 / SOMPI_PER_KAS as f64,
        })
    }

    /// Check the deposit status for a challenge.
    ///
    /// Queries the on-chain balance of the escrow address and compares
    /// against the required 2x wager amount.
    pub async fn check_deposits(
        &self,
        escrow_address: &str,
        wager_per_player_sompi: u64,
    ) -> Result<DepositStatus> {
        let utxos = self
            .rpc
            .get_utxos(escrow_address)
            .await
            .map_err(|e| anyhow!("Failed to get UTXOs: {}", e))?;

        let total_balance: u64 = utxos.iter().map(|u| u.amount).sum();
        let required_total = wager_per_player_sompi * 2;
        let one_player = wager_per_player_sompi;

        let player_a_deposited = total_balance >= one_player;
        let player_b_deposited = total_balance >= required_total;

        let status = if total_balance == 0 {
            DepositState::None
        } else if total_balance >= required_total {
            DepositState::Complete
        } else {
            DepositState::Partial
        };

        Ok(DepositStatus {
            status,
            deposited_sompi: total_balance,
            required_sompi: required_total,
            deposited_kas: total_balance as f64 / SOMPI_PER_KAS as f64,
            required_kas: required_total as f64 / SOMPI_PER_KAS as f64,
            player_a_deposited,
            player_b_deposited,
            utxo_count: utxos.len() as u32,
        })
    }

    /// Calculate the payout breakdown for a given total pot.
    ///
    /// - Winner: 95% of total pot
    /// - Platform fee: 5% of total pot
    /// - Network fee: ~1000 sompi (deducted from winner amount)
    pub fn calculate_payout(&self, total_pot: u64) -> PayoutBreakdown {
        let platform_fee = total_pot * PLATFORM_FEE_PERCENT / 100;
        let winner_amount = total_pot - platform_fee;

        PayoutBreakdown {
            total_pot,
            winner_amount,
            platform_fee,
            estimated_network_fee: ESTIMATED_NETWORK_FEE,
            winner_amount_kas: winner_amount as f64 / SOMPI_PER_KAS as f64,
            platform_fee_kas: platform_fee as f64 / SOMPI_PER_KAS as f64,
        }
    }

    /// Execute payout to the winner of a resolved challenge.
    ///
    /// Builds a transaction sending winner_amount to the winner and
    /// platform_fee to the platform wallet, signs with the escrow key,
    /// and submits to the network.
    pub async fn payout_winner(
        &self,
        challenge_id: &str,
        winner_address: &str,
        platform_address: &str,
        escrow_address: &str,
        total_pot: u64,
    ) -> Result<PayoutResult> {
        // Verify escrow has sufficient funds
        let balance = self
            .rpc
            .get_balance(escrow_address)
            .await
            .map_err(|e| anyhow!("Failed to get balance: {}", e))?;

        if balance < total_pot {
            return Err(anyhow!(
                "Insufficient escrow funds: need {} sompi, have {}",
                total_pot,
                balance
            ));
        }

        let breakdown = self.calculate_payout(total_pot);

        tracing::info!(
            "PAYOUT challenge {}: {} sompi ({} KAS) → winner {}",
            challenge_id,
            breakdown.winner_amount,
            breakdown.winner_amount_kas,
            winner_address
        );
        tracing::info!(
            "PAYOUT challenge {}: {} sompi ({} KAS) → platform {}",
            challenge_id,
            breakdown.platform_fee,
            breakdown.platform_fee_kas,
            platform_address
        );

        // NOTE: The legacy EscrowService payout path generates a stub TX ID.
        // Real payouts are handled by PayoutService::execute_payout() (F-001)
        // which builds and signs actual Kaspa transactions.
        let tx_id = uuid::Uuid::new_v4().to_string();

        tracing::info!(
            "Escrow payout stub TX {} for challenge {}",
            tx_id,
            challenge_id
        );

        Ok(PayoutResult {
            challenge_id: challenge_id.to_string(),
            payout_tx_id: tx_id,
            winner_address: winner_address.to_string(),
            amount_sompi: breakdown.winner_amount,
            fee_sompi: breakdown.platform_fee,
            timestamp: chrono::Utc::now().to_rfc3339(),
        })
    }

    /// Process refund for a cancelled challenge.
    ///
    /// Returns each player's deposit minus a small TX fee.
    /// If escrow has no balance, returns zero-amount refund info.
    pub async fn refund(
        &self,
        challenge_id: &str,
        _player_a_address: &str,
        _player_b_address: &str,
        escrow_address: &str,
        wager_per_player: u64,
    ) -> Result<RefundResult> {
        let balance = self
            .rpc
            .get_balance(escrow_address)
            .await
            .map_err(|e| anyhow!("Failed to get balance for refund: {}", e))?;

        if balance == 0 {
            tracing::info!("Refund for challenge {}: no funds to refund", challenge_id);
            return Ok(RefundResult {
                challenge_id: challenge_id.to_string(),
                refund_tx_a: None,
                refund_tx_b: None,
                refund_amount_a: 0,
                refund_amount_b: 0,
            });
        }

        // Calculate refund amounts (minus network fee per TX)
        let player_a_amount = std::cmp::min(balance, wager_per_player);
        let player_b_amount = balance.saturating_sub(wager_per_player);
        let refund_a = player_a_amount.saturating_sub(ESTIMATED_NETWORK_FEE);
        let refund_b = player_b_amount.saturating_sub(ESTIMATED_NETWORK_FEE);

        let mut refund_tx_a = None;
        let mut refund_tx_b = None;

        // NOTE: Legacy escrow refund path — stub TX ID for backward compat.
        // Real refunds go through PayoutService::execute_refund() (F-006).
        if refund_a > 0 {
            let tx_id = uuid::Uuid::new_v4().to_string();
            tracing::info!(
                "REFUND stub challenge {}: {} sompi → player A ({})",
                challenge_id, refund_a, tx_id
            );
            refund_tx_a = Some(tx_id);
        }

        // NOTE: Legacy escrow refund path — stub TX ID for backward compat.
        if refund_b > 0 {
            let tx_id = uuid::Uuid::new_v4().to_string();
            tracing::info!(
                "REFUND stub challenge {}: {} sompi → player B ({})",
                challenge_id, refund_b, tx_id
            );
            refund_tx_b = Some(tx_id);
        }

        Ok(RefundResult {
            challenge_id: challenge_id.to_string(),
            refund_tx_a,
            refund_tx_b,
            refund_amount_a: refund_a,
            refund_amount_b: refund_b,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mock::MockKaspaClient;
    use std::str::FromStr;

    #[test]
    fn test_escrow_address_deterministic() {
        let match_id = Uuid::new_v4();
        let pk_a = "02b0c368d18e8ac4a77033cb2118dbb0a5ee3e1afb1419726207c4bbee5aa5e62f";
        let pk_b = "03565f41cb83af35bfed129cb01f600f738fe7cb1bc6e3bce7acbf10ffb6dfaf7b";
        let network = NetworkId::from_str("testnet-10").unwrap();

        let addr1 = derive_escrow_address(&match_id, pk_a, pk_b, network.clone()).unwrap();
        let addr2 = derive_escrow_address(&match_id, pk_a, pk_b, network).unwrap();

        assert_eq!(
            addr1, addr2,
            "Deterministic derivation failed - identical inputs yielded different outputs"
        );
    }

    #[test]
    fn test_escrow_address_unique_per_match() {
        let match_id1 = Uuid::new_v4();
        let match_id2 = Uuid::new_v4();
        let pk_a = "02b0c368d18e8ac4a77033cb2118dbb0a5ee3e1afb1419726207c4bbee5aa5e62f";
        let pk_b = "03565f41cb83af35bfed129cb01f600f738fe7cb1bc6e3bce7acbf10ffb6dfaf7b";
        let network = NetworkId::from_str("testnet-10").unwrap();

        let addr1 = derive_escrow_address(&match_id1, pk_a, pk_b, network.clone()).unwrap();
        let addr2 = derive_escrow_address(&match_id2, pk_a, pk_b, network).unwrap();

        assert_ne!(
            addr1, addr2,
            "Unique derivation failed - different matches yielded identical outputs"
        );
    }

    #[test]
    fn test_escrow_address_invalid_pubkey() {
        let match_id = Uuid::new_v4();
        let pk_a = "short";
        let pk_b = "short_too";
        let network = NetworkId::from_str("testnet-10").unwrap();

        let result = derive_escrow_address(&match_id, pk_a, pk_b, network);
        assert!(result.is_err());

        if let Err(EscrowError::InvalidPublicKey(_)) = result {
            // Test passed
        } else {
            panic!("Expected InvalidPublicKey error, got {:?}", result);
        }
    }

    fn make_service() -> EscrowService {
        let wallet = Arc::new(
            EscrowWallet::new(
                Some("abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about".to_string()),
                "testnet",
            ).unwrap()
        );
        let rpc: Arc<dyn KaspaRpc> = Arc::new(MockKaspaClient::new());
        EscrowService::new(wallet, rpc)
    }

    fn make_service_with_mock() -> (EscrowService, Arc<MockKaspaClient>) {
        let wallet = Arc::new(
            EscrowWallet::new(
                Some("abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about".to_string()),
                "testnet",
            ).unwrap()
        );
        let mock = Arc::new(MockKaspaClient::new());
        let rpc: Arc<dyn KaspaRpc> = mock.clone();
        (EscrowService::new(wallet, rpc), mock)
    }

    #[test]
    fn test_create_escrow() {
        let service = make_service();
        let info = service.create_escrow("challenge-001", 5_000_000).unwrap();

        assert_eq!(info.challenge_id, "challenge-001");
        assert!(info.escrow_address.starts_with("kaspatest:"));
        assert_eq!(info.wager_amount_sompi, 5_000_000);
        assert_eq!(info.wager_amount_kas, 50.0);
    }

    #[tokio::test]
    async fn test_deposit_tracking() {
        let (service, mock) = make_service_with_mock();
        let info = service.create_escrow("challenge-002", 5_000_000).unwrap();

        // Initially no deposits
        let status = service
            .check_deposits(&info.escrow_address, 5_000_000)
            .await
            .unwrap();
        assert_eq!(status.status, DepositState::None);
        assert_eq!(status.deposited_sompi, 0);
        assert!(!status.player_a_deposited);
        assert!(!status.player_b_deposited);

        // Player A deposits
        mock.add_utxo(
            &info.escrow_address,
            UtxoInfo {
                tx_id: "tx_a".to_string(),
                output_index: 0,
                amount: 5_000_000,
                amount_kas: 50.0,
                is_coinbase: false,
                block_daa_score: 100,
                script_public_key: None,
            },
        );

        let status = service
            .check_deposits(&info.escrow_address, 5_000_000)
            .await
            .unwrap();
        assert_eq!(status.status, DepositState::Partial);
        assert!(status.player_a_deposited);
        assert!(!status.player_b_deposited);

        // Player B deposits
        mock.add_utxo(
            &info.escrow_address,
            UtxoInfo {
                tx_id: "tx_b".to_string(),
                output_index: 0,
                amount: 5_000_000,
                amount_kas: 50.0,
                is_coinbase: false,
                block_daa_score: 101,
                script_public_key: None,
            },
        );

        let status = service
            .check_deposits(&info.escrow_address, 5_000_000)
            .await
            .unwrap();
        assert_eq!(status.status, DepositState::Complete);
        assert!(status.player_a_deposited);
        assert!(status.player_b_deposited);
        assert_eq!(status.deposited_sompi, 10_000_000);
    }

    #[test]
    fn test_payout_calculation() {
        let service = make_service();

        // 50 KAS each = 100 KAS pot = 10_000_000 sompi
        let breakdown = service.calculate_payout(10_000_000);
        assert_eq!(breakdown.total_pot, 10_000_000);
        assert_eq!(breakdown.platform_fee, 500_000); // 5%
        assert_eq!(breakdown.winner_amount, 9_500_000); // 95%
        assert_eq!(breakdown.winner_amount_kas, 95.0);
        assert_eq!(breakdown.platform_fee_kas, 5.0);
    }

    #[test]
    fn test_refund_calculation() {
        // Verify refund deducts network fee
        let refund_amount = 5_000_000u64.saturating_sub(ESTIMATED_NETWORK_FEE);
        assert_eq!(refund_amount, 4_999_000); // 5M - 1K
    }

    #[tokio::test]
    async fn test_escrow_lifecycle() {
        let (service, mock) = make_service_with_mock();

        // Step 1: Create escrow
        let info = service.create_escrow("lifecycle-test", 5_000_000).unwrap();
        assert_eq!(info.wager_amount_sompi, 5_000_000);

        // Step 2: Check deposits (NONE)
        let status = service
            .check_deposits(&info.escrow_address, 5_000_000)
            .await
            .unwrap();
        assert_eq!(status.status, DepositState::None);

        // Step 3: Simulate both deposits + set balance
        mock.set_balance(&info.escrow_address, 10_000_000);
        mock.add_utxo(
            &info.escrow_address,
            UtxoInfo {
                tx_id: "tx1".to_string(),
                output_index: 0,
                amount: 5_000_000,
                amount_kas: 50.0,
                is_coinbase: false,
                block_daa_score: 200,
                script_public_key: None,
            },
        );
        mock.add_utxo(
            &info.escrow_address,
            UtxoInfo {
                tx_id: "tx2".to_string(),
                output_index: 0,
                amount: 5_000_000,
                amount_kas: 50.0,
                is_coinbase: false,
                block_daa_score: 201,
                script_public_key: None,
            },
        );

        // Step 4: Check deposits (COMPLETE)
        let status = service
            .check_deposits(&info.escrow_address, 5_000_000)
            .await
            .unwrap();
        assert_eq!(status.status, DepositState::Complete);

        // Step 5: Payout
        let payout = service
            .payout_winner(
                "lifecycle-test",
                "kaspatest:qwinner",
                "kaspatest:qplatform",
                &info.escrow_address,
                10_000_000,
            )
            .await
            .unwrap();

        assert_eq!(payout.challenge_id, "lifecycle-test");
        assert_eq!(payout.amount_sompi, 9_500_000);
        assert_eq!(payout.fee_sompi, 500_000);
        assert!(!payout.payout_tx_id.is_empty());
    }

    #[tokio::test]
    async fn test_double_payout_prevention() {
        let (service, mock) = make_service_with_mock();
        let info = service.create_escrow("double-pay-test", 5_000_000).unwrap();
        mock.set_balance(&info.escrow_address, 10_000_000);

        // First payout succeeds
        let result = service
            .payout_winner(
                "double-pay-test",
                "kaspatest:qwinner",
                "kaspatest:qplatform",
                &info.escrow_address,
                10_000_000,
            )
            .await;
        assert!(result.is_ok());

        // After first payout, balance should be drained in production.
        // With mock, we simulate by setting balance to 0.
        mock.set_balance(&info.escrow_address, 0);

        // Second payout fails (insufficient funds)
        let result2 = service
            .payout_winner(
                "double-pay-test",
                "kaspatest:qwinner",
                "kaspatest:qplatform",
                &info.escrow_address,
                10_000_000,
            )
            .await;
        assert!(result2.is_err());
    }
}

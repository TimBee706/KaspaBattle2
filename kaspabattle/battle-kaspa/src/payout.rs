/// Payout service — builds, signs, and submits real Kaspa transactions.
///
/// F-001: Replaces mock TX hashes with real transaction construction.
/// Uses the escrow wallet's private key to sign UTXO inputs via Schnorr
/// and submits as RpcTransaction. Two outputs: 95% winner, 5% treasury.
///
/// F-009: execute_payout() refuses to run if the match state blocks payouts.
use crate::errors::PayoutError;
use crate::rpc::KaspaBackend;
use battle_core::models::match_::BattleMatch;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

// Kaspa consensus imports
use kaspa_addresses::Address;
use kaspa_consensus_core::{
    hashing::{
        sighash::{calc_schnorr_signature_hash, SigHashReusedValues},
        sighash_type::SIG_HASH_ALL,
    },
    subnets::SUBNETWORK_ID_NATIVE,
    tx::{
        PopulatedTransaction, Transaction, TransactionId, TransactionInput, TransactionOutpoint,
        TransactionOutput, UtxoEntry,
    },
};
use kaspa_txscript::pay_to_address_script;

// RPC type conversion imports
use kaspa_rpc_core::model::tx::{
    RpcTransaction, RpcTransactionInput, RpcTransactionOutpoint, RpcTransactionOutput,
};

/// Fee percentage constants — must sum to 100.
const WINNER_PCT: u64 = 95;
// Treasury receives 5% total (3% protocol + 2% oracle reserve, consolidated until Phase 2)

/// Minimum transaction mass estimate in grams (used when fee estimate unavailable).
const ESTIMATED_TX_MASS_GRAMS: u64 = 2000;

pub struct PayoutService {
    rpc_client: Arc<dyn KaspaBackend>,
    treasury_address: String,
    /// Map: escrow_address → 32-byte raw private key (Schnorr/secp256k1)
    escrow_private_keys: Arc<Mutex<HashMap<String, [u8; 32]>>>,
}

#[derive(Debug, Clone)]
pub struct PayoutResult {
    pub winner_tx_id: String,
    pub winner_amount_sompi: u64,
    pub treasury_amount_sompi: u64,
    pub fee_sompi: u64,
}

impl PayoutService {
    pub fn new(
        rpc_client: Arc<dyn KaspaBackend>,
        treasury_address: String,
        escrow_private_keys: HashMap<String, [u8; 32]>,
    ) -> Self {
        Self {
            rpc_client,
            treasury_address,
            escrow_private_keys: Arc::new(Mutex::new(escrow_private_keys)),
        }
    }

    /// Register an escrow signing key at runtime (called after wallet derives a new address).
    pub async fn register_escrow_key(&self, escrow_address: String, private_key: [u8; 32]) {
        let mut keys = self.escrow_private_keys.lock().await;
        keys.insert(escrow_address, private_key);
    }

    /// Validate that the escrow holds enough funds.
    pub async fn validate_escrow_balance(
        &self,
        escrow_address: &str,
        expected_amount_sompi: u64,
    ) -> Result<bool, PayoutError> {
        for attempt in 0..3u32 {
            match self.rpc_client.get_balance(escrow_address).await {
                Ok(balance) => return Ok(balance >= expected_amount_sompi),
                Err(_e) if attempt < 2 => {
                    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
                    continue;
                }
                Err(e) => return Err(PayoutError::KaspaRpcError(e)),
            }
        }
        Ok(false)
    }

    /// F-001: Execute a real payout to the match winner.
    ///
    /// Builds a transaction with two outputs:
    ///   - 95% of total_pot → winner_address
    ///   - 5%  of total_pot → treasury_address
    ///
    /// Transaction inputs are the escrow UTXOs. Each input is signed with
    /// Schnorr using the escrow's private key.
    ///
    /// F-009: Returns Err(MatchInDisputedState) when payout is blocked.
    pub async fn execute_payout(
        &self,
        battle_match: &BattleMatch,
        winner_address: &str,
    ) -> Result<PayoutResult, PayoutError> {
        // F-009: Block payouts when state doesn't allow it
        if !battle_match.status.allows_payout() {
            return Err(PayoutError::MatchInDisputedState);
        }

        // F-003: Node sync check
        let synced = self
            .rpc_client
            .is_synced()
            .await
            .map_err(PayoutError::KaspaRpcError)?;
        if !synced {
            return Err(PayoutError::NodeNotReady);
        }

        let total_pot = battle_match.wager_amount_sompi * 2;

        // Validate escrow balance
        let is_valid = self
            .validate_escrow_balance(&battle_match.escrow_address, total_pot)
            .await?;
        if !is_valid {
            let found = self
                .rpc_client
                .get_balance(&battle_match.escrow_address)
                .await
                .unwrap_or(0);
            return Err(PayoutError::InsufficientEscrowBalance {
                expected: total_pot,
                found,
            });
        }

        // Get fee estimate
        let fee_estimate = self
            .rpc_client
            .get_fee_estimate()
            .await
            .map_err(PayoutError::KaspaRpcError)?;
        let fee_sompi = ((fee_estimate.normal_bucket_feerate * ESTIMATED_TX_MASS_GRAMS as f64)
            .ceil() as u64)
            .max(1000);

        // Calculate split
        let net_pot = total_pot.saturating_sub(fee_sompi);
        let winner_amount = net_pot * WINNER_PCT / 100;
        let treasury_amount = net_pot.saturating_sub(winner_amount);

        // Get UTXOs
        let utxos = self
            .rpc_client
            .get_utxos(&battle_match.escrow_address)
            .await
            .map_err(PayoutError::KaspaRpcError)?;

        if utxos.is_empty() {
            return Err(PayoutError::InsufficientEscrowBalance {
                expected: total_pot,
                found: 0,
            });
        }

        // Retrieve escrow signing key
        let private_key_bytes = {
            let keys = self.escrow_private_keys.lock().await;
            keys.get(&battle_match.escrow_address)
                .copied()
                .ok_or_else(|| {
                    PayoutError::SigningKeyNotFound(battle_match.escrow_address.clone())
                })?
        };

        // Build addresses
        let winner_addr = Address::try_from(winner_address)
            .map_err(|e| PayoutError::TxBuildError(format!("Invalid winner address: {}", e)))?;
        let treasury_addr = Address::try_from(self.treasury_address.as_str())
            .map_err(|e| PayoutError::TxBuildError(format!("Invalid treasury address: {}", e)))?;
        let escrow_addr = Address::try_from(battle_match.escrow_address.as_str())
            .map_err(|e| PayoutError::TxBuildError(format!("Invalid escrow address: {}", e)))?;
        let escrow_script_pk = pay_to_address_script(&escrow_addr);

        // Build transaction inputs
        let inputs: Vec<TransactionInput> = utxos
            .iter()
            .map(|u| TransactionInput {
                previous_outpoint: TransactionOutpoint {
                    transaction_id: TransactionId::from_slice(
                        &hex::decode(&u.tx_id).unwrap_or_else(|_| vec![0u8; 32]),
                    ),
                    index: u.output_index,
                },
                signature_script: vec![],
                sequence: u64::MAX,
                sig_op_count: 1,
            })
            .collect();

        // Build transaction outputs
        let outputs = vec![
            TransactionOutput {
                value: winner_amount,
                script_public_key: pay_to_address_script(&winner_addr),
            },
            TransactionOutput {
                value: treasury_amount,
                script_public_key: pay_to_address_script(&treasury_addr),
            },
        ];

        // Assemble unsigned transaction
        let mut tx = Transaction::new(0, inputs, outputs, 0, SUBNETWORK_ID_NATIVE, 0, vec![]);

        // Build UTXO entries for signing
        let utxo_entries: Vec<UtxoEntry> = utxos
            .iter()
            .map(|u| UtxoEntry {
                amount: u.amount,
                script_public_key: escrow_script_pk.clone(),
                block_daa_score: u.block_daa_score,
                is_coinbase: u.is_coinbase,
            })
            .collect();

        // Sign each input with Schnorr.
        // We collect sighashes first to avoid borrow conflicts with tx.inputs
        // (PopulatedTransaction borrows &tx immutably, so we must drop it
        // before mutating tx.inputs below).
        let secp = secp256k1::Secp256k1::new();
        let sk = secp256k1::SecretKey::from_slice(&private_key_bytes)
            .map_err(|e| PayoutError::TxBuildError(format!("Invalid secret key: {}", e)))?;
        let keypair = secp256k1::Keypair::from_secret_key(&secp, &sk);

        // Pass 1: compute all sighashes while holding the immutable borrow
        let sighashes: Vec<kaspa_hashes::Hash> = {
            let mut reused_values = SigHashReusedValues::new();
            let populated = PopulatedTransaction::new(&tx, utxo_entries);
            (0..tx.inputs.len())
                .map(|i| {
                    calc_schnorr_signature_hash(&populated, i, SIG_HASH_ALL, &mut reused_values)
                })
                .collect()
        }; // `populated` and its borrow of `tx` are dropped here

        // Pass 2: sign and insert signature scripts
        for (i, sighash) in sighashes.iter().enumerate() {
            let msg = secp256k1::Message::from_digest(sighash.as_bytes());

            let sig = secp.sign_schnorr(&msg, &keypair);

            // P2PK Schnorr signature: OP_DATA_65 <sig_64_bytes> <sighash_type_byte>
            let mut script = Vec::with_capacity(66);
            script.push(0x41); // OP_DATA_65 (65 bytes follow)
            script.extend_from_slice(&sig.serialize());
            script.push(SIG_HASH_ALL.to_u8());
            tx.inputs[i].signature_script = script;
        }

        let rpc_tx = transaction_to_rpc(tx);
        let _tx_id = rpc_tx.get_id();
        let payload = serde_json::to_string(&rpc_tx)
            .map_err(|e| PayoutError::TxBuildError(format!("Serialization failed: {}", e)))?;

        let submitted_id = self
            .rpc_client
            .submit_transaction(&payload)
            .await
            .map_err(PayoutError::KaspaRpcError)?;

        tracing::info!(
            "✅ Payout TX {} submitted: {} sompi → winner ({}), {} sompi → treasury, {} sompi fee",
            submitted_id,
            winner_amount,
            winner_address,
            treasury_amount,
            fee_sompi
        );

        Ok(PayoutResult {
            winner_tx_id: submitted_id,
            winner_amount_sompi: winner_amount,
            treasury_amount_sompi: treasury_amount,
            fee_sompi,
        })
    }

    /// Execute a refund for a cancelled/timed-out match.
    /// Sends 50/50 of deposited amounts (minus fees) back to both players.
    pub async fn execute_refund(
        &self,
        battle_match: &BattleMatch,
    ) -> Result<(String, String), PayoutError> {
        let synced = self
            .rpc_client
            .is_synced()
            .await
            .map_err(PayoutError::KaspaRpcError)?;
        if !synced {
            return Err(PayoutError::NodeNotReady);
        }

        let escrow_balance = self
            .rpc_client
            .get_balance(&battle_match.escrow_address)
            .await
            .map_err(PayoutError::KaspaRpcError)?;

        if escrow_balance == 0 {
            return Ok(("no-deposit-a".to_string(), "no-deposit-b".to_string()));
        }

        let fee_estimate = self
            .rpc_client
            .get_fee_estimate()
            .await
            .map_err(PayoutError::KaspaRpcError)?;
        let fee_sompi = ((fee_estimate.normal_bucket_feerate * ESTIMATED_TX_MASS_GRAMS as f64)
            .ceil() as u64)
            .max(1000);

        let net = escrow_balance.saturating_sub(fee_sompi);
        let half = net / 2;

        let private_key_bytes = {
            let keys = self.escrow_private_keys.lock().await;
            keys.get(&battle_match.escrow_address)
                .copied()
                .ok_or_else(|| {
                    PayoutError::SigningKeyNotFound(battle_match.escrow_address.clone())
                })?
        };

        let utxos = self
            .rpc_client
            .get_utxos(&battle_match.escrow_address)
            .await
            .map_err(PayoutError::KaspaRpcError)?;

        let escrow_addr = Address::try_from(battle_match.escrow_address.as_str())
            .map_err(|e| PayoutError::TxBuildError(format!("Invalid escrow address: {}", e)))?;
        let addr_a = Address::try_from(battle_match.player_a_kas_address.as_str())
            .map_err(|e| PayoutError::TxBuildError(format!("Invalid player A address: {}", e)))?;
        let addr_b = Address::try_from(battle_match.player_b_kas_address.as_str())
            .map_err(|e| PayoutError::TxBuildError(format!("Invalid player B address: {}", e)))?;

        let escrow_script_pk = pay_to_address_script(&escrow_addr);

        let inputs: Vec<TransactionInput> = utxos
            .iter()
            .map(|u| TransactionInput {
                previous_outpoint: TransactionOutpoint {
                    transaction_id: TransactionId::from_slice(
                        &hex::decode(&u.tx_id).unwrap_or_else(|_| vec![0u8; 32]),
                    ),
                    index: u.output_index,
                },
                signature_script: vec![],
                sequence: u64::MAX,
                sig_op_count: 1,
            })
            .collect();

        let outputs = vec![
            TransactionOutput {
                value: half,
                script_public_key: pay_to_address_script(&addr_a),
            },
            TransactionOutput {
                value: net - half,
                script_public_key: pay_to_address_script(&addr_b),
            },
        ];

        let mut tx = Transaction::new(0, inputs, outputs, 0, SUBNETWORK_ID_NATIVE, 0, vec![]);

        let utxo_entries: Vec<UtxoEntry> = utxos
            .iter()
            .map(|u| UtxoEntry {
                amount: u.amount,
                script_public_key: escrow_script_pk.clone(),
                block_daa_score: u.block_daa_score,
                is_coinbase: u.is_coinbase,
            })
            .collect();

        let secp = secp256k1::Secp256k1::new();
        let sk = secp256k1::SecretKey::from_slice(&private_key_bytes)
            .map_err(|e| PayoutError::TxBuildError(format!("Invalid secret key: {}", e)))?;
        let keypair = secp256k1::Keypair::from_secret_key(&secp, &sk);

        // Pass 1: collect sighashes (drops borrow before mutation)
        let sighashes: Vec<kaspa_hashes::Hash> = {
            let mut reused_values = SigHashReusedValues::new();
            let populated = PopulatedTransaction::new(&tx, utxo_entries);
            (0..tx.inputs.len())
                .map(|i| {
                    calc_schnorr_signature_hash(&populated, i, SIG_HASH_ALL, &mut reused_values)
                })
                .collect()
        };

        // Pass 2: sign and mutate
        for (i, sighash) in sighashes.iter().enumerate() {
            let msg = secp256k1::Message::from_digest(sighash.as_bytes());
            let sig = secp.sign_schnorr(&msg, &keypair);
            let mut script = Vec::with_capacity(66);
            script.push(0x41);
            script.extend_from_slice(&sig.serialize());
            script.push(SIG_HASH_ALL.to_u8());
            tx.inputs[i].signature_script = script;
        }

        let rpc_tx = transaction_to_rpc(tx);
        let payload = serde_json::to_string(&rpc_tx)
            .map_err(|e| PayoutError::TxBuildError(format!("Serialization failed: {}", e)))?;

        let refund_tx_id = self
            .rpc_client
            .submit_transaction(&payload)
            .await
            .map_err(PayoutError::KaspaRpcError)?;

        tracing::info!(
            "💸 Refund TX {} submitted: {} sompi → A, {} sompi → B",
            refund_tx_id,
            half,
            net - half
        );

        Ok((refund_tx_id.clone(), refund_tx_id))
    }
}

/// Convert a consensus Transaction to an RpcTransaction for submission.
fn transaction_to_rpc(tx: Transaction) -> RpcTransaction {
    RpcTransaction {
        version: tx.version,
        inputs: tx
            .inputs
            .into_iter()
            .map(|inp| RpcTransactionInput {
                previous_outpoint: RpcTransactionOutpoint {
                    transaction_id: inp.previous_outpoint.transaction_id,
                    index: inp.previous_outpoint.index,
                },
                signature_script: inp.signature_script,
                sequence: inp.sequence,
                sig_op_count: inp.sig_op_count,
                verbose_data: None,
            })
            .collect(),
        outputs: tx
            .outputs
            .into_iter()
            .map(|out| RpcTransactionOutput {
                value: out.value,
                script_public_key: out.script_public_key,
                verbose_data: None,
            })
            .collect(),
        lock_time: tx.lock_time,
        subnetwork_id: tx.subnetwork_id,
        gas: tx.gas,
        payload: tx.payload,
        mass: 0, // Will be computed by the node
        verbose_data: None,
    }
}

/// Extension trait for RpcTransaction to compute the TX ID locally.
trait RpcTransactionExt {
    fn get_id(&self) -> String;
}

impl RpcTransactionExt for RpcTransaction {
    fn get_id(&self) -> String {
        use sha2::{Digest, Sha256};
        // Simplified ID: hash of (inputs concat outputs) for logging
        let mut hasher = Sha256::new();
        for inp in &self.inputs {
            hasher.update(inp.previous_outpoint.transaction_id.as_bytes());
            hasher.update(inp.previous_outpoint.index.to_le_bytes());
        }
        hex::encode(hasher.finalize())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mock::MockKaspaClient;
    use battle_core::models::match_::{BattleMatch, MatchStatus};
    use uuid::Uuid;

    fn make_service(mock: Arc<MockKaspaClient>) -> PayoutService {
        PayoutService::new(
            mock as Arc<dyn KaspaBackend>,
            "kaspatest:qtreasury".to_string(),
            HashMap::new(),
        )
    }

    fn make_match_with_state(status: MatchStatus) -> BattleMatch {
        BattleMatch {
            id: Uuid::new_v4(),
            player_a_kas_address: "kaspatest:qalice".to_string(),
            player_b_kas_address: "kaspatest:qbob".to_string(),
            player_a_faceit_id: "a".to_string(),
            player_b_faceit_id: "b".to_string(),
            faceit_match_id: None,
            wager_amount_sompi: 5_000_000,
            escrow_address: "kaspatest:qescrow".to_string(),
            status,
            winner_kas_address: None,
            payout_tx_hash: None,
            oracle_result_signature: None,
            created_at: chrono::Utc::now(),
            locked_at: None,
            resolved_at: None,
            timeout_at: chrono::Utc::now() + chrono::Duration::minutes(30),
        }
    }

    #[test]
    fn test_payout_split_95_5() {
        let total_pot = 10_000_000u64;
        let fee = 2_000u64;
        let net_pot = total_pot.saturating_sub(fee);
        let winner_amount = net_pot * WINNER_PCT / 100;
        let treasury_amount = net_pot.saturating_sub(winner_amount);

        assert!(winner_amount > treasury_amount * 15);
        assert_eq!(winner_amount + treasury_amount, net_pot);
    }

    #[test]
    fn test_refund_split_50_50() {
        let balance = 10_000_000u64;
        let fee = 1_000u64;
        let net = balance.saturating_sub(fee);
        let half = net / 2;

        assert_eq!(half + (net - half), net);
    }

    #[tokio::test]
    async fn test_execute_payout_blocked_when_disputed() {
        let mock = Arc::new(MockKaspaClient::new());
        let service = make_service(mock);

        let battle_match = make_match_with_state(MatchStatus::Disputed);

        let result = service
            .execute_payout(&battle_match, "kaspatest:qwinner")
            .await;

        assert!(result.is_err());
        assert!(
            matches!(result.unwrap_err(), PayoutError::MatchInDisputedState),
            "Should be blocked by dispute guard"
        );
    }

    #[tokio::test]
    async fn test_execute_payout_blocked_when_cancelled() {
        let mock = Arc::new(MockKaspaClient::new());
        let service = make_service(mock);

        let battle_match = make_match_with_state(MatchStatus::Cancelled);

        let result = service
            .execute_payout(&battle_match, "kaspatest:qwinner")
            .await;

        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            PayoutError::MatchInDisputedState
        ));
    }

    #[tokio::test]
    async fn test_execute_payout_insufficient_balance() {
        let mock = Arc::new(MockKaspaClient::new());
        let service = make_service(mock);

        let battle_match = make_match_with_state(MatchStatus::Resolved);

        let result = service
            .execute_payout(&battle_match, "kaspatest:qalice")
            .await;

        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            PayoutError::InsufficientEscrowBalance { .. }
        ));
    }

    #[tokio::test]
    async fn test_execute_payout_node_not_ready() {
        let mock = Arc::new(MockKaspaClient::with_synced(false));
        let service = make_service(mock);

        let battle_match = make_match_with_state(MatchStatus::Resolved);

        let result = service
            .execute_payout(&battle_match, "kaspatest:qalice")
            .await;

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), PayoutError::NodeNotReady));
    }
}

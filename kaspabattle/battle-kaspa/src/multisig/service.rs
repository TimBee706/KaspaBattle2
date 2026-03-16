//! High-level multisig escrow lifecycle service.
//!
//! Orchestrates the full escrow flow: address generation → deposit tracking →
//! payout TX creation → signature collection → broadcast.

use std::collections::HashMap;
use std::sync::Arc;

use anyhow::{anyhow, Result};
use chrono::Utc;
use kaspa_addresses::Prefix;
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::multisig::scripts::{
    build_multisig_redeem_script, redeem_script_to_address, redeem_script_to_p2sh,
};
use crate::multisig::transaction::{
    assemble_signed_tx, compute_all_sighashes, create_unsigned_payout_tx,
    create_unsigned_refund_tx, sign_sighash, to_rpc_transaction,
};
use crate::multisig::types::*;
use crate::rpc::KaspaRpc;

use battle_core::types::SOMPI_PER_KAS;

/// Platform fee percentage (5% of total pot)
const PLATFORM_FEE_PERCENT: u64 = 5;

/// Estimated transaction mass in grams (for fee calculation)
const ESTIMATED_TX_MASS_GRAMS: u64 = 3000;

/// MultisigEscrowService — manages the complete lifecycle of multisig escrows.
///
/// ## Architecture
///
/// - **Backend-held keys model**: The backend holds all three keys (Player A, B, Oracle).
///   This is custodial but simpler for v1 UX.
/// - **Key storage**: Private keys are stored in-memory via `escrow_keys` map.
/// - **RPC**: Uses `KaspaRpc` trait for blockchain queries and TX submission.
///
/// ## Lifecycle
///
/// 1. `create_escrow()` → generates 2-of-3 P2SH address
/// 2. `check_deposits()` → polls escrow UTXO balance
/// 3. `create_payout()` → builds unsigned TX, signs with 2 keys, broadcasts
/// 4. `create_refund()` → splits funds back to both players
pub struct MultisigEscrowService {
    rpc: Arc<dyn KaspaRpc>,
    /// Network prefix (Mainnet or Testnet)
    prefix: Prefix,
    /// Platform/Oracle private key (32 bytes)
    oracle_private_key: [u8; 32],
    /// Oracle x-only public key hex
    oracle_pubkey_hex: String,
    /// Platform treasury address for fee collection
    treasury_address: String,
    /// Player private keys: pubkey_hex → private_key_bytes
    /// In backend-held model, we generate and store player keys.
    player_keys: Arc<Mutex<HashMap<String, [u8; 32]>>>,
    /// Active escrows: match_id → MultisigEscrow
    escrows: Arc<Mutex<HashMap<Uuid, MultisigEscrow>>>,
}

impl MultisigEscrowService {
    /// Create a new MultisigEscrowService.
    ///
    /// # Arguments
    /// * `rpc` - Kaspa RPC client
    /// * `prefix` - Network prefix (testnet/mainnet)
    /// * `oracle_private_key` - 32-byte private key for the platform oracle
    /// * `treasury_address` - Address to receive platform fees
    pub fn new(
        rpc: Arc<dyn KaspaRpc>,
        prefix: Prefix,
        oracle_private_key: [u8; 32],
        treasury_address: String,
    ) -> Result<Self> {
        // Derive oracle public key from private key
        let secp = secp256k1::Secp256k1::new();
        let sk = secp256k1::SecretKey::from_slice(&oracle_private_key)
            .map_err(|e| anyhow!("Invalid oracle private key: {}", e))?;
        let keypair = secp256k1::Keypair::from_secret_key(&secp, &sk);
        let (xonly, _) = keypair.x_only_public_key();
        let oracle_pubkey_hex = hex::encode(xonly.serialize());

        Ok(Self {
            rpc,
            prefix,
            oracle_private_key,
            oracle_pubkey_hex,
            treasury_address,
            player_keys: Arc::new(Mutex::new(HashMap::new())),
            escrows: Arc::new(Mutex::new(HashMap::new())),
        })
    }

    /// Create a new 2-of-3 multisig escrow for a match.
    ///
    /// Generates key pairs for both players (backend-held model), builds
    /// the redeem script, and derives the P2SH escrow address.
    ///
    /// # Arguments
    /// * `match_id` - Unique match identifier
    /// * `wager_per_player_sompi` - Wager amount per player in sompi
    /// * `timelock_timestamp` - Optional CLTV time-lock (Unix seconds)
    pub async fn create_escrow(
        &self,
        match_id: Uuid,
        wager_per_player_sompi: u64,
        timelock_timestamp: Option<u64>,
    ) -> Result<MultisigEscrowInfo> {
        // Generate deterministic player keys from match_id
        let (pk_a_hex, sk_a) = self.derive_player_key(&match_id, "player_a")?;
        let (pk_b_hex, sk_b) = self.derive_player_key(&match_id, "player_b")?;

        // Store player keys
        {
            let mut keys = self.player_keys.lock().await;
            keys.insert(pk_a_hex.clone(), sk_a);
            keys.insert(pk_b_hex.clone(), sk_b);
        }

        // Build 2-of-3 redeem script: [Player A, Player B, Oracle]
        let pubkeys = vec![
            pk_a_hex.clone(),
            pk_b_hex.clone(),
            self.oracle_pubkey_hex.clone(),
        ];
        let redeem_script = build_multisig_redeem_script(&pubkeys, 2)
            .map_err(|e| anyhow!("Failed to build redeem script: {}", e))?;

        // Derive P2SH address
        let address = redeem_script_to_address(&redeem_script, self.prefix);
        let address_str = address.to_string();
        let redeem_script_hex = hex::encode(&redeem_script);

        // Create escrow record
        let escrow = MultisigEscrow {
            match_id,
            pubkey_a_hex: pk_a_hex.clone(),
            pubkey_b_hex: pk_b_hex.clone(),
            pubkey_oracle_hex: self.oracle_pubkey_hex.clone(),
            config: MultisigConfig::v1(),
            redeem_script_hex: redeem_script_hex.clone(),
            p2sh_address: address_str.clone(),
            wager_per_player_sompi,
            status: EscrowStatus::Created,
            timelock_timestamp,
            created_at: Utc::now(),
        };

        // Store escrow
        {
            let mut escrows = self.escrows.lock().await;
            escrows.insert(match_id, escrow);
        }

        tracing::info!(
            "✅ Multisig escrow created: match={}, address={}, threshold=2-of-3",
            match_id,
            address_str
        );

        Ok(MultisigEscrowInfo {
            match_id: match_id.to_string(),
            escrow_address: address_str,
            redeem_script_hex,
            pubkeys,
            threshold: 2,
            wager_per_player_sompi,
            wager_per_player_kas: wager_per_player_sompi as f64 / SOMPI_PER_KAS as f64,
            timelock_timestamp,
        })
    }

    /// Check the deposit status for an escrow.
    pub async fn check_deposits(
        &self,
        escrow_address: &str,
        wager_per_player_sompi: u64,
    ) -> Result<MultisigDepositStatus> {
        let utxos = self
            .rpc
            .get_utxos(escrow_address)
            .await
            .map_err(|e| anyhow!("Failed to get UTXOs: {}", e))?;

        let total_balance: u64 = utxos.iter().map(|u| u.amount).sum();
        let required_total = wager_per_player_sompi * 2;

        Ok(MultisigDepositStatus {
            deposited_sompi: total_balance,
            required_sompi: required_total,
            is_fully_funded: total_balance >= required_total,
            utxo_count: utxos.len() as u32,
        })
    }

    /// Execute a payout to the match winner.
    ///
    /// Builds the payout TX, signs with 2-of-3 keys (winner + oracle),
    /// and broadcasts to the network.
    ///
    /// # Arguments
    /// * `match_id` - Match identifier
    /// * `winner_address` - Winner's Kaspa address
    /// * `signer_a_hex` - First signer's pubkey hex (from the escrow's pubkeys)
    /// * `signer_b_hex` - Second signer's pubkey hex (from the escrow's pubkeys)
    pub async fn execute_payout(
        &self,
        match_id: &Uuid,
        winner_address: &str,
    ) -> Result<MultisigPayoutResult> {
        // Get escrow details
        let escrow = {
            let escrows = self.escrows.lock().await;
            escrows
                .get(match_id)
                .cloned()
                .ok_or_else(|| anyhow!("Escrow not found for match {}", match_id))?
        };

        // Verify node is synced
        let synced = self
            .rpc
            .is_synced()
            .await
            .map_err(|e| anyhow!("RPC error: {}", e))?;
        if !synced {
            return Err(anyhow!("Kaspa node is not synced"));
        }

        // Get escrow UTXOs
        let utxos = self
            .rpc
            .get_utxos(&escrow.p2sh_address)
            .await
            .map_err(|e| anyhow!("Failed to get UTXOs: {}", e))?;

        let total_balance: u64 = utxos.iter().map(|u| u.amount).sum();
        let expected = escrow.wager_per_player_sompi * 2;
        if total_balance < expected {
            return Err(anyhow!(
                "Insufficient escrow balance: have {} sompi, need {}",
                total_balance,
                expected
            ));
        }

        // Calculate fee
        let fee_estimate = self
            .rpc
            .get_fee_estimate()
            .await
            .map_err(|e| anyhow!("Fee estimate failed: {}", e))?;
        let network_fee = ((fee_estimate.normal_bucket_feerate * ESTIMATED_TX_MASS_GRAMS as f64)
            .ceil() as u64)
            .max(1000);

        // Calculate split
        let net_pot = total_balance.saturating_sub(network_fee);
        let platform_fee = net_pot * PLATFORM_FEE_PERCENT / 100;
        let winner_amount = net_pot - platform_fee;

        // Build redeem script and P2SH for signing
        let redeem_script = hex::decode(&escrow.redeem_script_hex)
            .map_err(|e| anyhow!("Invalid redeem script hex: {}", e))?;
        let p2sh_spk = redeem_script_to_p2sh(&redeem_script);

        // Create unsigned TX
        let (tx, utxo_entries) = create_unsigned_payout_tx(
            &utxos,
            winner_address,
            winner_amount,
            &self.treasury_address,
            platform_fee,
            &p2sh_spk,
        )
        .map_err(|e| anyhow!("Failed to create TX: {}", e))?;

        // Compute sighashes
        let sighashes = compute_all_sighashes(&tx, utxo_entries);

        // In backend-held model, we sign with 2 keys:
        // Winner's key + Oracle key (standard payout pattern)
        // We try: first player key that matches, then oracle key
        let player_keys = self.player_keys.lock().await;

        // Find a player key to sign with (the "winner" key)
        let first_signer_key = player_keys
            .get(&escrow.pubkey_a_hex)
            .or_else(|| player_keys.get(&escrow.pubkey_b_hex))
            .ok_or_else(|| anyhow!("No player key found"))?;

        // Sign each input with both keys
        let mut sigs_per_input: Vec<Vec<Vec<u8>>> = Vec::new();
        for sighash in &sighashes {
            let sig1 = sign_sighash(sighash, first_signer_key)
                .map_err(|e| anyhow!("Signing failed (player): {}", e))?;
            let sig2 = sign_sighash(sighash, &self.oracle_private_key)
                .map_err(|e| anyhow!("Signing failed (oracle): {}", e))?;
            sigs_per_input.push(vec![sig1, sig2]);
        }

        drop(player_keys);

        // Assemble signed TX
        let signed_tx = assemble_signed_tx(tx, &sigs_per_input, &redeem_script)
            .map_err(|e| anyhow!("Failed to assemble TX: {}", e))?;

        // Broadcast
        let rpc_tx = to_rpc_transaction(signed_tx);
        let tx_id = self
            .rpc
            .submit_rpc_transaction(rpc_tx)
            .await
            .map_err(|e| anyhow!("Broadcast failed: {}", e))?;

        // Update escrow status
        {
            let mut escrows = self.escrows.lock().await;
            if let Some(esc) = escrows.get_mut(match_id) {
                esc.status = EscrowStatus::Settled;
            }
        }

        tracing::info!(
            "✅ Multisig payout TX {} submitted: {} sompi → {} (winner), {} sompi → treasury",
            tx_id,
            winner_amount,
            winner_address,
            platform_fee,
        );

        Ok(MultisigPayoutResult {
            match_id: match_id.to_string(),
            tx_id,
            winner_address: winner_address.to_string(),
            winner_amount_sompi: winner_amount,
            platform_fee_sompi: platform_fee,
            network_fee_sompi: network_fee,
            timestamp: Utc::now().to_rfc3339(),
        })
    }

    /// Execute a refund — splits escrow funds equally back to both players.
    pub async fn execute_refund(
        &self,
        match_id: &Uuid,
        player_a_address: &str,
        player_b_address: &str,
    ) -> Result<MultisigPayoutResult> {
        let escrow = {
            let escrows = self.escrows.lock().await;
            escrows
                .get(match_id)
                .cloned()
                .ok_or_else(|| anyhow!("Escrow not found for match {}", match_id))?
        };

        let utxos = self
            .rpc
            .get_utxos(&escrow.p2sh_address)
            .await
            .map_err(|e| anyhow!("Failed to get UTXOs: {}", e))?;

        let total_balance: u64 = utxos.iter().map(|u| u.amount).sum();

        let fee_estimate = self
            .rpc
            .get_fee_estimate()
            .await
            .map_err(|e| anyhow!("Fee estimate failed: {}", e))?;
        let network_fee = ((fee_estimate.normal_bucket_feerate * ESTIMATED_TX_MASS_GRAMS as f64)
            .ceil() as u64)
            .max(1000);

        let redeem_script = hex::decode(&escrow.redeem_script_hex)
            .map_err(|e| anyhow!("Invalid redeem script hex: {}", e))?;
        let p2sh_spk = redeem_script_to_p2sh(&redeem_script);

        let (tx, utxo_entries) = create_unsigned_refund_tx(
            &utxos,
            player_a_address,
            player_b_address,
            total_balance,
            network_fee,
            &p2sh_spk,
        )
        .map_err(|e| anyhow!("Failed to create refund TX: {}", e))?;

        let sighashes = compute_all_sighashes(&tx, utxo_entries);

        // Sign with any 2 keys (both players for consensus)
        let player_keys = self.player_keys.lock().await;
        let sk_a = player_keys
            .get(&escrow.pubkey_a_hex)
            .ok_or_else(|| anyhow!("Player A key not found"))?;

        let mut sigs_per_input = Vec::new();
        for sighash in &sighashes {
            let sig1 = sign_sighash(sighash, sk_a)
                .map_err(|e| anyhow!("Signing failed: {}", e))?;
            let sig2 = sign_sighash(sighash, &self.oracle_private_key)
                .map_err(|e| anyhow!("Signing failed: {}", e))?;
            sigs_per_input.push(vec![sig1, sig2]);
        }
        drop(player_keys);

        let signed_tx = assemble_signed_tx(tx, &sigs_per_input, &redeem_script)
            .map_err(|e| anyhow!("Failed to assemble TX: {}", e))?;

        let rpc_tx = to_rpc_transaction(signed_tx);
        let tx_id = self
            .rpc
            .submit_rpc_transaction(rpc_tx)
            .await
            .map_err(|e| anyhow!("Broadcast failed: {}", e))?;

        {
            let mut escrows = self.escrows.lock().await;
            if let Some(esc) = escrows.get_mut(match_id) {
                esc.status = EscrowStatus::Refunded;
            }
        }

        tracing::info!("💸 Multisig refund TX {} submitted for match {}", tx_id, match_id);

        let net = total_balance.saturating_sub(network_fee);
        Ok(MultisigPayoutResult {
            match_id: match_id.to_string(),
            tx_id,
            winner_address: format!("{} / {}", player_a_address, player_b_address),
            winner_amount_sompi: net,
            platform_fee_sompi: 0,
            network_fee_sompi: network_fee,
            timestamp: Utc::now().to_rfc3339(),
        })
    }

    /// Get the oracle's public key hex.
    pub fn oracle_pubkey_hex(&self) -> &str {
        &self.oracle_pubkey_hex
    }

    /// Get an escrow by match ID.
    pub async fn get_escrow(&self, match_id: &Uuid) -> Option<MultisigEscrow> {
        let escrows = self.escrows.lock().await;
        escrows.get(match_id).cloned()
    }

    // ─── Internal Helpers ────────────────────────────────────────────────────

    /// Derives a deterministic keypair for a player from match_id and role.
    fn derive_player_key(
        &self,
        match_id: &Uuid,
        role: &str,
    ) -> Result<(String, [u8; 32])> {
        use sha2::{Digest, Sha256};

        let input = format!("{}-{}-kaspabattle-multisig", match_id, role);
        let mut hasher = Sha256::new();
        hasher.update(input.as_bytes());
        let hash = hasher.finalize();

        let mut sk_bytes = [0u8; 32];
        sk_bytes.copy_from_slice(&hash);

        let secp = secp256k1::Secp256k1::new();
        let sk = secp256k1::SecretKey::from_slice(&sk_bytes)
            .map_err(|e| anyhow!("Failed to create player key: {}", e))?;
        let keypair = secp256k1::Keypair::from_secret_key(&secp, &sk);
        let (xonly, _) = keypair.x_only_public_key();
        let pk_hex = hex::encode(xonly.serialize());

        Ok((pk_hex, sk_bytes))
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mock::MockKaspaClient;
    use crate::rpc::UtxoInfo;

    fn make_service() -> (MultisigEscrowService, Arc<MockKaspaClient>) {
        let mock = Arc::new(MockKaspaClient::new());
        // Use a deterministic oracle key for testing
        let oracle_sk: [u8; 32] =
            hex::decode("e1e2e3e4e5e6e7e8e9e0f1f2f3f4f5f6f7f8f9f0a1a2a3a4a5a6a7a8a9a0b1b2")
                .unwrap()
                .try_into()
                .unwrap();

        let service = MultisigEscrowService::new(
            mock.clone() as Arc<dyn KaspaRpc>,
            Prefix::Testnet,
            oracle_sk,
            "kaspatest:qz7ks4hqswjj40zr58hxnhkdq75ky7f0kquq9ltyrdm5cpygkhfg5j8pf83l".to_string(),
        )
        .unwrap();

        (service, mock)
    }

    #[tokio::test]
    async fn test_create_escrow() {
        let (service, _mock) = make_service();
        let match_id = Uuid::new_v4();

        let info = service
            .create_escrow(match_id, 5_000_000, None)
            .await
            .unwrap();

        assert_eq!(info.match_id, match_id.to_string());
        assert!(
            info.escrow_address.starts_with("kaspatest:"),
            "Address should be testnet: {}",
            info.escrow_address
        );
        assert_eq!(info.threshold, 2);
        assert_eq!(info.pubkeys.len(), 3);
        assert!(!info.redeem_script_hex.is_empty());
        assert_eq!(info.wager_per_player_sompi, 5_000_000);
    }

    #[tokio::test]
    async fn test_create_escrow_deterministic() {
        let (service, _mock) = make_service();
        let match_id = Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap();

        let info1 = service
            .create_escrow(match_id, 5_000_000, None)
            .await
            .unwrap();

        // Re-create service with same oracle key
        let (service2, _mock2) = make_service();
        let info2 = service2
            .create_escrow(match_id, 5_000_000, None)
            .await
            .unwrap();

        assert_eq!(info1.escrow_address, info2.escrow_address);
        assert_eq!(info1.redeem_script_hex, info2.redeem_script_hex);
    }

    #[tokio::test]
    async fn test_create_escrow_unique_per_match() {
        let (service, _mock) = make_service();
        let match_id1 = Uuid::new_v4();
        let match_id2 = Uuid::new_v4();

        let info1 = service
            .create_escrow(match_id1, 5_000_000, None)
            .await
            .unwrap();
        let info2 = service
            .create_escrow(match_id2, 5_000_000, None)
            .await
            .unwrap();

        assert_ne!(info1.escrow_address, info2.escrow_address);
    }

    #[tokio::test]
    async fn test_check_deposits_none() {
        let (service, _mock) = make_service();
        let match_id = Uuid::new_v4();

        let info = service
            .create_escrow(match_id, 5_000_000, None)
            .await
            .unwrap();

        let status = service
            .check_deposits(&info.escrow_address, 5_000_000)
            .await
            .unwrap();

        assert_eq!(status.deposited_sompi, 0);
        assert!(!status.is_fully_funded);
    }

    #[tokio::test]
    async fn test_check_deposits_partial() {
        let (service, mock) = make_service();
        let match_id = Uuid::new_v4();

        let info = service
            .create_escrow(match_id, 5_000_000, None)
            .await
            .unwrap();

        mock.add_utxo(
            &info.escrow_address,
            UtxoInfo {
                tx_id: "aabb".to_string(),
                output_index: 0,
                amount: 5_000_000,
                amount_kas: 0.05,
                is_coinbase: false,
                block_daa_score: 100,
                script_public_key: None,
            },
        );

        let status = service
            .check_deposits(&info.escrow_address, 5_000_000)
            .await
            .unwrap();

        assert_eq!(status.deposited_sompi, 5_000_000);
        assert!(!status.is_fully_funded);
    }

    #[tokio::test]
    async fn test_check_deposits_complete() {
        let (service, mock) = make_service();
        let match_id = Uuid::new_v4();

        let info = service
            .create_escrow(match_id, 5_000_000, None)
            .await
            .unwrap();

        mock.add_utxo(
            &info.escrow_address,
            UtxoInfo {
                tx_id: "aabb".to_string(),
                output_index: 0,
                amount: 5_000_000,
                amount_kas: 0.05,
                is_coinbase: false,
                block_daa_score: 100,
                script_public_key: None,
            },
        );
        mock.add_utxo(
            &info.escrow_address,
            UtxoInfo {
                tx_id: "ccdd".to_string(),
                output_index: 0,
                amount: 5_000_000,
                amount_kas: 0.05,
                is_coinbase: false,
                block_daa_score: 101,
                script_public_key: None,
            },
        );

        let status = service
            .check_deposits(&info.escrow_address, 5_000_000)
            .await
            .unwrap();

        assert_eq!(status.deposited_sompi, 10_000_000);
        assert!(status.is_fully_funded);
    }

    #[tokio::test]
    async fn test_get_escrow() {
        let (service, _mock) = make_service();
        let match_id = Uuid::new_v4();

        let _info = service
            .create_escrow(match_id, 5_000_000, None)
            .await
            .unwrap();

        let escrow = service.get_escrow(&match_id).await;
        assert!(escrow.is_some());
        assert_eq!(escrow.unwrap().status, EscrowStatus::Created);
    }

    #[tokio::test]
    async fn test_get_escrow_not_found() {
        let (service, _mock) = make_service();
        let escrow = service.get_escrow(&Uuid::new_v4()).await;
        assert!(escrow.is_none());
    }

    #[tokio::test]
    async fn test_oracle_pubkey_consistent() {
        let (service, _) = make_service();
        assert!(!service.oracle_pubkey_hex().is_empty());
        assert_eq!(service.oracle_pubkey_hex().len(), 64); // 32 bytes hex
    }
}

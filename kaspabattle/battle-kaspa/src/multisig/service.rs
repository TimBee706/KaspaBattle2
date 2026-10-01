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
use crate::rpc::KaspaBackend;

use battle_core::types::SOMPI_PER_KAS;
use battle_core::constants::PLATFORM_FEE_PERCENT;

// Kaspa mass-based fee calculation imports
use kaspa_consensus_core::network::{NetworkId, NetworkType};
use kaspa_consensus_core::tx::TransactionId;
use kaspa_consensus_client::UtxoEntry as ClientUtxoEntry;
use kaspa_consensus_client::UtxoEntryReference;
use kaspa_consensus_client::TransactionOutpoint as ClientOutpoint;
use kaspa_wallet_core::tx::mass::MassCalculator;
use kaspa_wallet_core::utxo::NetworkParams;

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
    rpc: Arc<dyn KaspaBackend>,
    /// Network prefix (Mainnet or Testnet)
    prefix: Prefix,
    /// Platform/Oracle private key (32 bytes)
    oracle_private_key: [u8; 32],
    /// Oracle x-only public key hex
    oracle_pubkey_hex: String,
    /// Platform treasury address for fee collection
    treasury_address: String,
    /// Server-side secret used to key the player-key derivation (SEC-MULTISIG-01).
    ///
    /// **Must never be derivable from public data.** Player keys are derived as
    /// `HMAC-SHA256(key_derivation_secret, match_id || role)`, so knowledge of the secret
    /// is required in addition to the (public) match ID. See `derive_player_key`.
    key_derivation_secret: [u8; 32],
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
    /// * `key_derivation_secret` - 32-byte server secret for player-key derivation
    ///   (SEC-MULTISIG-01). Must be kept confidential — anyone who knows it, together
    ///   with a match ID, can reconstruct both players' escrow keys.
    pub fn new(
        rpc: Arc<dyn KaspaBackend>,
        prefix: Prefix,
        oracle_private_key: [u8; 32],
        treasury_address: String,
        key_derivation_secret: [u8; 32],
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
            key_derivation_secret,
            player_keys: Arc::new(Mutex::new(HashMap::new())),
            escrows: Arc::new(Mutex::new(HashMap::new())),
        })
    }

    /// Builds the public info view of a stored escrow record.
    fn escrow_info(escrow: &MultisigEscrow) -> MultisigEscrowInfo {
        MultisigEscrowInfo {
            match_id: escrow.match_id.to_string(),
            escrow_address: escrow.p2sh_address.clone(),
            redeem_script_hex: escrow.redeem_script_hex.clone(),
            pubkeys: vec![
                escrow.pubkey_a_hex.clone(),
                escrow.pubkey_b_hex.clone(),
                escrow.pubkey_oracle_hex.clone(),
            ],
            threshold: 2,
            wager_per_player_sompi: escrow.wager_per_player_sompi,
            wager_per_player_kas: escrow.wager_per_player_sompi as f64 / SOMPI_PER_KAS as f64,
            timelock_timestamp: escrow.timelock_timestamp,
        }
    }

    /// Create a new 2-of-3 multisig escrow for a match.
    ///
    /// Idempotent: if an escrow already exists for `match_id` it is returned unchanged
    /// and the passed wager/timelock are ignored.
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
        // The payout/refund paths compute `wager * 2`; reject amounts that could overflow
        // (a wrapped product would defeat the "escrow is fully funded" balance check).
        if wager_per_player_sompi == 0 || wager_per_player_sompi.checked_mul(2).is_none() {
            return Err(anyhow!(
                "Invalid wager: {} sompi (must be > 0 and fit twice into u64)",
                wager_per_player_sompi
            ));
        }

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

        // Store escrow. get-or-create: never replace an existing record, otherwise a repeated
        // call with different wager/timelock values would silently rewrite the terms of a
        // live escrow (payout/refund read `wager_per_player_sompi` from this record).
        {
            let mut escrows = self.escrows.lock().await;
            if let Some(existing) = escrows.get(&match_id) {
                tracing::warn!(
                    "Escrow for match {} already exists — returning existing record unchanged",
                    match_id
                );
                return Ok(Self::escrow_info(existing));
            }
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
        // saturating: an (impossible-by-construction) overflow must fail closed, not wrap to a tiny value
        let expected = escrow.wager_per_player_sompi.saturating_mul(2);
        if total_balance < expected {
            return Err(anyhow!(
                "Insufficient escrow balance: have {} sompi, need {}",
                total_balance,
                expected
            ));
        }

        // Build redeem script and P2SH for signing
        let redeem_script = hex::decode(&escrow.redeem_script_hex)
            .map_err(|e| anyhow!("Invalid redeem script hex: {}", e))?;
        let p2sh_spk = redeem_script_to_p2sh(&redeem_script);

        // --- Kaspa Mass-based Fee Calculation (v2) ---
        // Stage 1: Build provisional TX (fee=0) to measure real TX mass.
        // We use provisional split amounts to get accurate output structure.
        let provisional_platform_fee = total_balance * PLATFORM_FEE_PERCENT / 100;
        let provisional_winner_amount = total_balance - provisional_platform_fee;
        let (tmp_tx, _) = create_unsigned_payout_tx(
            &utxos,
            winner_address,
            provisional_winner_amount,
            &self.treasury_address,
            provisional_platform_fee,
            &p2sh_spk,
        )
        .map_err(|e| anyhow!("Failed to create provisional payout TX: {}", e))?;

        // Stage 2: Compute real fee from actual TX mass + node fee-rate.
        let network_fee = self
            .compute_network_fee_for_tx(&tmp_tx, &utxos, &p2sh_spk, 2)
            .await
            .unwrap_or_else(|e| {
                tracing::warn!("[FALLBACK] Fee calculation failed ({}), using conservative 5000 sompi", e);
                5000
            });

        // Stage 3: Final split with correct fee.
        let net_pot = total_balance.saturating_sub(network_fee);
        let platform_fee = net_pot * PLATFORM_FEE_PERCENT / 100;
        let winner_amount = net_pot - platform_fee;

        tracing::info!(
            match_id = %match_id,
            total_balance,
            utxo_count = utxos.len(),
            network_fee,
            platform_fee,
            winner_amount,
            "Payout TX fees computed (mass-based)"
        );

        // Stage 4: Build final unsigned TX with correct amounts.
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

        let rpc_tx = to_rpc_transaction(signed_tx);
        let payload = serde_json::to_string(&rpc_tx)
            .map_err(|e| anyhow!("Serialization failed: {}", e))?;
        let tx_id = self
            .rpc
            .submit_transaction(&payload)
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
    ///
    /// Guards:
    /// - **Sync check**: Verifies the Kaspa node is synced before querying UTXOs.
    /// - **Zero balance**: If escrow has no funds, returns immediately without
    ///   broadcasting a transaction (idempotent for already-refunded escrows).
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

        // Verify node is synced (consistent with execute_payout)
        let synced = self
            .rpc
            .is_synced()
            .await
            .map_err(|e| anyhow!("RPC error checking sync: {}", e))?;
        if !synced {
            return Err(anyhow!("Kaspa node is not synced — cannot execute refund"));
        }

        let utxos = self
            .rpc
            .get_utxos(&escrow.p2sh_address)
            .await
            .map_err(|e| anyhow!("Failed to get UTXOs: {}", e))?;

        let total_balance: u64 = utxos.iter().map(|u| u.amount).sum();

        // Zero-balance guard: nothing to refund (escrow already emptied or never funded)
        if total_balance == 0 {
            tracing::info!(
                "💸 Refund for match {}: escrow has 0 balance — marking as refunded (no TX needed)",
                match_id
            );
            {
                let mut escrows = self.escrows.lock().await;
                if let Some(esc) = escrows.get_mut(match_id) {
                    esc.status = EscrowStatus::Refunded;
                }
            }
            return Ok(MultisigPayoutResult {
                match_id: match_id.to_string(),
                tx_id: String::new(),
                winner_address: format!("{} / {}", player_a_address, player_b_address),
                winner_amount_sompi: 0,
                platform_fee_sompi: 0,
                network_fee_sompi: 0,
                timestamp: Utc::now().to_rfc3339(),
            });
        }

        let redeem_script = hex::decode(&escrow.redeem_script_hex)
            .map_err(|e| anyhow!("Invalid redeem script hex: {}", e))?;
        let p2sh_spk = redeem_script_to_p2sh(&redeem_script);

        // --- Kaspa Mass-based Fee Calculation (v2) ---
        // Stage 1: Provisional refund TX with fee=0 to measure real mass.
        let (tmp_tx, _) = create_unsigned_refund_tx(
            &utxos,
            player_a_address,
            player_b_address,
            total_balance,
            0, // fee=0 for mass measurement only
            &p2sh_spk,
        )
        .map_err(|e| anyhow!("Failed to create provisional refund TX: {}", e))?;

        // Stage 2: Compute real fee from TX mass + node fee-rate.
        let network_fee = self
            .compute_network_fee_for_tx(&tmp_tx, &utxos, &p2sh_spk, 2)
            .await
            .unwrap_or_else(|e| {
                tracing::warn!("[FALLBACK] Refund fee calc failed ({}), using conservative 5000 sompi", e);
                5000
            });

        tracing::info!(
            match_id = %match_id,
            total_balance,
            utxo_count = utxos.len(),
            network_fee,
            "Refund TX fees computed (mass-based)"
        );

        // Stage 3: Build final refund TX with correct fee.
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
        let payload = serde_json::to_string(&rpc_tx)
            .map_err(|e| anyhow!("Serialization failed: {}", e))?;
        let tx_id = self
            .rpc
            .submit_transaction(&payload)
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

    /// Create a Partially Signed Kaspa Transaction (PSKT) for the winner payout.
    ///
    /// Signs each input **only with the Oracle key**. The winner must add their
    /// own signature via the `submit-signature` endpoint to complete the 2-of-3
    /// threshold.
    ///
    /// Returns the assembled PSKT as a hex-encoded byte string.
    ///
    /// ## Layout
    ///
    /// The hex string encodes:
    ///   `{oracle_sig_len_u32_le}{oracle_sig_bytes}{unsigned_tx_bytes}`
    ///
    /// The winner's client decodes this, adds their signature at the same
    /// script position, and submits the raw signed TX via `submit_rpc_transaction`.
    ///
    /// For the backend-held-keys MVP, we store the oracle sig + raw unsigned TX in the
    /// simplest possible format — a two-part hex string separated by `||`.
    ///
    /// Format: `{oracle_sigs_hex}||{tx_bytes_hex}||{redeem_script_hex}||{fee_info_hex}`
    ///
    /// where `oracle_sigs_hex` is `serde_json` of `Vec<String>` (hex per input).
    pub async fn create_pskt(
        &self,
        match_id: &Uuid,
        winner_address: &str,
    ) -> Result<PsktResult> {
        // ── Load escrow ────────────────────────────────────────────────────
        let escrow = {
            let escrows = self.escrows.lock().await;
            escrows
                .get(match_id)
                .cloned()
                .ok_or_else(|| anyhow!("Escrow not found for match {} — may need recovery from DB", match_id))?
        };

        // ── RPC: check node sync ───────────────────────────────────────────
        let synced = self
            .rpc
            .is_synced()
            .await
            .map_err(|e| anyhow!("RPC error checking sync: {}", e))?;
        if !synced {
            return Err(anyhow!("Kaspa node is not synced — cannot create PSKT"));
        }

        // ── Fetch UTXOs ────────────────────────────────────────────────────
        let utxos = self
            .rpc
            .get_utxos(&escrow.p2sh_address)
            .await
            .map_err(|e| anyhow!("Failed to get UTXOs for escrow: {}", e))?;

        let total_balance: u64 = utxos.iter().map(|u| u.amount).sum();
        // saturating: an (impossible-by-construction) overflow must fail closed, not wrap to a tiny value
        let expected = escrow.wager_per_player_sompi.saturating_mul(2);
        if total_balance < expected {
            return Err(anyhow!(
                "Insufficient escrow balance for PSKT: have {} sompi, need {}",
                total_balance,
                expected
            ));
        }

        // ── Fee calculation (Kaspa Mass-based, v2) ────────────────────────
        // Stage 1: Build provisional TX to measure real TX mass.
        let redeem_script = hex::decode(&escrow.redeem_script_hex)
            .map_err(|e| anyhow!("Invalid redeem script hex: {}", e))?;
        let p2sh_spk = crate::multisig::scripts::redeem_script_to_p2sh(&redeem_script);

        let provisional_platform_fee = total_balance * PLATFORM_FEE_PERCENT / 100;
        let provisional_winner_amount = total_balance - provisional_platform_fee;
        let (tmp_tx, _) = create_unsigned_payout_tx(
            &utxos,
            winner_address,
            provisional_winner_amount,
            &self.treasury_address,
            provisional_platform_fee,
            &p2sh_spk,
        )
        .map_err(|e| anyhow!("Failed to create provisional PSKT TX: {}", e))?;

        // Stage 2: Compute real fee from TX mass + node fee-rate.
        let network_fee = self
            .compute_network_fee_for_tx(&tmp_tx, &utxos, &p2sh_spk, 2)
            .await
            .unwrap_or_else(|e| {
                tracing::warn!("[FALLBACK] PSKT fee calc failed ({}), using conservative 5000 sompi", e);
                5000
            });

        // Stage 3: Final amounts.
        let net_pot = total_balance.saturating_sub(network_fee);
        let platform_fee = net_pot * PLATFORM_FEE_PERCENT / 100;
        let winner_amount = net_pot - platform_fee;

        tracing::info!(
            match_id = %match_id,
            total_balance,
            utxo_count = utxos.len(),
            network_fee,
            platform_fee,
            winner_amount,
            winner_address,
            "Creating PSKT for payout (mass-based fee)"
        );

        // ── Build final unsigned TX ────────────────────────────────────────
        let (tx, utxo_entries) = create_unsigned_payout_tx(
            &utxos,
            winner_address,
            winner_amount,
            &self.treasury_address,
            platform_fee,
            &p2sh_spk,
        )
        .map_err(|e| anyhow!("Failed to create unsigned TX: {}", e))?;

        // ── Compute sighashes ──────────────────────────────────────────────
        let sighashes = compute_all_sighashes(&tx, utxo_entries);

        // ── Sign with Oracle key only ──────────────────────────────────────
        // In the non-custodial PSKT model:
        //   - Oracle signs here (1 of 2 required)
        //   - Winner adds their signature client-side
        // In the custodial MVP: we sign with both player_a + oracle here and
        // call it "fully signed" (same as execute_payout).
        // The PSKT hex encodes the oracle sigs so the submit-signature endpoint
        // can add the winner's sig and assemble the final TX.
        let mut oracle_sigs: Vec<String> = Vec::new();
        for sighash in &sighashes {
            let sig = sign_sighash(sighash, &self.oracle_private_key)
                .map_err(|e| anyhow!("Oracle signing failed: {}", e))?;
            oracle_sigs.push(hex::encode(&sig));
        }

        // ── Serialize TX to bytes for PSKT ────────────────────────────────
        // We use a simple JSON envelope so the client can reconstruct everything.
        let pskt_payload = serde_json::json!({
            "match_id": match_id.to_string(),
            "winner_address": winner_address,
            "winner_amount_sompi": winner_amount,
            "platform_fee_sompi": platform_fee,
            "network_fee_sompi": network_fee,
            "escrow_address": escrow.p2sh_address,
            "redeem_script_hex": escrow.redeem_script_hex,
            "oracle_sigs": oracle_sigs,
            "input_count": sighashes.len(),
            // Note: raw TX bytes would go here in a full PSKT implementation.
            // For the backend-held-keys MVP, execute_payout handles the final broadcast.
            "pskt_version": "1.0-backend-held",
        });
        let pskt_hex = hex::encode(pskt_payload.to_string().as_bytes());

        tracing::info!(
            match_id = %match_id,
            winner_amount,
            "✅ PSKT created (oracle-signed)"
        );

        Ok(PsktResult {
            match_id: match_id.to_string(),
            pskt_hex,
            winner_address: winner_address.to_string(),
            winner_amount_sompi: winner_amount,
            platform_fee_sompi: platform_fee,
            network_fee_sompi: network_fee,
        })
    }

    /// Restore an escrow from the DB into the in-memory HashMap.
    ///
    /// Call this at server startup to recover crashes:
    /// ```sql
    /// SELECT * FROM multisig_escrows WHERE status != 'SETTLED'
    /// ```
    #[allow(clippy::too_many_arguments)]
    pub async fn restore_escrow_from_row(
        &self,
        match_id: Uuid,
        pubkey_a_hex: String,
        pubkey_b_hex: String,
        pubkey_oracle_hex: String,
        redeem_script_hex: String,
        p2sh_address: String,
        wager_per_player_sompi: u64,
        timelock_timestamp: Option<u64>,
    ) {
        let escrow = MultisigEscrow {
            match_id,
            pubkey_a_hex: pubkey_a_hex.clone(),
            pubkey_b_hex: pubkey_b_hex.clone(),
            pubkey_oracle_hex,
            config: MultisigConfig::v1(),
            redeem_script_hex,
            p2sh_address,
            wager_per_player_sompi,
            status: EscrowStatus::Funded, // conservative default
            timelock_timestamp,
            created_at: Utc::now(),
        };

        // Restore player keys (deterministic — same derivation as create_escrow)
        if let Ok((_, sk_a)) = self.derive_player_key(&match_id, "player_a") {
            if let Ok((_, sk_b)) = self.derive_player_key(&match_id, "player_b") {
                let mut keys = self.player_keys.lock().await;
                keys.insert(pubkey_a_hex, sk_a);
                keys.insert(pubkey_b_hex, sk_b);
            }
        }
        let mut escrows = self.escrows.lock().await;
        escrows.insert(match_id, escrow);
    }

    // ─── Internal Helpers ────────────────────────────────────────────────────

    /// Derives a deterministic keypair for a player from match_id, role, and the
    /// server-side `key_derivation_secret`.
    ///
    /// # Security — SEC-MULTISIG-01 (fixed 2026-09-29)
    ///
    /// The original implementation derived player keys as
    /// `SHA256("{match_id}-{role}-kaspabattle-multisig")` — a function of **public**
    /// data only (match IDs are exposed via `GET /lobbies` and `GET /matches/:id`
    /// without authentication). That meant *anyone* could reconstruct both player
    /// keys for any match and satisfy the "2-of-3" threshold alone, without the
    /// Oracle key and without ever compromising the server — a critical,
    /// server-independent theft primitive, not just a custodial-trust concern.
    ///
    /// The fix keys the derivation with `HMAC-SHA256(key_derivation_secret, ...)`
    /// where `key_derivation_secret` is a 32-byte secret held only by the server
    /// (`MULTISIG_KEY_DERIVATION_SECRET`). Derivation remains deterministic across
    /// restarts (needed by `restore_escrow_from_row`, since player keys are not
    /// persisted to the DB) but now additionally requires knowledge of that secret.
    ///
    /// This does **not** make the model non-custodial — the server still holds
    /// (derives) all three key roles and could misbehave — but it removes the
    /// public/anyone-can-derive flaw. Full decentralization is deferred to the
    /// SilverScript L1 covenant migration (see `docs/LEARNINGS.md`).
    fn derive_player_key(
        &self,
        match_id: &Uuid,
        role: &str,
    ) -> Result<(String, [u8; 32])> {
        use hmac::{Hmac, Mac};
        use sha2::Sha256;

        type HmacSha256 = Hmac<Sha256>;

        let input = format!("{}-{}-kaspabattle-multisig-v2", match_id, role);
        let mut mac = HmacSha256::new_from_slice(&self.key_derivation_secret)
            .map_err(|e| anyhow!("Invalid key derivation secret: {}", e))?;
        mac.update(input.as_bytes());
        let hash = mac.finalize().into_bytes();

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

    /// Computes the network fee for an unsigned transaction based on its actual mass.
    ///
    /// # Kaspa Mass-based Fee Calculation
    ///
    /// Uses `MassCalculator` from `kaspa-wallet-core` to determine the exact
    /// compute + storage mass of the unsigned transaction, then derives the fee
    /// from the node's current fee-rate. This ensures fees are never rejected
    /// as "fees under required amount" by the Kaspa node.
    ///
    /// # Arguments
    /// * `tx` - The unsigned consensus transaction (inputs must be set, signatures empty)
    /// * `utxos` - UTXOs being spent (needed for storage mass calculation)
    /// * `p2sh_spk` - The escrow P2SH script public key (applied to all UTXO entries)
    /// * `minimum_signatures` - Number of required signatures (2 for 2-of-3 multisig)
    async fn compute_network_fee_for_tx(
        &self,
        tx: &kaspa_consensus_core::tx::Transaction,
        utxos: &[crate::rpc::UtxoInfo],
        p2sh_spk: &kaspa_consensus_core::tx::ScriptPublicKey,
        minimum_signatures: u16,
    ) -> anyhow::Result<u64> {
        // 1. Get current fee rate from the node.
        let fee_estimate = self
            .rpc
            .get_fee_estimate()
            .await
            .map_err(|e| anyhow!("Fee estimate RPC failed: {}", e))?;
        let fee_rate = fee_estimate.normal_bucket_feerate;

        // 2. Map prefix → NetworkId (NetworkParams::from panics for bare Testnet without suffix).
        let network_id = match self.prefix {
            Prefix::Mainnet => NetworkId::new(NetworkType::Mainnet),
            _ => NetworkId::with_suffix(NetworkType::Testnet, 10), // Default: Testnet-10
        };

        // 3. Build MassCalculator with network-specific params (same pattern as wallet generator).
        let network_params = NetworkParams::from(network_id);
        let mass_calc = MassCalculator::new(&network_id.into(), network_params);

        // 4. Build UtxoEntryReference list from our UtxoInfo + escrow script public key.
        let utxo_refs: Vec<UtxoEntryReference> = utxos
            .iter()
            .map(|u| {
                let tx_id_bytes = hex::decode(&u.tx_id).unwrap_or_else(|_| vec![0u8; 32]);
                let tx_id = TransactionId::from_slice(&tx_id_bytes);
                let outpoint = ClientOutpoint::new(tx_id, u.output_index);
                let entry = ClientUtxoEntry {
                    address: None,
                    outpoint,
                    amount: u.amount,
                    script_public_key: p2sh_spk.clone(),
                    block_daa_score: u.block_daa_score,
                    is_coinbase: u.is_coinbase,
                };
                UtxoEntryReference::from(entry)
            })
            .collect();

        // 5. Calculate overall mass (compute mass + storage mass combined per KIP-9).
        let mass_unsigned = mass_calc
            .calc_overall_mass_for_unsigned_consensus_transaction(tx, &utxo_refs, minimum_signatures)
            .map_err(|e| anyhow!("Mass calculation failed: {}", e))?;

        // Add 10% overhead to account for signature script bytes not present in the
        // unsigned provisional TX. Each P2SH sig script adds 66 bytes per signature
        // + redeem script length, increasing storage mass beyond the provisional estimate.
        let mass = ((mass_unsigned as f64) * 1.1).ceil() as u64;

        // 6. Derive fee: take the maximum of minimum relay fee and feerate-based fee.
        let min_fee = mass_calc.calc_minimum_transaction_fee_from_mass(mass);
        let feerate_fee = (fee_rate * mass as f64).ceil() as u64;
        let fee = min_fee.max(feerate_fee).max(1000);

        tracing::debug!(
            mass_unsigned,
            mass,
            fee_rate,
            min_fee,
            feerate_fee,
            fee,
            "Mass-based fee computed"
        );

        Ok(fee)
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mock::MockKaspaClient;
    use crate::rpc::UtxoInfo;

    /// Fixed test-only key-derivation secret. NEVER use a hardcoded secret like
    /// this outside of tests — see `MultisigEscrowService::key_derivation_secret`.
    const TEST_KEY_DERIVATION_SECRET: [u8; 32] = [0x42; 32];

    fn make_service() -> (MultisigEscrowService, Arc<MockKaspaClient>) {
        make_service_with_secret(TEST_KEY_DERIVATION_SECRET)
    }

    fn make_service_with_secret(
        key_derivation_secret: [u8; 32],
    ) -> (MultisigEscrowService, Arc<MockKaspaClient>) {
        let mock = Arc::new(MockKaspaClient::new());
        // Use a deterministic oracle key for testing
        let oracle_sk: [u8; 32] =
            hex::decode("e1e2e3e4e5e6e7e8e9e0f1f2f3f4f5f6f7f8f9f0a1a2a3a4a5a6a7a8a9a0b1b2")
                .unwrap()
                .try_into()
                .unwrap();

        // Valid kaspatest treasury address (derived from known keypair)
        use kaspa_addresses::{Address, Version};
        let treasury_sk: [u8; 32] =
            hex::decode("5f6e7a8b9c0d1e2f3a4b5c6d7e8f9a0b1c2d3e4f5061728394a5b6c7d8e9f001")
                .unwrap()
                .try_into()
                .unwrap();
        let secp = secp256k1::Secp256k1::new();
        let sk = secp256k1::SecretKey::from_slice(&treasury_sk).unwrap();
        let keypair = secp256k1::Keypair::from_secret_key(&secp, &sk);
        let (xonly, _) = keypair.x_only_public_key();
        let treasury_addr =
            Address::new(Prefix::Testnet, Version::PubKey, &xonly.serialize()).to_string();

        let service = MultisigEscrowService::new(
            mock.clone() as Arc<dyn KaspaBackend>,
            Prefix::Testnet,
            oracle_sk,
            treasury_addr,
            key_derivation_secret,
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

    /// AUDIT F-01: a second create_escrow call for an existing match must not rewrite
    /// the stored wager/timelock (previously `escrows.insert` overwrote the record).
    #[tokio::test]
    async fn test_create_escrow_is_idempotent_and_does_not_overwrite() {
        let (service, _mock) = make_service();
        let match_id = Uuid::new_v4();

        let first = service.create_escrow(match_id, 5_000_000, None).await.unwrap();
        let second = service
            .create_escrow(match_id, 1, Some(42))
            .await
            .unwrap();

        assert_eq!(second.wager_per_player_sompi, 5_000_000);
        assert_eq!(second.timelock_timestamp, None);
        assert_eq!(second.escrow_address, first.escrow_address);
        let stored = service.get_escrow(&match_id).await.unwrap();
        assert_eq!(stored.wager_per_player_sompi, 5_000_000);
        assert_eq!(stored.timelock_timestamp, None);
    }

    /// AUDIT F-02: wagers whose double overflows u64 (or zero) must be rejected.
    #[tokio::test]
    async fn test_create_escrow_rejects_zero_and_overflowing_wager() {
        let (service, _mock) = make_service();
        for bad in [0u64, u64::MAX / 2 + 1, u64::MAX] {
            let res = service.create_escrow(Uuid::new_v4(), bad, None).await;
            assert!(res.is_err(), "wager {} must be rejected", bad);
        }
        // Boundary: largest value whose double still fits is accepted.
        assert!(service
            .create_escrow(Uuid::new_v4(), u64::MAX / 2, None)
            .await
            .is_ok());
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

    /// SEC-MULTISIG-01 regression test.
    ///
    /// Player-key derivation must depend on the server-side `key_derivation_secret`,
    /// not just on the (publicly known) match ID and role. This proves an attacker
    /// who only knows the match ID — but not the secret — cannot reconstruct the
    /// escrow address or redeem script, closing the "anyone can derive both player
    /// keys" theft primitive described on `derive_player_key`.
    #[tokio::test]
    async fn test_player_keys_require_derivation_secret() {
        let match_id = Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap();

        let (service_a, _mock_a) = make_service_with_secret([0x11; 32]);
        let (service_b, _mock_b) = make_service_with_secret([0x22; 32]);

        let info_a = service_a
            .create_escrow(match_id, 5_000_000, None)
            .await
            .unwrap();
        let info_b = service_b
            .create_escrow(match_id, 5_000_000, None)
            .await
            .unwrap();

        // Same match ID, same oracle key, different derivation secret →
        // different player keys → different redeem script / escrow address.
        assert_ne!(
            info_a.escrow_address, info_b.escrow_address,
            "escrow address must depend on the server secret, not just the public match_id"
        );
        assert_ne!(info_a.redeem_script_hex, info_b.redeem_script_hex);
        assert_ne!(
            info_a.pubkeys[0], info_b.pubkeys[0],
            "player A key must differ when the derivation secret differs"
        );
        assert_ne!(
            info_a.pubkeys[1], info_b.pubkeys[1],
            "player B key must differ when the derivation secret differs"
        );
        // The oracle key (3rd pubkey) is independent of the derivation secret and
        // must stay identical.
        assert_eq!(info_a.pubkeys[2], info_b.pubkeys[2]);

        // Sanity check against the OLD (vulnerable) formula: SHA256(match_id-role-kaspabattle-multisig)
        // must NOT equal either player's derived key — i.e. the fix actually changed the scheme,
        // it isn't accidentally reproducing the old, publicly-derivable keys.
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(format!("{}-player_a-kaspabattle-multisig", match_id).as_bytes());
        let old_sk_a: [u8; 32] = hasher.finalize().into();
        let secp = secp256k1::Secp256k1::new();
        let old_pk_a_hex = secp256k1::SecretKey::from_slice(&old_sk_a)
            .ok()
            .map(|sk| {
                let kp = secp256k1::Keypair::from_secret_key(&secp, &sk);
                hex::encode(kp.x_only_public_key().0.serialize())
            });
        if let Some(old_pk_a_hex) = old_pk_a_hex {
            assert_ne!(
                info_a.pubkeys[0], old_pk_a_hex,
                "derived key must not match the old, publicly-derivable SHA256-only scheme"
            );
        }
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

    // ─── Mass-based Fee Calculation Tests ────────────────────────────────────

    /// Build a minimal UtxoInfo for use in fee tests.
    fn make_utxo_info(tx_id: &str, amount: u64) -> UtxoInfo {
        UtxoInfo {
            tx_id: tx_id.to_string(),
            output_index: 0,
            amount,
            amount_kas: amount as f64 / 1e8,
            is_coinbase: false,
            block_daa_score: 100,
            script_public_key: None,
        }
    }

    /// Returns a valid kaspatest P2PK address for use in tests.
    fn test_kaspatest_address() -> String {
        use kaspa_addresses::{Address, Version};
        // Deterministic key: sha256 of "kaspabattle-test"
        let sk_bytes: [u8; 32] =
            hex::decode("1d99c236b1f37b3b845336e6c568ba37e9ced4769d83b7a096eec446b940d160")
                .unwrap()
                .try_into()
                .unwrap();
        let secp = secp256k1::Secp256k1::new();
        let sk = secp256k1::SecretKey::from_slice(&sk_bytes).unwrap();
        let keypair = secp256k1::Keypair::from_secret_key(&secp, &sk);
        let (xonly, _) = keypair.x_only_public_key();
        let addr = Address::new(Prefix::Testnet, Version::PubKey, &xonly.serialize());
        addr.to_string()
    }

    /// Returns a second valid kaspatest address for player B.
    fn test_kaspatest_address_b() -> String {
        use kaspa_addresses::{Address, Version};
        let sk_bytes: [u8; 32] =
            hex::decode("349ca0c824948fed8c2c568ce205e9d9be4468ef099cad76e3e5ec918954aca4")
                .unwrap()
                .try_into()
                .unwrap();
        let secp = secp256k1::Secp256k1::new();
        let sk = secp256k1::SecretKey::from_slice(&sk_bytes).unwrap();
        let keypair = secp256k1::Keypair::from_secret_key(&secp, &sk);
        let (xonly, _) = keypair.x_only_public_key();
        let addr = Address::new(Prefix::Testnet, Version::PubKey, &xonly.serialize());
        addr.to_string()
    }

    #[tokio::test]
    async fn test_compute_network_fee_for_tx_returns_nonzero() {
        use crate::multisig::scripts::redeem_script_to_p2sh;
        use crate::multisig::transaction::create_unsigned_refund_tx;

        let (service, _mock) = make_service();
        let match_id = uuid::Uuid::new_v4();
        let info = service.create_escrow(match_id, 5_000_000, None).await.unwrap();

        let redeem_script = hex::decode(&info.redeem_script_hex).unwrap();
        let p2sh_spk = redeem_script_to_p2sh(&redeem_script);

        let utxos = vec![
            make_utxo_info(
                "63020db736215f8b1105a9281f7bcbb6473d965ecc45bb2fb5da59bd35e6ff84",
                5_000_000,
            ),
            make_utxo_info(
                "73020db736215f8b1105a9281f7bcbb6473d965ecc45bb2fb5da59bd35e6ff84",
                5_000_000,
            ),
        ];

        let addr_a = test_kaspatest_address();
        let addr_b = test_kaspatest_address_b();
        let (tmp_tx, _) = create_unsigned_refund_tx(
            &utxos, &addr_a, &addr_b, 10_000_000, 0, &p2sh_spk,
        )
        .unwrap();

        let fee = service
            .compute_network_fee_for_tx(&tmp_tx, &utxos, &p2sh_spk, 2)
            .await
            .expect("Fee calculation should succeed");

        assert!(fee >= 1000, "Fee should be at least 1000 sompi, got {}", fee);
        tracing::info!("Computed fee: {} sompi (old ESTIMATED_TX_MASS_GRAMS=3000 approach was ~3000)", fee);
    }

    #[tokio::test]
    async fn test_execute_refund_with_mock_uses_mass_based_fee() {
        let (service, mock) = make_service();
        let match_id = uuid::Uuid::new_v4();

        let info = service.create_escrow(match_id, 5_000_000, None).await.unwrap();

        mock.add_utxo(
            &info.escrow_address,
            UtxoInfo {
                tx_id: "aabbccdd00000000000000000000000000000000000000000000000000000000"
                    .to_string(),
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
                tx_id: "eeff001122000000000000000000000000000000000000000000000000000000"
                    .to_string(),
                output_index: 0,
                amount: 5_000_000,
                amount_kas: 0.05,
                is_coinbase: false,
                block_daa_score: 101,
                script_public_key: None,
            },
        );

        let player_a = test_kaspatest_address();
        let player_b = test_kaspatest_address_b();

        let result = service
            .execute_refund(&match_id, &player_a, &player_b)
            .await
            .expect("Refund should succeed");

        assert!(!result.tx_id.is_empty(), "TX ID should not be empty");
        assert!(
            result.network_fee_sompi >= 1000,
            "Fee should be at least 1000 sompi, got {}",
            result.network_fee_sompi
        );
        assert_eq!(result.platform_fee_sompi, 0, "Refund has no platform fee");

        let submitted = mock.get_submitted_tx_ids();
        assert_eq!(submitted.len(), 1, "Exactly one TX should be submitted");
    }

    #[tokio::test]
    async fn test_execute_payout_with_mock_uses_mass_based_fee() {
        let (service, mock) = make_service();
        let match_id = uuid::Uuid::new_v4();

        let info = service.create_escrow(match_id, 5_000_000, None).await.unwrap();

        mock.add_utxo(
            &info.escrow_address,
            UtxoInfo {
                tx_id: "1100000000000000000000000000000000000000000000000000000000000011"
                    .to_string(),
                output_index: 0,
                amount: 5_000_000,
                amount_kas: 0.05,
                is_coinbase: false,
                block_daa_score: 200,
                script_public_key: None,
            },
        );
        mock.add_utxo(
            &info.escrow_address,
            UtxoInfo {
                tx_id: "2200000000000000000000000000000000000000000000000000000000000022"
                    .to_string(),
                output_index: 0,
                amount: 5_000_000,
                amount_kas: 0.05,
                is_coinbase: false,
                block_daa_score: 201,
                script_public_key: None,
            },
        );

        let winner = test_kaspatest_address();

        let result = service
            .execute_payout(&match_id, &winner)
            .await
            .expect("Payout should succeed");

        assert!(!result.tx_id.is_empty());
        assert!(
            result.network_fee_sompi >= 1000,
            "Fee should be >= 1000 sompi, got {}",
            result.network_fee_sompi
        );
        assert!(result.winner_amount_sompi > 0, "Winner should receive something");

        let submitted = mock.get_submitted_tx_ids();
        assert_eq!(submitted.len(), 1);
    }
}

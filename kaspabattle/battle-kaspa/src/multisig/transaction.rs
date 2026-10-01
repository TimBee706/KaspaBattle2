//! Transaction building and signing for multisig escrows.
//!
//! Handles unsigned TX creation, sighash computation, Schnorr signing,
//! and final TX assembly with signature scripts.

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
use kaspa_hashes::Hash;
use kaspa_rpc_core::model::tx::{
    RpcTransaction, RpcTransactionInput, RpcTransactionOutpoint, RpcTransactionOutput,
};
use kaspa_txscript::pay_to_address_script;

use crate::multisig::scripts::{build_multisig_sig_script, format_schnorr_sig, ScriptError};
use crate::rpc::UtxoInfo;

use thiserror::Error;

#[derive(Error, Debug)]
pub enum TxError {
    #[error("No UTXOs available for escrow address")]
    NoUtxos,

    #[error("Invalid address: {0}")]
    InvalidAddress(String),

    #[error("Invalid transaction ID hex: {0}")]
    InvalidTxId(String),

    #[error("Signing failed: {0}")]
    SigningFailed(String),

    #[error("Script error: {0}")]
    ScriptError(#[from] ScriptError),

    #[error("Insufficient signatures: have {have}, need {need}")]
    InsufficientSignatures { have: usize, need: usize },
}

/// Creates an unsigned transaction spending the escrow UTXOs.
///
/// # Arguments
/// * `utxos` - UTXOs at the escrow P2SH address
/// * `winner_address` - Address to receive the winner payout
/// * `winner_amount` - Sompi to send to winner
/// * `platform_address` - Address for platform fee
/// * `platform_fee` - Sompi to send as platform fee
///
/// # Returns
/// `(Transaction, Vec<UtxoEntry>)` — the unsigned TX and the UTXO entries
/// needed for sighash computation.
pub fn create_unsigned_payout_tx(
    utxos: &[UtxoInfo],
    winner_address: &str,
    winner_amount: u64,
    platform_address: &str,
    platform_fee: u64,
    escrow_script_public_key: &kaspa_consensus_core::tx::ScriptPublicKey,
) -> Result<(Transaction, Vec<UtxoEntry>), TxError> {
    if utxos.is_empty() {
        return Err(TxError::NoUtxos);
    }

    let winner_addr = Address::try_from(winner_address)
        .map_err(|e| TxError::InvalidAddress(format!("winner: {}", e)))?;
    let platform_addr = Address::try_from(platform_address)
        .map_err(|e| TxError::InvalidAddress(format!("platform: {}", e)))?;

    // Build inputs from UTXOs
    // sig_op_count must equal the number of public keys in the redeem script
    // because OP_CHECKMULTISIG counts N sigops (one per key), not M (the threshold).
    // For a 2-of-3 multisig: 3 keys → sig_op_count = 3.
    // Setting this to 2 (the threshold) causes node rejection:
    // "sig op count exceeds passed limit of 2"
    let inputs: Vec<TransactionInput> = utxos
        .iter()
        .map(|u| {
            let tx_id =
                TransactionId::from_slice(&hex::decode(&u.tx_id).unwrap_or_else(|_| vec![0u8; 32]));
            TransactionInput {
                previous_outpoint: TransactionOutpoint {
                    transaction_id: tx_id,
                    index: u.output_index,
                },
                signature_script: vec![],
                sequence: u64::MAX,
                // For P2SH multisig, Kaspa counts SigOps as N (total public keys),
                // not M (threshold). A 2-of-3 multisig has N=3 → sig_op_count=3.
                sig_op_count: 3,
            }
        })
        .collect();

    // Build outputs
    let mut outputs = vec![TransactionOutput {
        value: winner_amount,
        script_public_key: pay_to_address_script(&winner_addr),
    }];

    if platform_fee > 0 {
        outputs.push(TransactionOutput {
            value: platform_fee,
            script_public_key: pay_to_address_script(&platform_addr),
        });
    }

    let tx = Transaction::new(0, inputs, outputs, 0, SUBNETWORK_ID_NATIVE, 0, vec![]);

    // Build UtxoEntry for each input
    let utxo_entries: Vec<UtxoEntry> = utxos
        .iter()
        .map(|u| UtxoEntry {
            amount: u.amount,
            script_public_key: escrow_script_public_key.clone(),
            block_daa_score: u.block_daa_score,
            is_coinbase: u.is_coinbase,
        })
        .collect();

    Ok((tx, utxo_entries))
}

/// Creates an unsigned refund transaction splitting the escrow equally.
///
/// # Arguments
/// * `utxos` - UTXOs at the escrow P2SH address
/// * `player_a_address` - Address for player A's refund
/// * `player_b_address` - Address for player B's refund
/// * `total_balance` - Total balance in escrow (in sompi)
/// * `network_fee` - Estimated network fee (in sompi)
pub fn create_unsigned_refund_tx(
    utxos: &[UtxoInfo],
    player_a_address: &str,
    player_b_address: &str,
    total_balance: u64,
    network_fee: u64,
    escrow_script_public_key: &kaspa_consensus_core::tx::ScriptPublicKey,
) -> Result<(Transaction, Vec<UtxoEntry>), TxError> {
    if utxos.is_empty() {
        return Err(TxError::NoUtxos);
    }

    let addr_a = Address::try_from(player_a_address)
        .map_err(|e| TxError::InvalidAddress(format!("player A: {}", e)))?;
    let addr_b = Address::try_from(player_b_address)
        .map_err(|e| TxError::InvalidAddress(format!("player B: {}", e)))?;

    let net = total_balance.saturating_sub(network_fee);
    let half = net / 2;

    // sig_op_count = 3: OP_CHECKMULTISIG counts one sigop per public key in the
    // redeem script (n, not m). For a 2-of-3 multisig there are 3 public keys.
    let inputs: Vec<TransactionInput> = utxos
        .iter()
        .map(|u| {
            let tx_id =
                TransactionId::from_slice(&hex::decode(&u.tx_id).unwrap_or_else(|_| vec![0u8; 32]));
            TransactionInput {
                previous_outpoint: TransactionOutpoint {
                    transaction_id: tx_id,
                    index: u.output_index,
                },
                signature_script: vec![],
                sequence: u64::MAX,
                // For P2SH multisig, Kaspa counts SigOps as N (total public keys),
                // not M (threshold). A 2-of-3 multisig has N=3 → sig_op_count=3.
                sig_op_count: 3,
            }
        })
        .collect();

    let mut outputs = Vec::new();
    if player_a_address == player_b_address {
        // Single player refund (Player B never joined) -> send full balance in one output
        // This prevents wallet UI bugs that fail to parse multiple outputs to the same address
        outputs.push(TransactionOutput {
            value: net,
            script_public_key: pay_to_address_script(&addr_a),
        });
    } else {
        // Two-player refund -> split 50/50
        outputs.push(TransactionOutput {
            value: half,
            script_public_key: pay_to_address_script(&addr_a),
        });
        outputs.push(TransactionOutput {
            value: net - half,
            script_public_key: pay_to_address_script(&addr_b),
        });
    }

    let tx = Transaction::new(0, inputs, outputs, 0, SUBNETWORK_ID_NATIVE, 0, vec![]);

    let utxo_entries: Vec<UtxoEntry> = utxos
        .iter()
        .map(|u| UtxoEntry {
            amount: u.amount,
            script_public_key: escrow_script_public_key.clone(),
            block_daa_score: u.block_daa_score,
            is_coinbase: u.is_coinbase,
        })
        .collect();

    Ok((tx, utxo_entries))
}

/// Computes the Schnorr sighash for a specific input of the transaction.
///
/// This is the hash that must be signed by each participant.
pub fn compute_sighash(tx: &Transaction, utxo_entries: Vec<UtxoEntry>, input_index: usize) -> Hash {
    let mut reused_values = SigHashReusedValues::new();
    let populated = PopulatedTransaction::new(tx, utxo_entries);
    calc_schnorr_signature_hash(&populated, input_index, SIG_HASH_ALL, &mut reused_values)
}

/// Computes sighashes for ALL inputs of a transaction.
///
/// Returns one Hash per input. Each signer must sign all of these.
pub fn compute_all_sighashes(tx: &Transaction, utxo_entries: Vec<UtxoEntry>) -> Vec<Hash> {
    let mut reused_values = SigHashReusedValues::new();
    let populated = PopulatedTransaction::new(tx, utxo_entries);
    (0..tx.inputs.len())
        .map(|i| calc_schnorr_signature_hash(&populated, i, SIG_HASH_ALL, &mut reused_values))
        .collect()
}

/// Signs a sighash with a private key, producing a formatted Schnorr signature.
///
/// Returns the signature in multisig format: `OP_DATA_65 | sig_64 | sighash_type`
pub fn sign_sighash(sighash: &Hash, private_key_bytes: &[u8; 32]) -> Result<Vec<u8>, TxError> {
    let secp = secp256k1::Secp256k1::new();
    let sk = secp256k1::SecretKey::from_slice(private_key_bytes)
        .map_err(|e| TxError::SigningFailed(format!("invalid secret key: {}", e)))?;
    let keypair = secp256k1::Keypair::from_secret_key(&secp, &sk);

    let msg = secp256k1::Message::from_digest(sighash.as_bytes());
    let sig = secp.sign_schnorr(&msg, &keypair);

    let sig_bytes: [u8; 64] = sig.serialize();
    Ok(format_schnorr_sig(&sig_bytes, SIG_HASH_ALL.to_u8()))
}

/// Assembles a fully-signed transaction by inserting signature scripts into each input.
///
/// # Arguments
/// * `tx` - The unsigned transaction (will be mutated)
/// * `signatures_per_input` - For each input, a list of formatted signatures
///   (each 66 bytes: OP_DATA_65 + 64-byte sig + sighash_type)
/// * `redeem_script` - The multisig redeem script
///
/// # Returns
/// The signed transaction ready for broadcast.
pub fn assemble_signed_tx(
    mut tx: Transaction,
    signatures_per_input: &[Vec<Vec<u8>>],
    redeem_script: &[u8],
) -> Result<Transaction, TxError> {
    for (i, sigs) in signatures_per_input.iter().enumerate() {
        if i >= tx.inputs.len() {
            break;
        }

        let sig_script = build_multisig_sig_script(sigs, redeem_script)?;
        tx.inputs[i].signature_script = sig_script;
    }

    Ok(tx)
}

/// Converts a consensus `Transaction` to an `RpcTransaction` for node submission.
pub fn to_rpc_transaction(tx: Transaction) -> RpcTransaction {
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
        mass: 0, // Computed by the node
        verbose_data: None,
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::multisig::scripts::{build_multisig_redeem_script, redeem_script_to_p2sh};
    use kaspa_addresses::{Prefix, Version};

    fn test_keypairs() -> (secp256k1::Keypair, secp256k1::Keypair, secp256k1::Keypair) {
        let kp1 = secp256k1::Keypair::from_seckey_slice(
            secp256k1::SECP256K1,
            &hex::decode("1d99c236b1f37b3b845336e6c568ba37e9ced4769d83b7a096eec446b940d160")
                .unwrap(),
        )
        .unwrap();
        let kp2 = secp256k1::Keypair::from_seckey_slice(
            secp256k1::SECP256K1,
            &hex::decode("349ca0c824948fed8c2c568ce205e9d9be4468ef099cad76e3e5ec918954aca4")
                .unwrap(),
        )
        .unwrap();
        let kp3 = secp256k1::Keypair::from_seckey_slice(
            secp256k1::SECP256K1,
            &hex::decode("5f6e7a8b9c0d1e2f3a4b5c6d7e8f9a0b1c2d3e4f5061728394a5b6c7d8e9f001")
                .unwrap(),
        )
        .unwrap();
        (kp1, kp2, kp3)
    }

    fn test_pubkey_hexes(
        kp1: &secp256k1::Keypair,
        kp2: &secp256k1::Keypair,
        kp3: &secp256k1::Keypair,
    ) -> (String, String, String) {
        (
            hex::encode(kp1.x_only_public_key().0.serialize()),
            hex::encode(kp2.x_only_public_key().0.serialize()),
            hex::encode(kp3.x_only_public_key().0.serialize()),
        )
    }

    /// Derives a valid kaspatest P2PK address from a keypair
    fn test_address(kp: &secp256k1::Keypair) -> String {
        let pk_bytes = kp.x_only_public_key().0.serialize();
        let addr = Address::new(Prefix::Testnet, Version::PubKey, &pk_bytes);
        addr.to_string()
    }

    fn make_test_utxos(
        script_pub_key: &kaspa_consensus_core::tx::ScriptPublicKey,
    ) -> Vec<UtxoInfo> {
        vec![
            UtxoInfo {
                tx_id: "63020db736215f8b1105a9281f7bcbb6473d965ecc45bb2fb5da59bd35e6ff84"
                    .to_string(),
                output_index: 0,
                amount: 5_000_000,
                amount_kas: 0.05,
                is_coinbase: false,
                block_daa_score: 100,
                script_public_key: Some(hex::encode(script_pub_key.script())),
            },
            UtxoInfo {
                tx_id: "73020db736215f8b1105a9281f7bcbb6473d965ecc45bb2fb5da59bd35e6ff84"
                    .to_string(),
                output_index: 0,
                amount: 5_000_000,
                amount_kas: 0.05,
                is_coinbase: false,
                block_daa_score: 101,
                script_public_key: Some(hex::encode(script_pub_key.script())),
            },
        ]
    }

    #[test]
    fn test_create_unsigned_payout_tx() {
        let (kp1, kp2, kp3) = test_keypairs();
        let winner_addr = test_address(&kp1);
        let platform_addr = test_address(&kp2);
        let (pk_a, pk_b, pk_o) = test_pubkey_hexes(&kp1, &kp2, &kp3);
        let redeem_script = build_multisig_redeem_script(&[pk_a, pk_b, pk_o], 2).unwrap();
        let p2sh_spk = redeem_script_to_p2sh(&redeem_script);
        let utxos = make_test_utxos(&p2sh_spk);

        let (tx, entries) = create_unsigned_payout_tx(
            &utxos,
            &winner_addr,
            9_500_000,
            &platform_addr,
            500_000,
            &p2sh_spk,
        )
        .unwrap();

        assert_eq!(tx.inputs.len(), 2, "Should have 2 inputs (from 2 UTXOs)");
        assert_eq!(
            tx.outputs.len(),
            2,
            "Should have 2 outputs (winner + platform)"
        );
        assert_eq!(tx.outputs[0].value, 9_500_000);
        assert_eq!(tx.outputs[1].value, 500_000);
        assert_eq!(entries.len(), 2);
    }

    #[test]
    fn test_create_unsigned_payout_no_utxos() {
        let p2sh_spk = kaspa_consensus_core::tx::ScriptPublicKey::default();
        let (kp1, _, _) = test_keypairs();
        let addr = test_address(&kp1);
        let result = create_unsigned_payout_tx(&[], &addr, 9_500_000, &addr, 500_000, &p2sh_spk);
        assert!(result.is_err());
    }

    #[test]
    fn test_create_unsigned_refund_tx() {
        let (kp1, kp2, kp3) = test_keypairs();
        let addr_a = test_address(&kp1);
        let addr_b = test_address(&kp2);
        let (pk_a, pk_b, pk_o) = test_pubkey_hexes(&kp1, &kp2, &kp3);
        let redeem_script = build_multisig_redeem_script(&[pk_a, pk_b, pk_o], 2).unwrap();
        let p2sh_spk = redeem_script_to_p2sh(&redeem_script);
        let utxos = make_test_utxos(&p2sh_spk);

        let (tx, _) =
            create_unsigned_refund_tx(&utxos, &addr_a, &addr_b, 10_000_000, 1_000, &p2sh_spk)
                .unwrap();

        assert_eq!(tx.inputs.len(), 2);
        assert_eq!(tx.outputs.len(), 2);
        // Each player gets ~half of (10M - 1K)
        let total_out: u64 = tx.outputs.iter().map(|o| o.value).sum();
        assert_eq!(total_out, 9_999_000);
    }

    #[test]
    fn test_sign_and_assemble() {
        let (kp1, kp2, kp3) = test_keypairs();
        let winner_addr = test_address(&kp1);
        let platform_addr = test_address(&kp2);
        let (pk_a, pk_b, pk_o) = test_pubkey_hexes(&kp1, &kp2, &kp3);
        let redeem_script = build_multisig_redeem_script(&[pk_a, pk_b, pk_o], 2).unwrap();
        let p2sh_spk = redeem_script_to_p2sh(&redeem_script);
        let utxos = make_test_utxos(&p2sh_spk);

        let (tx, entries) = create_unsigned_payout_tx(
            &utxos,
            &winner_addr,
            9_500_000,
            &platform_addr,
            500_000,
            &p2sh_spk,
        )
        .unwrap();

        // Compute sighashes for all inputs
        let sighashes = compute_all_sighashes(&tx, entries);
        assert_eq!(sighashes.len(), 2, "Should have sighash per input");

        // Sign with keys 1 and 2 (2-of-3)
        let sk1_bytes: [u8; 32] =
            hex::decode("1d99c236b1f37b3b845336e6c568ba37e9ced4769d83b7a096eec446b940d160")
                .unwrap()
                .try_into()
                .unwrap();
        let sk2_bytes: [u8; 32] =
            hex::decode("349ca0c824948fed8c2c568ce205e9d9be4468ef099cad76e3e5ec918954aca4")
                .unwrap()
                .try_into()
                .unwrap();

        let mut sigs_per_input: Vec<Vec<Vec<u8>>> = Vec::new();
        for sighash in &sighashes {
            let sig1 = sign_sighash(sighash, &sk1_bytes).unwrap();
            let sig2 = sign_sighash(sighash, &sk2_bytes).unwrap();
            assert_eq!(sig1.len(), 66, "Formatted sig should be 66 bytes");
            assert_eq!(sig2.len(), 66);
            sigs_per_input.push(vec![sig1, sig2]);
        }

        // Assemble signed TX
        let signed_tx = assemble_signed_tx(tx, &sigs_per_input, &redeem_script).unwrap();

        // Verify signature scripts are populated
        for input in &signed_tx.inputs {
            assert!(
                !input.signature_script.is_empty(),
                "Signature script should be populated"
            );
        }
    }

    #[test]
    fn test_to_rpc_transaction() {
        let tx = Transaction::new(0, vec![], vec![], 0, SUBNETWORK_ID_NATIVE, 0, vec![]);
        let rpc_tx = to_rpc_transaction(tx);
        assert_eq!(rpc_tx.version, 0);
        assert!(rpc_tx.inputs.is_empty());
        assert!(rpc_tx.outputs.is_empty());
        assert_eq!(rpc_tx.mass, 0);
    }
}

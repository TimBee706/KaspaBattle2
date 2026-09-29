//! Interpreter-level tests for KaspaBattle's MatchEscrow contract (Gate 3).
//! All 12 tests pass against the real kaspa-txscript interpreter
//! (`covenants_enabled: true`) at the pinned rusty-kaspa revision, covering
//! all five entry points (including `refund_timeout` -- `this.ageDaa` reads
//! the spending input's own `sequence` field directly, so it's testable here
//! too, no simnet needed; see `tx_input_with_sequence` below).
//!
//! This copy is committed as a reference at
//! `contracts/silverscript/tests/interpreter_tests.rs` in the KaspaBattle
//! repo, alongside a README explaining how to run it (drop it into a clone
//! of kaspanet/silverscript at tag v1.0.0, next to `tests/common.rs`, since
//! it reuses that file and SilverScript's own dev-dependencies). It is not
//! runnable in-place inside the KaspaBattle repo, since kaspabattle's own
//! Cargo workspace pins an incompatible, older rusty-kaspa (0.15.x, no
//! covenant support). Porting this into a real, git-dependency-based Cargo
//! workspace alongside `kaspabattle/battle-silverscript` is follow-up
//! implementation work.
//!
//! Adjust the `include_str!` path in `source()` below to point at your local
//! checkout of contracts/silverscript/match_escrow.sil.

mod common;

use blake2b_simd::Params as Blake2bParams;
use kaspa_consensus_core::Hash;
use kaspa_consensus_core::hashing::sighash::{SigHashReusedValuesUnsync, calc_schnorr_signature_hash};
use kaspa_consensus_core::hashing::sighash_type::SIG_HASH_ALL;
use kaspa_consensus_core::tx::{
    CovenantBinding, PopulatedTransaction, ScriptPublicKey, Transaction, TransactionId, TransactionInput, TransactionOutpoint,
    TransactionOutput, UtxoEntry,
};
use kaspa_txscript::{EngineFlags, pay_to_script_hash_script, pay_to_script_hash_signature_script_with_flags};
use secp256k1::{Keypair, Message, Secp256k1, SecretKey};
use silverscript_abi::ArtifactValue;
use silverscript_lang::compiler::{CompileOptions, compile_to_sil_abi_artifact_with_options};

const STAKE_SOMPI: i64 = 1_000_000_000;
const FEE_BPS: i64 = 100; // 1%
const JOIN_TIMEOUT_DAA: i64 = 3_600;
const RESULT_TIMEOUT_DAA: i64 = 7_200;

fn source() -> &'static str {
    // ADJUST ME: path to KaspaBattle's contracts/silverscript/match_escrow.sil,
    // relative to this file's location inside your SilverScript checkout.
    include_str!("REPLACE_WITH_PATH_TO/kaspabattle/contracts/silverscript/match_escrow.sil")
}

struct Actor {
    keypair: Keypair,
    pubkey_bytes: Vec<u8>,
    owner_hash: [u8; 32],
}

fn blake2b32(data: &[u8]) -> [u8; 32] {
    let hash = Blake2bParams::new().hash_length(32).to_state().update(data).finalize();
    let mut out = [0u8; 32];
    out.copy_from_slice(hash.as_bytes());
    out
}

fn actor_from_seed(seed: u8) -> Actor {
    let secp = Secp256k1::new();
    let secret = SecretKey::from_slice(&[seed; 32]).expect("valid deterministic secret key");
    let keypair = Keypair::from_secret_key(&secp, &secret);
    let (x_only, _) = keypair.x_only_public_key();
    let pubkey_bytes = x_only.serialize().to_vec();
    let owner_hash = blake2b32(&pubkey_bytes);
    Actor { keypair, pubkey_bytes, owner_hash }
}

/// Kaspa's classic CScriptNum little-endian encoding, matching
/// `kaspa_txscript::data_stack::serialize_i64` at the pinned rev. Only
/// non-negative inputs are needed for this contract's attestation fields.
fn script_num_bytes(value: i64, size: usize) -> Vec<u8> {
    assert!(value >= 0, "only non-negative values used in this contract");
    let mut positive = value as u64;
    let mut out = Vec::with_capacity(size);
    while positive != 0 {
        out.push((positive & 0xff) as u8);
        positive >>= 8;
    }
    assert!(out.len() <= size, "value does not fit in {size} bytes");
    out.resize(size, 0);
    out
}

fn p2pk_script(pubkey_bytes: &[u8]) -> ScriptPublicKey {
    let mut v = Vec::with_capacity(34);
    v.push(0x20u8);
    v.extend_from_slice(pubkey_bytes);
    v.push(0xacu8);
    ScriptPublicKey::new(0, v.into())
}

/// What `tx.outputs[idx].scriptPubKey` actually evaluates to in SilverScript:
/// `version (2 bytes, big-endian) || script bytes` -- verified against
/// `kaspa_txscript::SpkEncoding::to_bytes` at the pinned rev. The contract
/// must build its comparison value the same way (see match_escrow.sil).
fn spk_full_bytes(spk: &ScriptPublicKey) -> Vec<u8> {
    let mut v = spk.version().to_be_bytes().to_vec();
    v.extend_from_slice(spk.script());
    v
}

#[allow(dead_code)] // treasury_owner kept for future tests that need to sign as the treasury
struct CtorFixture {
    network_domain: [u8; 32],
    match_id: [u8; 16],
    game_id_hash: [u8; 32],
    player_a: Actor,
    oracle: Actor,
    treasury_owner: Actor,
    treasury_spk: ScriptPublicKey,
    treasury_commitment: [u8; 32],
}

fn fixture() -> CtorFixture {
    let player_a = actor_from_seed(0xA1);
    let oracle = actor_from_seed(0xB2);
    let treasury_owner = actor_from_seed(0xC3);
    let treasury_spk = p2pk_script(&treasury_owner.pubkey_bytes);
    let treasury_commitment = blake2b32(&spk_full_bytes(&treasury_spk));
    CtorFixture {
        network_domain: [0x01; 32],
        match_id: [0x02; 16],
        game_id_hash: [0x03; 32],
        player_a,
        oracle,
        treasury_owner,
        treasury_spk,
        treasury_commitment,
    }
}

fn ctor_args(fx: &CtorFixture, status: i64, player_b: [u8; 32]) -> Vec<ArtifactValue> {
    vec![
        ArtifactValue::Int(1),
        fx.network_domain.to_vec().into(),
        fx.match_id.to_vec().into(),
        fx.game_id_hash.to_vec().into(),
        fx.player_a.owner_hash.to_vec().into(),
        player_b.to_vec().into(),
        ArtifactValue::Int(STAKE_SOMPI),
        ArtifactValue::Int(status),
        fx.oracle.owner_hash.to_vec().into(),
        ArtifactValue::Int(JOIN_TIMEOUT_DAA),
        ArtifactValue::Int(RESULT_TIMEOUT_DAA),
        ArtifactValue::Int(FEE_BPS),
        fx.treasury_commitment.to_vec().into(),
    ]
}

fn compile(ctor: &[ArtifactValue]) -> silverscript_abi::SilAbiArtifact {
    compile_to_sil_abi_artifact_with_options(source(), ctor, CompileOptions::default()).expect("match_escrow.sil compiles")
}

fn tx_input(signature_script: Vec<u8>) -> TransactionInput {
    tx_input_with_sequence(signature_script, 0)
}

/// `sequence` is what `this.ageDaa` reads (verified against
/// `compiles_require_age_daa_to_csv_and_verifies` in SilverScript's own
/// `compiler_tests.rs`: `this.ageDaa >= N` lowers to `OpCheckSequenceVerify`,
/// which checks the spending input's own `sequence` field directly -- no
/// simnet/live-consensus context needed, contrary to an earlier assumption
/// in this file's own doc comment (see docs/LEARNINGS.md).
fn tx_input_with_sequence(signature_script: Vec<u8>, sequence: u64) -> TransactionInput {
    TransactionInput::new(TransactionOutpoint { transaction_id: TransactionId::from_bytes([7u8; 32]), index: 0 }, signature_script, sequence, 1)
}

fn entry_sigscript(compiled: &silverscript_abi::SilAbiArtifact, entry: &str, args: Vec<ArtifactValue>) -> Vec<u8> {
    let sigscript = common::encode_entry_sig_script(compiled, entry, &args).expect("sigscript builds");
    pay_to_script_hash_signature_script_with_flags(
        common::bytecode(compiled),
        sigscript,
        EngineFlags { covenants_enabled: true, ..Default::default() },
    )
    .expect("wrap p2sh sigscript")
}

fn sign_tx_input_schnorr(tx: &Transaction, entries: &[UtxoEntry], actor: &Actor) -> Vec<u8> {
    let reused_values = SigHashReusedValuesUnsync::new();
    let populated = PopulatedTransaction::new(tx, entries.to_vec());
    let sig_hash = calc_schnorr_signature_hash(&populated, 0, SIG_HASH_ALL, &reused_values);
    let msg = Message::from_digest_slice(sig_hash.as_bytes().as_slice()).expect("valid sighash message");
    let sig = actor.keypair.sign_schnorr(msg);
    let mut signature = Vec::new();
    signature.extend_from_slice(sig.as_ref());
    signature.push(SIG_HASH_ALL.to_u8());
    signature
}

fn oracle_settle_digest(fx: &CtorFixture, winner_selector: i64, result_hash: [u8; 32], observed_at: i64, nonce: [u8; 32]) -> [u8; 32] {
    let mut msg = Vec::new();
    msg.extend_from_slice(b"KASPABATTLE_RESULT_V1");
    msg.extend_from_slice(&fx.network_domain);
    msg.extend_from_slice(&script_num_bytes(1, 4)); // contract_version
    msg.extend_from_slice(&fx.match_id);
    msg.extend_from_slice(&fx.game_id_hash);
    msg.extend_from_slice(&script_num_bytes(winner_selector, 1));
    msg.extend_from_slice(&result_hash);
    msg.extend_from_slice(&script_num_bytes(observed_at, 8));
    msg.extend_from_slice(&nonce);
    blake2b32(&msg)
}

const COV_ID: Hash = Hash::from_bytes([0x99u8; 32]);

fn covenant_utxo(compiled: &silverscript_abi::SilAbiArtifact, value: u64) -> UtxoEntry {
    UtxoEntry::new(value, pay_to_script_hash_script(&common::bytecode(compiled)), 0, false, Some(COV_ID))
}

fn covenant_output(compiled: &silverscript_abi::SilAbiArtifact, value: u64) -> TransactionOutput {
    TransactionOutput {
        value,
        script_public_key: pay_to_script_hash_script(&common::bytecode(compiled)),
        covenant: Some(CovenantBinding { authorizing_input: 0, covenant_id: COV_ID }),
    }
}

// A terminal (non-continuing) payout output. Its `covenant_id` must equal the
// SPENT input's covenant_id (COV_ID) to be treated as a "continuation" by
// `CovenantsContext::from_tx` -- a different id would be classified as a
// "genesis" (new covenant) and rejected as WrongGenesisCovenantId, since a
// genesis id must be the protocol-derived id from the spent outpoint, not an
// arbitrary value. Continuation outputs are NOT required to have a
// covenant-shaped scriptPubKey; a plain P2PK destination continuing the same
// id is exactly how OpAuthOutputCount/OpAuthOutputIdx count a terminal payout
// as "authorized by this input" (verified against covenants.rs at the pinned
// rev -- this took one failed hypothesis, see docs/LEARNINGS.md, to pin down).
fn plain_output(spk: ScriptPublicKey, value: u64) -> TransactionOutput {
    TransactionOutput { value, script_public_key: spk, covenant: Some(CovenantBinding { authorizing_input: 0, covenant_id: COV_ID }) }
}

fn unauthorized_output(spk: ScriptPublicKey, value: u64) -> TransactionOutput {
    TransactionOutput { value, script_public_key: spk, covenant: None }
}

// ─── join ───────────────────────────────────────────────────────────────────

#[test]
fn join_succeeds() {
    let fx = fixture();
    let player_b = actor_from_seed(0xD4);

    let active = compile(&ctor_args(&fx, 0, [0u8; 32]));
    let next = compile(&ctor_args(&fx, 1, player_b.owner_hash));

    let entries = vec![covenant_utxo(&active, STAKE_SOMPI as u64)];
    let outputs = vec![covenant_output(&next, 2 * STAKE_SOMPI as u64)];

    let placeholder_sigscript =
        entry_sigscript(&active, "join", vec![ArtifactValue::Bytes(vec![0u8; 65]), player_b.pubkey_bytes.clone().into()]);
    let mut tx = Transaction::new(1, vec![tx_input(placeholder_sigscript)], outputs, 0, Default::default(), 0, vec![]);
    let sig = sign_tx_input_schnorr(&tx, &entries, &player_b);
    tx.inputs[0].signature_script = entry_sigscript(&active, "join", vec![ArtifactValue::Bytes(sig), player_b.pubkey_bytes.into()]);

    let result = common::execute_input_with_covenants(tx, entries, 0);
    assert!(result.is_ok(), "join should succeed: {:?}", result.unwrap_err());
}

#[test]
fn join_rejects_creator_joining_own_match() {
    let fx = fixture();
    // player_a tries to "join" its own match as player_b.
    let active = compile(&ctor_args(&fx, 0, [0u8; 32]));
    let next = compile(&ctor_args(&fx, 1, fx.player_a.owner_hash));

    let entries = vec![covenant_utxo(&active, STAKE_SOMPI as u64)];
    let outputs = vec![covenant_output(&next, 2 * STAKE_SOMPI as u64)];

    let placeholder_sigscript =
        entry_sigscript(&active, "join", vec![ArtifactValue::Bytes(vec![0u8; 65]), fx.player_a.pubkey_bytes.clone().into()]);
    let mut tx = Transaction::new(1, vec![tx_input(placeholder_sigscript)], outputs, 0, Default::default(), 0, vec![]);
    let sig = sign_tx_input_schnorr(&tx, &entries, &fx.player_a);
    tx.inputs[0].signature_script =
        entry_sigscript(&active, "join", vec![ArtifactValue::Bytes(sig), fx.player_a.pubkey_bytes.clone().into()]);

    let result = common::execute_input_with_covenants(tx, entries, 0);
    assert!(result.is_err(), "join must reject player_a joining their own match");
}

#[test]
fn join_rejects_wrong_output_value() {
    let fx = fixture();
    let player_b = actor_from_seed(0xD4);
    let active = compile(&ctor_args(&fx, 0, [0u8; 32]));
    let next = compile(&ctor_args(&fx, 1, player_b.owner_hash));

    let entries = vec![covenant_utxo(&active, STAKE_SOMPI as u64)];
    // Output value is short by 1 sompi -- must be rejected.
    let outputs = vec![covenant_output(&next, 2 * STAKE_SOMPI as u64 - 1)];

    let placeholder_sigscript =
        entry_sigscript(&active, "join", vec![ArtifactValue::Bytes(vec![0u8; 65]), player_b.pubkey_bytes.clone().into()]);
    let mut tx = Transaction::new(1, vec![tx_input(placeholder_sigscript)], outputs, 0, Default::default(), 0, vec![]);
    let sig = sign_tx_input_schnorr(&tx, &entries, &player_b);
    tx.inputs[0].signature_script = entry_sigscript(&active, "join", vec![ArtifactValue::Bytes(sig), player_b.pubkey_bytes.into()]);

    let result = common::execute_input_with_covenants(tx, entries, 0);
    assert!(result.is_err(), "join must reject an under-funded continuation output");
}

// ─── cancel_unjoined ────────────────────────────────────────────────────────

#[test]
fn cancel_unjoined_succeeds() {
    let fx = fixture();
    let active = compile(&ctor_args(&fx, 0, [0u8; 32]));
    let entries = vec![covenant_utxo(&active, STAKE_SOMPI as u64)];
    // Refund can go anywhere player_a chooses (single-party disposal) -- here,
    // an arbitrary destination unrelated to player_a's own P2PK, to prove the
    // contract truly does not constrain it once the signature is proven.
    let arbitrary_destination = actor_from_seed(0xEE);
    let outputs = vec![unauthorized_output(p2pk_script(&arbitrary_destination.pubkey_bytes), STAKE_SOMPI as u64)];

    let placeholder_sigscript =
        entry_sigscript(&active, "cancel_unjoined", vec![ArtifactValue::Bytes(vec![0u8; 65]), fx.player_a.pubkey_bytes.clone().into()]);
    let mut tx = Transaction::new(1, vec![tx_input(placeholder_sigscript)], outputs, 0, Default::default(), 0, vec![]);
    let sig = sign_tx_input_schnorr(&tx, &entries, &fx.player_a);
    tx.inputs[0].signature_script =
        entry_sigscript(&active, "cancel_unjoined", vec![ArtifactValue::Bytes(sig), fx.player_a.pubkey_bytes.clone().into()]);

    let result = common::execute_input_with_covenants(tx, entries, 0);
    assert!(result.is_ok(), "cancel_unjoined should succeed: {:?}", result.unwrap_err());
}

#[test]
fn cancel_unjoined_rejects_wrong_signer() {
    let fx = fixture();
    let active = compile(&ctor_args(&fx, 0, [0u8; 32]));
    let entries = vec![covenant_utxo(&active, STAKE_SOMPI as u64)];
    let impostor = actor_from_seed(0x77);
    let outputs = vec![unauthorized_output(p2pk_script(&impostor.pubkey_bytes), STAKE_SOMPI as u64)];

    let placeholder_sigscript =
        entry_sigscript(&active, "cancel_unjoined", vec![ArtifactValue::Bytes(vec![0u8; 65]), impostor.pubkey_bytes.clone().into()]);
    let mut tx = Transaction::new(1, vec![tx_input(placeholder_sigscript)], outputs, 0, Default::default(), 0, vec![]);
    let sig = sign_tx_input_schnorr(&tx, &entries, &impostor);
    tx.inputs[0].signature_script =
        entry_sigscript(&active, "cancel_unjoined", vec![ArtifactValue::Bytes(sig), impostor.pubkey_bytes.clone().into()]);

    let result = common::execute_input_with_covenants(tx, entries, 0);
    assert!(result.is_err(), "cancel_unjoined must reject a non-player_a signer");
}

// ─── mutual_settle ──────────────────────────────────────────────────────────

#[test]
fn mutual_settle_succeeds_with_agreed_split() {
    let fx = fixture();
    let player_b = actor_from_seed(0xD4);
    let active = compile(&ctor_args(&fx, 1, player_b.owner_hash));
    let entries = vec![covenant_utxo(&active, 2 * STAKE_SOMPI as u64)];

    let a_amount = 300_000_000i64;
    let b_amount = 2 * STAKE_SOMPI - a_amount;
    let outputs = vec![plain_output(p2pk_script(&fx.player_a.pubkey_bytes), a_amount as u64), plain_output(p2pk_script(&player_b.pubkey_bytes), b_amount as u64)];

    let build_args = |a_sig: Vec<u8>, b_sig: Vec<u8>| {
        vec![
            ArtifactValue::Bytes(a_sig),
            fx.player_a.pubkey_bytes.clone().into(),
            ArtifactValue::Bytes(b_sig),
            player_b.pubkey_bytes.clone().into(),
            ArtifactValue::Int(a_amount),
            ArtifactValue::Int(b_amount),
        ]
    };

    let placeholder = entry_sigscript(&active, "mutual_settle", build_args(vec![0u8; 65], vec![0u8; 65]));
    let mut tx = Transaction::new(1, vec![tx_input(placeholder)], outputs, 0, Default::default(), 0, vec![]);
    let a_sig = sign_tx_input_schnorr(&tx, &entries, &fx.player_a);
    let b_sig = sign_tx_input_schnorr(&tx, &entries, &player_b);
    tx.inputs[0].signature_script = entry_sigscript(&active, "mutual_settle", build_args(a_sig, b_sig));

    let result = common::execute_input_with_covenants(tx, entries, 0);
    assert!(result.is_ok(), "mutual_settle should succeed: {:?}", result.unwrap_err());
}

#[test]
fn mutual_settle_rejects_amounts_not_summing_to_pot() {
    let fx = fixture();
    let player_b = actor_from_seed(0xD4);
    let active = compile(&ctor_args(&fx, 1, player_b.owner_hash));
    let entries = vec![covenant_utxo(&active, 2 * STAKE_SOMPI as u64)];

    // Amounts sum to less than the pot -- the difference would be siphoned off.
    let a_amount = 300_000_000i64;
    let b_amount = 2 * STAKE_SOMPI - a_amount - 1;
    let outputs = vec![plain_output(p2pk_script(&fx.player_a.pubkey_bytes), a_amount as u64), plain_output(p2pk_script(&player_b.pubkey_bytes), b_amount as u64)];

    let build_args = |a_sig: Vec<u8>, b_sig: Vec<u8>| {
        vec![
            ArtifactValue::Bytes(a_sig),
            fx.player_a.pubkey_bytes.clone().into(),
            ArtifactValue::Bytes(b_sig),
            player_b.pubkey_bytes.clone().into(),
            ArtifactValue::Int(a_amount),
            ArtifactValue::Int(b_amount),
        ]
    };

    let placeholder = entry_sigscript(&active, "mutual_settle", build_args(vec![0u8; 65], vec![0u8; 65]));
    let mut tx = Transaction::new(1, vec![tx_input(placeholder)], outputs, 0, Default::default(), 0, vec![]);
    let a_sig = sign_tx_input_schnorr(&tx, &entries, &fx.player_a);
    let b_sig = sign_tx_input_schnorr(&tx, &entries, &player_b);
    tx.inputs[0].signature_script = entry_sigscript(&active, "mutual_settle", build_args(a_sig, b_sig));

    let result = common::execute_input_with_covenants(tx, entries, 0);
    assert!(result.is_err(), "mutual_settle must reject amounts that do not sum to the full pot");
}

// ─── oracle_settle ──────────────────────────────────────────────────────────

#[test]
fn oracle_settle_succeeds_for_winner_a() {
    let fx = fixture();
    let player_b = actor_from_seed(0xD4);
    let active = compile(&ctor_args(&fx, 1, player_b.owner_hash));
    let entries = vec![covenant_utxo(&active, 2 * STAKE_SOMPI as u64)];

    let winner_selector = 0i64; // player_a wins
    let result_hash = [0x07u8; 32];
    let observed_at = 123_456i64;
    let nonce = [0x08u8; 32];
    let digest = oracle_settle_digest(&fx, winner_selector, result_hash, observed_at, nonce);
    let oracle_sig = fx.oracle.keypair.sign_schnorr(Message::from_digest(digest)).as_ref().to_vec();

    let total = 2 * STAKE_SOMPI;
    let fee = total * FEE_BPS / 10_000;
    let payout = total - fee;
    let outputs = vec![plain_output(p2pk_script(&fx.player_a.pubkey_bytes), payout as u64), plain_output(fx.treasury_spk.clone(), fee as u64)];

    let args = vec![
        ArtifactValue::Bytes(oracle_sig),
        fx.oracle.pubkey_bytes.clone().into(),
        fx.player_a.pubkey_bytes.clone().into(),
        ArtifactValue::Bytes(spk_full_bytes(&fx.treasury_spk)),
        ArtifactValue::Int(winner_selector),
        result_hash.to_vec().into(),
        ArtifactValue::Int(observed_at),
        nonce.to_vec().into(),
    ];
    let sigscript = entry_sigscript(&active, "oracle_settle", args);
    let tx = Transaction::new(1, vec![tx_input(sigscript)], outputs, 0, Default::default(), 0, vec![]);

    let result = common::execute_input_with_covenants(tx, entries, 0);
    assert!(result.is_ok(), "oracle_settle should succeed: {:?}", result.unwrap_err());
}

#[test]
fn oracle_settle_rejects_forged_signature() {
    let fx = fixture();
    let player_b = actor_from_seed(0xD4);
    let active = compile(&ctor_args(&fx, 1, player_b.owner_hash));
    let entries = vec![covenant_utxo(&active, 2 * STAKE_SOMPI as u64)];

    let winner_selector = 0i64;
    let result_hash = [0x07u8; 32];
    let observed_at = 123_456i64;
    let nonce = [0x08u8; 32];
    let digest = oracle_settle_digest(&fx, winner_selector, result_hash, observed_at, nonce);
    let mut forged_sig = fx.oracle.keypair.sign_schnorr(Message::from_digest(digest)).as_ref().to_vec();
    forged_sig[0] ^= 0x01;

    let total = 2 * STAKE_SOMPI;
    let fee = total * FEE_BPS / 10_000;
    let payout = total - fee;
    let outputs = vec![plain_output(p2pk_script(&fx.player_a.pubkey_bytes), payout as u64), plain_output(fx.treasury_spk.clone(), fee as u64)];

    let args = vec![
        ArtifactValue::Bytes(forged_sig),
        fx.oracle.pubkey_bytes.clone().into(),
        fx.player_a.pubkey_bytes.clone().into(),
        ArtifactValue::Bytes(spk_full_bytes(&fx.treasury_spk)),
        ArtifactValue::Int(winner_selector),
        result_hash.to_vec().into(),
        ArtifactValue::Int(observed_at),
        nonce.to_vec().into(),
    ];
    let sigscript = entry_sigscript(&active, "oracle_settle", args);
    let tx = Transaction::new(1, vec![tx_input(sigscript)], outputs, 0, Default::default(), 0, vec![]);

    let result = common::execute_input_with_covenants(tx, entries, 0);
    assert!(result.is_err(), "oracle_settle must reject a forged oracle signature");
}

#[test]
fn oracle_settle_rejects_redirected_winner_output() {
    // A malicious backend tries to redirect the winner's payout to an
    // unrelated address while keeping a validly-signed attestation for the
    // real winner -- this is the "Output substitution" threat model entry.
    let fx = fixture();
    let player_b = actor_from_seed(0xD4);
    let active = compile(&ctor_args(&fx, 1, player_b.owner_hash));
    let entries = vec![covenant_utxo(&active, 2 * STAKE_SOMPI as u64)];

    let winner_selector = 0i64; // player_a really won
    let result_hash = [0x07u8; 32];
    let observed_at = 123_456i64;
    let nonce = [0x08u8; 32];
    let digest = oracle_settle_digest(&fx, winner_selector, result_hash, observed_at, nonce);
    let oracle_sig = fx.oracle.keypair.sign_schnorr(Message::from_digest(digest)).as_ref().to_vec();

    let total = 2 * STAKE_SOMPI;
    let fee = total * FEE_BPS / 10_000;
    let payout = total - fee;
    let attacker = actor_from_seed(0x66);
    // Output actually pays the attacker, not player_a.
    let outputs = vec![plain_output(p2pk_script(&attacker.pubkey_bytes), payout as u64), plain_output(fx.treasury_spk.clone(), fee as u64)];

    let args = vec![
        ArtifactValue::Bytes(oracle_sig),
        fx.oracle.pubkey_bytes.clone().into(),
        fx.player_a.pubkey_bytes.clone().into(), // winner_pk still claims to be player_a
        ArtifactValue::Bytes(spk_full_bytes(&fx.treasury_spk)),
        ArtifactValue::Int(winner_selector),
        result_hash.to_vec().into(),
        ArtifactValue::Int(observed_at),
        nonce.to_vec().into(),
    ];
    let sigscript = entry_sigscript(&active, "oracle_settle", args);
    let tx = Transaction::new(1, vec![tx_input(sigscript)], outputs, 0, Default::default(), 0, vec![]);

    let result = common::execute_input_with_covenants(tx, entries, 0);
    assert!(result.is_err(), "oracle_settle must reject a payout redirected away from the committed winner scriptPubKey");
}

// ─── refund_timeout ─────────────────────────────────────────────────────────

#[test]
fn refund_timeout_succeeds_once_timeout_elapsed() {
    let fx = fixture();
    let player_b = actor_from_seed(0xD4);
    let active = compile(&ctor_args(&fx, 1, player_b.owner_hash));
    let entries = vec![covenant_utxo(&active, 2 * STAKE_SOMPI as u64)];

    let outputs = vec![
        plain_output(p2pk_script(&fx.player_a.pubkey_bytes), STAKE_SOMPI as u64),
        plain_output(p2pk_script(&player_b.pubkey_bytes), STAKE_SOMPI as u64),
    ];
    let args = vec![fx.player_a.pubkey_bytes.clone().into(), player_b.pubkey_bytes.clone().into()];
    let sigscript = entry_sigscript(&active, "refund_timeout", args);
    // sequence == this.ageDaa; RESULT_TIMEOUT_DAA exactly met (>=, boundary case).
    let tx = Transaction::new(
        1,
        vec![tx_input_with_sequence(sigscript, RESULT_TIMEOUT_DAA as u64)],
        outputs,
        0,
        Default::default(),
        0,
        vec![],
    );

    let result = common::execute_input_with_covenants(tx, entries, 0);
    assert!(result.is_ok(), "refund_timeout should succeed once the timeout has elapsed: {:?}", result.unwrap_err());
}

#[test]
fn refund_timeout_rejects_before_timeout_elapsed() {
    let fx = fixture();
    let player_b = actor_from_seed(0xD4);
    let active = compile(&ctor_args(&fx, 1, player_b.owner_hash));
    let entries = vec![covenant_utxo(&active, 2 * STAKE_SOMPI as u64)];

    let outputs = vec![
        plain_output(p2pk_script(&fx.player_a.pubkey_bytes), STAKE_SOMPI as u64),
        plain_output(p2pk_script(&player_b.pubkey_bytes), STAKE_SOMPI as u64),
    ];
    let args = vec![fx.player_a.pubkey_bytes.clone().into(), player_b.pubkey_bytes.clone().into()];
    let sigscript = entry_sigscript(&active, "refund_timeout", args);
    // One tick short of RESULT_TIMEOUT_DAA -- must be rejected.
    let tx = Transaction::new(
        1,
        vec![tx_input_with_sequence(sigscript, (RESULT_TIMEOUT_DAA - 1) as u64)],
        outputs,
        0,
        Default::default(),
        0,
        vec![],
    );

    let result = common::execute_input_with_covenants(tx, entries, 0);
    assert!(result.is_err(), "refund_timeout must reject a claim before the timeout has elapsed");
}

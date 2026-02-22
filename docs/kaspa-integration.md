# Kaspa Integration

KaspaBattle is one of the first platforms to deeply integrate the `rusty-kaspa` ecosystem into a web application.

## SDKs & Crates

We utilize multiple layers of the Kaspa SDK:

1. **`kaspa-rpc-core` / `kaspa-wrpc-client`**: For high-performance communication with `kaspad`.
2. **`kaspa-wallet-core`**: For managing UTXOs, accounts, and transaction construction.
3. **`kaspa-wasm`**: For bringing native Kaspa performance to the browser environment.

## Integration Patterns

### 1. wRPC (WebSocket RPC)

Unlike traditional JSON-RPC, we use Kaspa's wRPC for:

- **Low Latency**: Faster updates on DAG state.
- **Subscriptions**: Real-time notifications when a player's deposit is confirmed.

### 2. Non-Custodial Escrow

For every match, a unique address is derived.

- In **Phase 1 (current)**: The backend generates the escrow address from a match-specific keypair produced by a master seed.
- In **Phase 2 (roadmap)**: Integration with **Kasplex L2** smart contracts for multi-sig and script-based logic.

### 3. Transaction Signing

We use the **Schnorr signature** scheme (standard in Kaspa). On the backend, signing happens in a secure two-pass process:

1. **Sighash Calculation**: Creating the preimage for all transaction inputs.
2. **Signature Injection**: Signing the hashes and updating the transaction inputs.

```rust
// Simplified Backend Signing Flow
let sighashes = {
    let populated = PopulatedTransaction::new(&tx, utxo_entries);
    (0..tx.inputs.len()).map(|i| {
        calc_schnorr_signature_hash(&populated, i, SIG_HASH_ALL, &mut reused_values)
    }).collect()
};
// Sign hashes and inject scripts...
```

## UTXO Management

To ensure reliability and prevent double-spending:

- **UTXO Index**: Our nodes must run with `--utxoindex` enabled.
- **Attribution**: The system verifies that each player sends at least the required wager amount from their designated address.

---
[Frontend ←](frontend.md) | [Security →](security.md)

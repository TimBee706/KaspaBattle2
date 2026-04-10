# Kaspa Integration Guide

This document explains how KaspaBattle integrates with the Kaspa blockchain — from node connectivity to UTXO management, escrow addresses, and transaction building.

---

## Table of Contents

- [Kaspa Blockchain Fundamentals](#kaspa-blockchain-fundamentals)
- [Node Connectivity](#node-connectivity)
- [RPC Client Setup](#rpc-client-setup)
- [UTXO Management](#utxo-management)
- [Escrow Address Derivation](#escrow-address-derivation)
- [Transaction Building](#transaction-building)
- [Wallet Integration](#wallet-integration)
- [Testnet vs Mainnet](#testnet-vs-mainnet)
- [Node Requirements](#node-requirements)

---

## Kaspa Blockchain Fundamentals

### BlockDAG and GhostDAG

Unlike traditional blockchains that form a single chain of blocks, Kaspa uses a **Directed Acyclic Graph (DAG)** structure called a **blockDAG**. Multiple blocks can be created in parallel and reference multiple parent blocks.

```
        ┌───┐
    ┌──▶│ B │──┐
    │   └───┘  │   ┌───┐
┌───┤          ├──▶│ D │
│ A │   ┌───┐  │   └───┘
└───┤──▶│ C │──┘
    │   └───┘
    │   ┌───┐
    └──▶│ E │──────▶ ...
        └───┘
```

**GhostDAG** is the consensus protocol that orders this DAG. It selects one block per "layer" as the **selected parent**, creating a canonical ordering of all transactions. Blocks not on the selected chain are still included in the DAG (as "merged" blocks), but may have lower priority.

**Key implications for KaspaBattle:**

- **Block rate**: ~10 blocks per second, providing near-instant transaction confirmation
- **Reorgs**: Because the DAG is ordered probabilistically, blocks can be reordered. The kdapp Engine handles this via rollbacks
- **Finality**: Unlike PoW chains, Kaspa achieves practical finality within seconds, but deep reorgs are theoretically possible

### UTXO Model

Kaspa uses the UTXO (Unspent Transaction Output) model:

- Transactions consume existing UTXOs as **inputs** and create new UTXOs as **outputs**
- Each UTXO has a **value** (in sompi) and a **locking script** (spending condition)
- To spend a UTXO, you must provide a valid **unlocking script** (typically a signature)

**1 KAS = 100,000,000 sompi** (10^8, similar to Bitcoin's satoshi)

---

## Node Connectivity

### wRPC vs gRPC

Kaspa nodes expose two RPC interfaces:

| Protocol | Port | Use Case | Notes |
|----------|------|----------|-------|
| **wRPC** (WebSocket) | 17110 (mainnet) / 17210 (testnet) | Browser clients, kdapp Proxy | Borsh-encoded, bidirectional |
| **gRPC** | 16110 (mainnet) / 16210 (testnet) | Server-to-server, backend | Protobuf-encoded, high performance |

**KaspaBattle uses wRPC** for both the backend (`battle-kaspa`) and the frontend (Kaspa WASM SDK). This simplifies the deployment since only one protocol needs to be configured.

### Public Node Network (PNN)

If you don't run your own node, Kaspa provides a **resolver** that discovers public nodes automatically:

```
Resolver → discovers available public nodes → returns wRPC URL
```

The kdapp Proxy uses this by default. For production, we recommend running your own node to avoid rate limits and ensure availability.

---

## RPC Client Setup

### Backend Connection (battle-kaspa)

The backend connects to kaspad via the `kaspa-wrpc-client` crate. The connection is established in `rpc.rs`:

The RPC client wrapper (`KaspaRpcService`) provides:

- **`get_balance(address)`** — Sum of all UTXO values for an address
- **`get_utxos(address)`** — List of all unspent outputs for an address
- **`submit_transaction(tx)`** — Broadcast a signed transaction to the network
- **`get_block_dag_info()`** — Current DAG state (tip hashes, DAA score)
- **`get_virtual_chain_from_block(hash)`** — VSPC changes since a given block

### Frontend Connection (WASM SDK)

The frontend uses the Kaspa WASM SDK compiled to WebAssembly. It connects directly to a Kaspa node via wRPC, bypassing the backend for wallet operations:

The WASM SDK handles:

- Key generation and storage (in-browser)
- Address derivation
- Transaction signing
- Balance queries

---

## UTXO Management

### Querying UTXOs

To check deposits or calculate balances, the backend queries UTXOs by address:

The `get_utxos_by_addresses` RPC call returns all unspent outputs for the given addresses. Each UTXO contains:

- `outpoint` — The transaction ID and output index that created this UTXO
- `utxo_entry` — The value, script, DAA score, and coinbase flag

### UTXO Selection for Transactions

When building a transaction, you need to select UTXOs as inputs. The selection strategy matters for:

- **Fee estimation** — More inputs = larger transaction = higher fee
- **Change outputs** — If inputs exceed the needed amount, create a change output
- **Dust prevention** — Avoid creating outputs below the dust threshold

KaspaBattle's escrow addresses typically have few UTXOs (1-2 per player deposit), so selection is straightforward: use all UTXOs from the escrow address.

---

## Escrow Address Derivation

Each match gets a unique, deterministic escrow address derived from the match parameters.

### Derivation Process

```
Input:  match_id + player_a_pubkey + player_b_pubkey
          │
          ▼
        SHA-256 hash
          │
          ▼
        32-byte secret key
          │
          ▼
        secp256k1 public key
          │
          ▼
        Kaspa address (x-only pubkey, Schnorr)
```

**Properties:**

- **Deterministic** — Same inputs always produce the same address
- **Unique** — Different match IDs produce different addresses
- **Non-custodial (future)** — With multisig, both players must co-sign to spend

### Network-Aware Addresses

Kaspa addresses include a network prefix:

| Network | Prefix | Example |
|---------|--------|---------|
| Mainnet | `kaspa:` | `kaspa:qz7yjd...` |
| Testnet-10 | `kaspatest:` | `kaspatest:qz7yjd...` |
| Testnet-11 | `kaspatest:` | `kaspatest:qz7yjd...` |

---

## Transaction Building

### Anatomy of a Kaspa Transaction

```
Transaction {
    version: 0,                    // TX_VERSION constant
    inputs: [                      // UTXOs being spent
        TransactionInput {
            previous_outpoint: (tx_id, index),
            signature_script: [...],    // Schnorr signature
            sequence: 0,
            sig_op_count: 1,
        }
    ],
    outputs: [                     // New UTXOs being created
        TransactionOutput {
            value: 950_000,             // Amount in sompi
            script_public_key: [...],   // Pay-to-pubkey-hash
        }
    ],
    lock_time: 0,
    subnetwork_id: NATIVE,
    gas: 0,
    payload: [...],                // Optional data (used by kdapp)
}
```

### Fee Estimation

Kaspa transaction fees are based on **mass** (similar to Bitcoin's weight):

- Base mass per transaction
- Additional mass per input and output
- Additional mass per byte of payload

For simple escrow transactions (2-3 inputs, 2-3 outputs, no payload), fees are typically around **1,000 sompi** (~0.00001 KAS).

### kdapp Transaction Pattern Matching

The kdapp Generator adds a unique twist: it iterates a nonce in the transaction payload until the resulting transaction ID matches a predefined bit pattern. This allows the Proxy to efficiently filter transactions on the network.

A 10-bit pattern means approximately 1 in 1,024 transactions will match, providing a good balance between filtering efficiency and nonce search time.

---

## Wallet Integration

### Backend Wallet (EscrowWallet)

The `EscrowWallet` in `battle-kaspa` manages escrow keys:

- Derives keys from a BIP-44 HD wallet using a mnemonic phrase
- Each match gets a unique derivation index
- Keys are used for signing payout and refund transactions
- Keys are zeroized on drop (security hardening)

### Frontend Wallet (WASM SDK)

The browser-side wallet uses the Kaspa WASM SDK:

- Generates a keypair in the browser
- Stores the private key in the browser (not sent to the server)
- Signs deposit transactions locally
- Connects to kaspad via wRPC for balance queries and TX submission

### Multisig Wallet (In Progress)

The `multisig/` module in `battle-kaspa` implements 2-of-2 multisig escrow:

- Both players and the Oracle must co-sign payout transactions
- Uses Partially Signed Kaspa Transactions (PSKT) for multi-party signing
- Eliminates server-side key custody

---

## Testnet vs Mainnet

### Configuration

The network is configured via environment variables:

| Variable | Default | Description |
|----------|---------|-------------|
| `KASPA_NETWORK` | `testnet-12` | Network identifier |
| `KASPA_NODE_URL` | (resolver) | Direct wRPC URL to kaspad |
| `VITE_KASPA_NETWORK` | `testnet-12` | Frontend network config |

### Testnet Resources

- **Faucet**: Get free testnet KAS at the Kaspa Testnet Faucet
- **Explorer**: View transactions at `https://explorer-tn12.kaspa.org`

### Switching to Mainnet

1. Set `KASPA_NETWORK=mainnet` in the backend `.env`
2. Set `VITE_KASPA_NETWORK=mainnet` in the frontend `.env`
3. Point `KASPA_NODE_URL` to a mainnet kaspad node
4. Ensure the escrow wallet mnemonic is backed up securely

---

## Node Requirements

### Running Your Own kaspad

For production, run your own Kaspa node:

```bash
# From rusty-kaspa
cargo build --release -p kaspad
./target/release/kaspad --utxoindex --rpclisten=0.0.0.0:16110
```

**Required flags:**

| Flag | Purpose |
|------|---------|
| `--utxoindex` | Enables UTXO queries by address (required for balance checks) |
| `--rpclisten` | Bind address for gRPC (default: localhost) |

**Recommended specs:**

| Resource | Minimum | Recommended |
|----------|---------|-------------|
| CPU | 4 cores | 8+ cores |
| RAM | 8 GB | 16+ GB |
| SSD | 100 GB | 250+ GB |
| Network | 10 Mbps | 100+ Mbps |

### Docker Setup

KaspaBattle's `docker-compose.yml` can include a kaspad container:

```yaml
kaspad:
  image: supertypo/rusty-kaspa:latest
  command: kaspad --utxoindex --rpclisten=0.0.0.0:16110
  ports:
    - "16110:16110"
    - "17110:17110"
```

---

## Next Steps

- [Architecture Overview](01-ARCHITECTURE.md) — System-level architecture
- [Security Model](03-SECURITY.md) — Trust model and escrow security
- [Episode Framework](04-EPISODE-FRAMEWORK.md) — Building on kdapp

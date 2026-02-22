# Backend Documentation

The KaspaBattle backend is a high-performance Rust application designed to manage match states, monitor the blockchain, and execute payouts.

## Project Structure

The backend is organized as a Cargo workspace:

- **`battle-api`**: The entry point. Handles HTTP requests, authentication, and routing.
- **`battle-kaspa`**: Contains logic for blockchain interaction, including the `PayoutService` and `KaspaRpc` abstractions.
- **`battle-core`**: Defines the shared domain models, states, and error types.

## Core Services

### 1. Watcher Task (`watcher_task.rs`)

A background loop that polls the Kaspa node for balance changes on active escrow addresses.

- **Deposit Detection**: Automatically transitions matches to `Funded` when both players have contributed.
- **Timeout Refunds**: Identifies matches that stayed in `WaitingForDeposits` too long and triggers automatic refunds.

### 2. Payout Service (`payout.rs`)

Handles the final step of the match cycle.

- **Transaction Building**: Uses `kaspa-consensus-core` types to build real transactions.
- **Signing**: Uses Schnorr signatures with the match-specific private keys.
- **Submission**: Sends the signed `RpcTransaction` to the node.

### 3. Oracle Service

Integrates with external APIs (like FACEIT) to verify match results.

- **Auth Guard**: Uses `X-Oracle-Key` to ensure only authorized resolvers can trigger payouts.
- **Verification**: Validates game IDs and player IDs before resolving.

## API Endpoints

| Method | Endpoint | Description |
|--------|----------|-------------|
| `POST` | `/api/matches` | Create a new match |
| `GET` | `/api/matches/{id}` | Get match status |
| `POST` | `/api/matches/{id}/join` | Join an existing match |
| `POST` | `/api/matches/{id}/resolve` | Resolve a match (Oracle Only) |
| `POST` | `/api/matches/{id}/dispute` | Flag a match for manual review |

## Configuration

The backend is configured via environment variables:

```env
DATABASE_URL=sqlite://matches.db
KASPA_NODE_URL=127.0.0.1:17110
KASPA_NETWORK=testnet-10
ORACLE_API_KEYS=your-secret-key
TREASURY_ADDRESS=kaspatest:q...
```

## Running Locally

1. Navigate to the backend folder: `cd kaspabattle`.
2. Install dependencies: `cargo build`.
3. Run the API: `cargo run -p battle-api`.
4. Run tests: `cargo test --workspace`.

---
[Architecture ←](architecture.md) | [Frontend →](frontend.md)

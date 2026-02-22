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

### 4. Auth Service (`auth.rs`)

Handles user lifecycle and session management.

- **Registration/Login**: Uses Argon2 for secure password hashing.
- **Session Management**: Secure random token generation with expiry tracking.
- **Identity**: Manages user profiles and linked Kaspa addresses.

### 5. FACEIT OAuth Service (`faceit_oauth.rs`)

Integrates with FACEIT Identity Provider.

- **PKCE Flow**: Implements Proof Key for Code Exchange (PKCE) for secure mobile/SPA auth.
- **Token Management**: Handles access/refresh token lifecycle.
- **Profile Linking**: Securely links KaspaBattle users to FACEIT player IDs.

## API Endpoints (v1)

| Method | Endpoint | Description |
| :--- | :--- | :--- |
| `POST` | `/api/v1/auth/register` | Create a new account |
| `POST` | `/api/v1/auth/login` | Authenticate and get session |
| `GET` | `/api/v1/auth/me` | Get current user profile |
| `GET` | `/api/v1/faceit/link` | Start FACEIT OAuth flow |
| `GET` | `/api/v1/faceit/status` | Get linked account status |

## Database Schema

The system uses SQLite (via SQLx) with the following core tables:

- **`users`**: Stores user profiles, emails (hashed), and Kaspa addresses.
- **`sessions`**: Manages active user sessions.
- **`faceit_links`**: Maps internal users to verified FACEIT player identities.
- **`matches`**: Tracks match state, escrow details, and payout status.

## Configuration

The backend is configured via environment variables:

```env
DATABASE_URL=sqlite://matches.db
KASPA_NODE_URL=127.0.0.1:17110
KASPA_NETWORK=testnet-10
ORACLE_API_KEYS=key-alpha,key-beta
FACEIT_CLIENT_ID=your-id
FACEIT_CLIENT_SECRET=your-secret
```

## Running Locally

1. Navigate to the backend folder: `cd kaspabattle`.
2. Install dependencies: `cargo build`.
3. Run the API: `cargo run -p battle-api`.
4. Run tests: `cargo test --workspace`.

---
[Architecture ←](architecture.md) | [Home ↑](../README.md) | [Frontend →](frontend.md)

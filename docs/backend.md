# Backend Documentation

The KaspaBattle backend consists of Rust services for API, domain logic, and Kaspa integration.

## Project Structure

The backend is organized as a Cargo workspace:

- **`battle-api`**: Axum-based REST API server
- **`battle-core`**: Shared Rust library for domain logic
- **`battle-kaspa`**: Kaspa blockchain integration library

## Core Services

### battle-api

**Entry Point:** `main.rs` - Sets up database, routes, starts server.

**Routes:**
- `/api/v1/auth`: Authentication endpoints (register, login, logout, me, set_kaspa_address)
- `/api/v1/faceit`: FACEIT OAuth (link, login, callback)
- `/api/v1/oracle`: Oracle job management (create, get)
- `/api/v1/matches`: Match operations (create, list, deposit, payout)

**Key Services:**
- `AuthService`: Session management
- `FaceitOAuth`: FACEIT integration
- `OracleService`: Result verification

### battle-core

**Modules:**
- `auth`: Authentication services
- `faceit_data`: FACEIT API integration
- `match_state`: Match state machine
- `models`: Domain models (User, Match, etc.)
- `oracle`: Match result verification
- `types`: Common types

**Public APIs:**
- `AuthService`: Handles user authentication
- `MatchStateMachine`: Manages match lifecycle
- `OracleService`: Verifies match results via FACEIT

### battle-kaspa

**Modules:**
- `escrow`: Multisig escrow management
- `rpc`: Kaspa RPC client
- `wallet`: Wallet operations
- `watcher`: Transaction monitoring

**Public APIs:**
- `EscrowManager`: Creates and manages escrow addresses
- `KaspaRpc`: Client for Kaspa node RPC
- `PayoutService`: Handles payout transactions

## API Endpoints

See [API Reference](api/index.md) for detailed endpoint documentation.

## Database Schema

Uses PostgreSQL with core tables for users, sessions, faceit_links, matches.

## Configuration

Environment variables for database, Kaspa node, FACEIT OAuth, admin tokens.

For build and deployment, see [Build & Deployment](build.md).
For testing, see [Tests & Quality](tests.md).

# Core Modules Reference

## rusty-kaspa

Full Kaspa node implementation providing core blockchain functionality.

### Key Modules

#### consensus
Handles block validation, DAG consensus, and transaction processing.

**Public APIs:**
- `ConsensusManager`: Manages consensus state and validation
- `BlockProcessor`: Processes incoming blocks
- `TransactionValidator`: Validates transactions against consensus rules

**Dependencies:** core, database, crypto

#### wallet
Provides wallet functionality for key management and transaction creation.

**Public APIs:**
- `Wallet`: Main wallet interface
- `Account`: Manages addresses and UTXOs
- `TransactionGenerator`: Creates signed transactions

**Dependencies:** consensus, crypto, database

#### rpc
RPC interfaces for node communication.

**Public APIs:**
- `RpcApi` trait: Defines all RPC methods (ping, get_system_info, get_connections, etc.)
- `RpcClient`: Client for connecting to RPC servers
- `RpcServer`: Server implementation for RPC endpoints

**Dependencies:** consensus, wallet, notify

#### kaspad
Main daemon binary that runs the Kaspa node.

**Entry Point:** `main.rs` - Parses args, creates core, runs the node.

**Key Functions:**
- `create_core(args, fd_budget)`: Initializes the node with consensus, wallet, RPC services
- `validate_args(args)`: Validates command-line arguments

## kaspabattle Workspace

### battle-core

Shared domain logic and models.

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

Kaspa blockchain integration.

**Modules:**
- `escrow`: Multisig escrow management
- `rpc`: Kaspa RPC client
- `wallet`: Wallet operations
- `watcher`: Transaction monitoring

**Public APIs:**
- `EscrowManager`: Creates and manages escrow addresses
- `KaspaRpc`: Client for Kaspa node RPC
- `PayoutService`: Handles payout transactions

### battle-api

REST API server using Axum.

**Entry Point:** `main.rs` - Sets up database, routes, starts server.

**Routes:**
- `/api/v1/auth`: Authentication endpoints
- `/api/v1/faceit`: FACEIT OAuth
- `/api/v1/oracle`: Oracle job management
- `/api/v1/matches`: Match operations

**Key Services:**
- `AuthService`: Session management
- `FaceitOAuth`: FACEIT integration
- `OracleService`: Result verification

## battle-frontend

React/TypeScript SPA.

**Key Components:**
- `App.tsx`: Main application component
- `LobbyPage`: Match browsing and creation
- `MatchPage`: Active match interface
- `WalletPage`: Wallet management

**Hooks:**
- `useKaspaInit`: Initializes WASM Kaspa client
- `useWallet`: Wallet state management
- `useBalance`: Balance tracking

**Stores:**
- `useAuthStore`: User authentication state
- `useLobbyStore`: Match lobby state
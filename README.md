# KaspaBattle

[![Kaspa](https://img.shields.io/badge/Blockchain-Kaspa-70cbca)](https://kaspa.org)
[![Rust](https://img.shields.io/badge/Backend-Rust-black?logo=rust)](https://www.rust-lang.org)
[![React](https://img.shields.io/badge/Frontend-React-blue?logo=react)](https://reactjs.org)

KaspaBattle is a decentralized P2P esports wagering platform built on the **Kaspa blockchain**. With our **v1.0 Lobby System**, players can seamlessly create or join challenges, connect their Kaspa wallets, and automatically transition into FACEIT matches to wager KAS in a non-custodial, trustless environment.

By leveraging Kaspa's high-throughput DAG architecture and real-time settlement, KaspaBattle provides a seamless, secure, and instant competitive gaming experience.

## Key Features

- **Full Lobby System (v1.0)**: Live overview of open challenges, easy creation of new matches (BO1-BO3), and seamless joining mechanics.
- **P2P Esports Wagers**: Challenge other players and wager KAS on match outcomes.
- **Non-Custodial Escrow**: Funds are held in per-match escrow addresses via the Kasplex MatchEscrow contract.
- **Automated FACEIT Integration**: Automatic match creation on FACEIT once both players have funded the escrow.
- **Oracle-Based Verification**: Automatic match resolution via authorized Oracles fetching results from FACEIT.
- **Fast Settlements**: Instant payout as soon as the DAG confirms the Oracle resolve transaction.
- **Secure Architecture**: PKCE for OAuth, comprehensive error handling, and multi-layer security protections.
- **Modern UI**: Clean React/TypeScript frontend (Tailwind + shadcn) with integrated Kaspa WASM wallet support.

## Project Structure

- `kaspabattle/`: Rust workspace containing the backend services.
  - `battle-api/`: Actix-Web REST API (v1 / v2 endpoints).
  - `battle-kaspa/`: Kaspa blockchain integration (RPC, Payouts, Watchers).
  - `battle-core/`: Shared models, authentication services, and core match state machine.
- `battle-frontend/`: React + Vite frontend application.
- `docs/`: Comprehensive project documentation.

## Getting Started

### Prerequisites

- **Docker**: Docker Desktop or Docker Engine must be installed and **running**.
- **Rust**: 1.75 or newer.
- **Node.js**: 18.x or newer.
- **Kaspa Node**: Access to a `kaspad` node (Mainnet or Testnet-10) with `--utxoindex`.

### Setup

The easiest way to run KaspaBattle locally is using **Docker**. Ensure Docker Desktop or Docker Engine is running on your machine.

1. **Clone the repository**:

   ```bash
   git clone https://github.com/TimBee706/KaspaBattle2.git
   cd KaspaBattle2
   ```

2. **Start all services with Docker (Recommended)**:

   ```bash
   docker-compose up -d
   ```

   - **Frontend**: Available at `http://localhost:3000`
   - **Backend API**: Available at `http://localhost:8080`
   - **Database**: PostgreSQL runs on port `5432`

---

#### Alternative: Manual Local Development

If you prefer running the app locally for development (you still need a PostgreSQL Database):

1. **Start the Database** (via Docker):

   ```bash
   docker-compose up -d postgres
   ```

2. **Run the Backend**:

   ```bash
   cd kaspabattle
   # Copy .env.example to .env and configure your Kaspa Node URL
   cargo run -p battle-api
   ```

3. **Run the Frontend**:

   ```bash
   cd battle-frontend
   npm install
   npm run dev
   ```

## Documentation

### 🚀 **NEW: [KaspaBattle Lobby Integration v1.0 Guide](docs/KASPA_BATTLE_LOBBY_DOCS.md)**

Comprehensive documentation for the newly released Lobby System, including architecture, frontend/backend flow, Smart Contract states, and deployment instructions. Look here for the ultimate Quickstart!

For more detailed developer information, please refer to the following historical architecture guides:

- [Project Overview](docs/overview.md) — Vision, Problem/Solution, and Match Cycle.
- [Architecture](docs/architecture.md) — Technical stack and data flows.
- [Backend Development](docs/backend.md) — Rust services and API details.
- [Frontend Development](docs/frontend.md) — React, WASM, and UI components.
- [Kaspa Integration](docs/kaspa-integration.md) — How we use the Kaspa SDK.
- [Security](docs/security.md) — Security measures and audit summary.
- [Contributing](docs/contributing.md) — Development guidelines.

---

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

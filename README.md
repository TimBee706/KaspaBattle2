# KaspaBattle

[![Kaspa](https://img.shields.io/badge/Blockchain-Kaspa-70cbca)](https://kaspa.org)
[![Rust](https://img.shields.io/badge/Backend-Rust-black?logo=rust)](https://www.rust-lang.org)
[![React](https://img.shields.io/badge/Frontend-React-blue?logo=react)](https://reactjs.org)

KaspaBattle is a decentralized P2P esports wagering platform built on the **Kaspa blockchain**. It allows players to compete in matches of skill (starting with FACEIT integrated games like CS2) and wager KAS in a non-custodial, trustless environment.

By leveraging Kaspa's high-throughput DAG architecture and real-time settlement, KaspaBattle provides a seamless, secure, and instant competitive gaming experience.

## Key Features

- **P2P Esports Wagers**: Challenge other players and wager KAS on match outcomes.
- **Non-Custodial Escrow**: Funds are held in per-match escrow addresses. You keep control of your keys.
- **User Accounts & FACEIT Linking**: Secure account system with FACEIT OAuth integration for verified player identities.
- **Oracle-Based Verification**: Automatic match resolution via authorized Oracles (FACEIT API integration).
- **Fast Settlements**: Instant payout as soon as the DAG confirms the Oracle resolve transaction.
- **Secure Architecture**: Argon2 password hashing, PKCE for OAuth, and multi-layer security protections.
- **Modern UI**: Clean React/TypeScript frontend with integrated Kaspa WASM wallet support.

## Project Structure

- `kaspabattle/`: Rust workspace containing the backend services.
  - `battle-api/`: Actix-Web REST API (v1 / v2 endpoints).
  - `battle-kaspa/`: Kaspa blockchain integration (RPC, Payouts, Watchers).
  - `battle-core/`: Shared models, authentication services, and core match state machine.
- `battle-frontend/`: React + Vite frontend application.
- `docs/`: Comprehensive project documentation.

## Getting Started

### Prerequisites

- **Rust**: 1.75 or newer.
- **Node.js**: 18.x or newer.
- **Kaspa Node**: Access to a `kaspad` node (Mainnet or Testnet-10) with `--utxoindex`.

### Setup

1. **Clone the repository**:

   ```bash
   git clone https://github.com/TimBee706/KaspaBattle2.git
   cd KaspaBattle2
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

For more detailed information, please refer to the following guide:

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

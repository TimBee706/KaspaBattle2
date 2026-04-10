# KaspaBattle

[![Kaspa](https://img.shields.io/badge/Blockchain-Kaspa-70cbca)](https://kaspa.org)
[![Rust](https://img.shields.io/badge/Backend-Rust-black?logo=rust)](https://www.rust-lang.org)
[![React](https://img.shields.io/badge/Frontend-React-blue?logo=react)](https://reactjs.org)
[![CI](https://github.com/TimBee706/KaspaBattle2/actions/workflows/ci.yml/badge.svg)](https://github.com/TimBee706/KaspaBattle2/actions)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)

> **Decentralized P2P esports wagering on the Kaspa blockchain.**
> KaspaBattle lets competitive gamers challenge each other, wager KAS in a non-custodial escrow, and receive instant payouts — all powered by Kaspa's 10-blocks-per-second DAG and the [kdapp](https://github.com/michaelsutton/kdapp) Episode framework.

---

## ✨ Features

| | Feature | Description |
|---|---------|-------------|
| 🎮 | **Live Lobby System** | Browse open challenges, create matches (BO1 / BO3), join with one click |
| 💰 | **P2P Wagers** | Stake KAS on match outcomes — 10 to 10,000 KAS per match |
| 🔒 | **Non-Custodial Escrow** | Funds are held in per-match multisig escrow addresses on-chain |
| 🤖 | **Automated FACEIT** | Matches are created and tracked on FACEIT automatically |
| ⚡ | **Oracle Verification** | Results are verified through authorized Oracles fetching FACEIT data |
| 🚀 | **Instant Settlement** | Payout lands in your wallet as soon as the DAG confirms |
| 🌐 | **Multi-Game Support** | Counter-Strike 2, Valorant, Rocket League, Dota 2, League of Legends |
| 🔐 | **Secure Auth** | FACEIT OAuth with PKCE, session management, rate limiting |

---

## 🏗️ Architecture

KaspaBattle is a three-layer system: a **React frontend** with an integrated Kaspa WASM wallet, a **Rust backend** handling API, authentication, and match orchestration, and the **Kaspa blockchain** providing consensus, settlement, and escrow.

```
┌─────────────────────────────────────────────────────┐
│                   Frontend Layer                     │
│  ┌──────────────┐   ┌──────────────────────────┐    │
│  │  React SPA   │   │  Kaspa WASM SDK (Wallet)  │   │
│  └──────┬───────┘   └────────────┬─────────────┘    │
└─────────┼────────────────────────┼──────────────────┘
          │ REST / WS              │ wRPC
┌─────────┼────────────────────────┼──────────────────┐
│         ▼        Backend Layer   │                   │
│  ┌──────────────┐  ┌───────────────┐  ┌──────────┐  │
│  │  battle-api  │  │ battle-kaspa  │  │  Oracle   │  │
│  │  (Axum REST) │  │ (RPC/Escrow)  │  │ (FACEIT)  │  │
│  └──────┬───────┘  └───────┬───────┘  └─────┬────┘  │
│         │                  │                 │       │
│         ▼                  │                 │       │
│  ┌──────────────┐          │                 │       │
│  │ battle-core  ├──────────┘                 │       │
│  │ (State/Models)                            │       │
│  └──────┬───────┘                            │       │
│         │         ┌──────────────────────┐   │       │
│         └────────►│     PostgreSQL       │   │       │
│                   └──────────────────────┘   │       │
└──────────────────────────┬───────────────────┘       │
                           │ wRPC/gRPC                 │
┌──────────────────────────┼───────────────────────────┘
│        Blockchain Layer  ▼                   │
│  ┌───────────┐  ┌──────────┐  ┌──────────┐  │
│  │  kaspad    │  │ Consensus│  │  Wallet  │  │
│  │  (Node)   │  │ (GhostDAG)│ │  (UTXO)  │  │
│  └───────────┘  └──────────┘  └──────────┘  │
└──────────────────────────────────────────────┘
```

→ Full details in [Architecture Overview](docs/01-ARCHITECTURE.md)

---

## 🚀 Quick Start

### Docker (Recommended)

```bash
git clone https://github.com/TimBee706/KaspaBattle2.git
cd KaspaBattle2
docker-compose up -d
```

- **Frontend**: <http://localhost:3000>
- **Backend API**: <http://localhost:8080>
- **Database**: PostgreSQL on port 5432

### Manual Development Setup

```bash
# 1. Start database
docker-compose up -d postgres

# 2. Backend
cd kaspabattle
#cp .env.example .env   # configure Kaspa Node URL
cargo run -p battle-api

# 3. Frontend (new terminal)
cd battle-frontend
npm install
npm run dev
```

---

## 📚 Documentation

| Document | Description |
|----------|-------------|
| [Architecture Overview](docs/01-ARCHITECTURE.md) | System architecture, crate layout, data flow |
| [Kaspa Integration](docs/02-KASPA-INTEGRATION.md) | Kaspa RPC, UTXO model, wallet integration |
| [Security Model](docs/03-SECURITY.md) | Trust model, threat analysis, escrow security |
| [KaspaBattle Integration](docs/06-KASPA-BATTLE-INTEGRATION.md) | Wager flow, Oracle, payout, FACEIT integration |
| [API Reference](docs/07-API-REFERENCE.md) | REST endpoints, WebSocket events, data models |
| [Contributing](docs/08-CONTRIBUTING.md) | Development setup, code style, PR process |

---

## 🎮 Supported Games

| Game | Platform | Status |
|------|----------|--------|
| Counter-Strike 2 | FACEIT | ✅ Live |
| Valorant | FACEIT | ✅ Live |
| Rocket League | FACEIT | ✅ Live |
| Dota 2 | FACEIT | ✅ Live |
| League of Legends | FACEIT | ✅ Live |

→ Details in [KaspaBattle Integration Guide](docs/06-KASPA-BATTLE-INTEGRATION.md)

---

## 🔐 Security

KaspaBattle uses a layered security model:

- **Escrow**: Per-match deterministic escrow addresses derived from match ID + player public keys
- **Authentication**: FACEIT OAuth 2.0 with PKCE, HTTP-only session cookies
- **Oracle**: Authorized backend Oracles verify match results via FACEIT API
- **State Machine**: Pure-function match state transitions with exhaustive test coverage

> ⚠️ **Current status**: Off-chain escrow with server-held keys (custodial). Migration to trustless on-chain escrow (multisig UTXO / kdapp Episodes) is in progress. See the [Security Model](docs/03-SECURITY.md) for the full threat analysis and roadmap.

---

## 📋 Requirements

| Requirement | Version |
|-------------|---------|
| Rust | 1.83+ |
| Node.js | 18+ |
| Docker | 20+ (recommended) |
| PostgreSQL | 15+ |
| Kaspa Node | rusty-kaspa v1.0.0+ with `--utxoindex` |

---

## 🛠️ Project Structure

```
KaspaBattle2/
├── kaspabattle/              # Rust backend workspace
│   ├── battle-api/           # Axum REST API + WebSocket server
│   ├── battle-core/          # Domain models, state machine, constants
│   ├── battle-kaspa/         # Kaspa RPC, escrow, wallet, multisig, Oracle
│   ├── battle-kdapp/         # kdapp Episode integration (WIP)
│   ├── migrations/           # SQL migrations
│   └── tests/                # Integration tests
├── battle-frontend/          # React + Vite + TypeScript frontend
│   ├── src/
│   │   ├── api/              # API client, auth, FACEIT
│   │   ├── components/       # UI components
│   │   ├── config/           # Constants, game definitions
│   │   ├── kaspa/            # WASM wallet integration
│   │   ├── stores/           # Zustand state management
│   │   └── pages/            # Route pages
│   └── public/               # Static assets, game logos
├── docs/                     # Project documentation
├── docker-compose.yml        # Development environment
└── Caddyfile.example         # Production reverse proxy config
```

---

## 🤝 Contributing

We welcome contributions! See the [Contributing Guide](docs/08-CONTRIBUTING.md) for:

- Development environment setup
- Code style (rustfmt, clippy, ESLint)
- Branching strategy and PR process
- Testing guidelines

---

## 📄 License

This project is licensed under the MIT License — see the [LICENSE](LICENSE) file for details.

The [kdapp framework](https://github.com/michaelsutton/kdapp) is licensed under the ISC License.

# Architecture Overview

This document describes the system architecture of KaspaBattle and how it integrates with the kdapp framework and the Kaspa blockchain.

---

## Table of Contents

- [High-Level Architecture](#high-level-architecture)
- [Workspace Structure](#workspace-structure)
- [The kdapp Framework](#the-kdapp-framework)
- [Kaspa UTXO Model as State Machine](#kaspa-utxo-model-as-state-machine)
- [Data Flow](#data-flow)
- [Component Interaction](#component-interaction)
- [Async Architecture](#async-architecture)
- [Comparison with Other Frameworks](#comparison-with-other-frameworks)

---

## High-Level Architecture

KaspaBattle is organized into three logical layers, each with clearly defined responsibilities.

```
┌──────────────────────────────────────────────────────────────────────┐
│                         FRONTEND LAYER                               │
│                                                                      │
│   React SPA (Vite + TypeScript)                                      │
│   ┌──────────────┐  ┌────────────────┐  ┌────────────────────────┐   │
│   │  Pages       │  │  Zustand       │  │  Kaspa WASM SDK        │   │
│   │  (Lobby,     │  │  Stores        │  │  (wallet, signing,     │   │
│   │   Match,     │  │  (Auth, Lobby, │  │   RPC connection)      │   │
│   │   Landing)   │  │   Match)       │  │                        │   │
│   └──────┬───────┘  └───────┬────────┘  └───────────┬────────────┘   │
│          │                  │                        │                │
└──────────┼──────────────────┼────────────────────────┼────────────────┘
           │ HTTP/WS          │                        │ wRPC
           ▼                  ▼                        ▼
┌──────────────────────────────────────────────────────────────────────┐
│                         BACKEND LAYER                                │
│                                                                      │
│   Rust Workspace (Axum + Tokio)                                      │
│   ┌──────────────┐  ┌────────────────┐  ┌────────────────────────┐   │
│   │  battle-api   │  │  battle-core   │  │  battle-kaspa          │   │
│   │  ─────────── │  │  ────────────  │  │  ────────────────────  │   │
│   │  REST routes  │  │  MatchState    │  │  KaspaBackend (RPC)    │   │
│   │  Auth guard   │  │  State machine │  │  EscrowService         │   │
│   │  WebSocket    │  │  Constants     │  │  EscrowWallet          │   │
│   │  Session mgmt │  │  Error types   │  │  Multisig service      │   │
│   │  FACEIT OAuth │  │  Models        │  │  PayoutService         │   │
│   └──────┬───────┘  └───────┬────────┘  │  Oracle                │   │
│          │                  │            │  FACEIT Watcher         │   │
│          │                  │            └───────────┬────────────┘   │
│          ▼                  │                        │                │
│   ┌──────────────┐          │                        │                │
│   │  PostgreSQL  │◄─────────┘                        │                │
│   └──────────────┘                                   │                │
└──────────────────────────────────────────────────────┼────────────────┘
                                                       │ wRPC / gRPC
                                                       ▼
┌──────────────────────────────────────────────────────────────────────┐
│                       BLOCKCHAIN LAYER                               │
│                                                                      │
│   Kaspa Node (rusty-kaspa)                                           │
│   ┌──────────────┐  ┌───────────────┐  ┌──────────────────────────┐  │
│   │  kaspad       │  │  GhostDAG     │  │  UTXO Index             │  │
│   │  (P2P node)   │  │  Consensus    │  │  (balance queries)      │  │
│   └──────────────┘  └───────────────┘  └──────────────────────────┘  │
└──────────────────────────────────────────────────────────────────────┘
```

---

## Workspace Structure

### Rust Backend — `kaspabattle/`

The backend is a Cargo workspace with four crates:

| Crate | Role | Key Dependencies |
|-------|------|-----------------|
| `battle-api` | HTTP server, routing, auth, WebSocket | axum, tower-http, sqlx |
| `battle-core` | Domain models, state machine, constants | serde (no I/O) |
| `battle-kaspa` | Kaspa RPC client, escrow, wallet, multisig, Oracle | kaspa-wrpc-client, secp256k1 |
| `battle-kdapp` | kdapp Episode integration (WIP) | kdapp |

**Design principle**: `battle-core` has zero I/O dependencies. All state transitions are pure functions that take a current state and an action, and return a new state or an error. This makes the core logic fully testable without mocks.

### Frontend — `battle-frontend/`

| Directory | Purpose |
|-----------|---------|
| `api/` | Axios HTTP client, auth helpers, FACEIT API |
| `components/` | Reusable UI components (auth, match, wallet, layout) |
| `config/` | Constants, supported games, environment config |
| `kaspa/` | WASM SDK integration for in-browser wallet |
| `stores/` | Zustand stores for auth, lobby, match state |
| `pages/` | Route-level page components |
| `locales/` | i18n translations (English, German) |

---

## The kdapp Framework

[kdapp](https://github.com/michaelsutton/kdapp) is a framework for building interactive decentralized applications on the Kaspa blockDAG. Its core abstraction is the **Episode** — a time-bounded, state-driven session that lives on-chain.

### Core Components

```
┌──────────────┐     ┌──────────────┐     ┌──────────────┐     ┌──────────────┐
│  Generator   │────▶│    Proxy     │────▶│   Engine     │────▶│   Episode    │
│              │     │              │     │              │     │              │
│  Crafts TXs  │     │  Listens to  │     │  Manages     │     │  Your app    │
│  with pattern│     │  Kaspa DAG   │     │  lifecycle   │     │  logic       │
│  matching    │     │  for matching │     │  + rollbacks │     │              │
│              │     │  TXs         │     │              │     │              │
└──────────────┘     └──────────────┘     └──────────────┘     └──────────────┘
```

1. **Generator** — Creates Kaspa transactions with serialized commands as payload. Iterates a nonce until the transaction ID matches a predefined bit pattern, enabling efficient discovery on the network.

2. **Proxy** — A wRPC client that polls the Kaspa DAG's virtual selected parent chain (VSPC). It filters transactions by pattern, fetches their payloads from merged blocks, and forwards them to the Engine.

3. **Engine** — Manages multiple Episode instances of the same type. Deserializes commands, verifies signatures, executes state transitions, and maintains a rollback stack for DAG reorganizations.

4. **Episode** (trait) — The developer interface. You implement this trait to define your application's state, command logic, and rollback behavior.

5. **EpisodeEventHandler** (trait) — A callback interface for reacting to Episode lifecycle events (initialization, command execution, rollback).

### How KaspaBattle Uses kdapp

KaspaBattle extends the kdapp pattern by adding:

- **Off-chain escrow** (current) — Server-managed deterministic escrow addresses
- **Oracle integration** — FACEIT API as an external data source for match results
- **Multisig escrow** (in progress) — 2-of-2 UTXO scripts for trustless fund custody
- **Episode-based wagers** (planned) — Full kdapp Episode implementation for the wager lifecycle

→ See [Episode Framework](04-EPISODE-FRAMEWORK.md) for the full developer guide.

---

## Kaspa UTXO Model as State Machine

Kaspa uses an unspent transaction output (UTXO) model, similar to Bitcoin. Each UTXO is a "coin" with:

- A **value** (amount in sompi; 1 KAS = 100,000,000 sompi)
- A **script** (spending condition, typically a public key hash)
- A **spent/unspent** status

KaspaBattle treats the UTXO set as a state machine:

```
State 0: Escrow UTXO created (empty)
   │
   ▼  Player A sends wager
State 1: Escrow has 1 UTXO (partial deposit)
   │
   ▼  Player B sends wager
State 2: Escrow has 2+ UTXOs (complete deposit, match locked)
   │
   ▼  Oracle resolves winner
State 3: Payout TX created (escrow UTXOs consumed, winner + platform outputs)
   │
   ▼  TX confirmed in DAG
State 4: Settled (escrow empty, winner received funds)
```

Unlike account-based blockchains (Ethereum, Solana), there is no persistent on-chain state. State is derived from the UTXO set at any point in time. This is why the kdapp Engine maintains in-memory state with rollback support.

---

## Data Flow

### Match Lifecycle — End to End

```
Player A                    Backend                    Kaspa DAG
   │                          │                          │
   │── Create Challenge ─────▶│                          │
   │                          │── Create Escrow ────────▶│
   │◄── Escrow Address ──────│                          │
   │                          │                          │
   │── Send Wager TX ────────────────────────────────────▶│
   │                          │◄── UTXO Notification ────│
   │                          │── Update Deposit State   │
   │                          │                          │
Player B                      │                          │
   │── Join Challenge ───────▶│                          │
   │── Send Wager TX ────────────────────────────────────▶│
   │                          │◄── UTXO Notification ────│
   │                          │── Both Deposited → Lock  │
   │                          │                          │
   │                          │── Create FACEIT Match    │
   │                          │                          │
   │ ─ ─ ─ Players play on FACEIT ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ │
   │                          │                          │
   │                          │── Oracle: Poll FACEIT ──▶│
   │                          │◄── Match Result ─────────│
   │                          │                          │
   │                          │── Build Payout TX ──────▶│
   │                          │                          │
Winner                        │◄── TX Confirmed ─────────│
   │◄── Funds in Wallet ─────│                          │
```

### Kaspa Transaction Flow (kdapp Level)

```
Developer App           Kaspa Network          kdapp Engine
     │                       │                       │
     │── build_command_tx() ─│                       │
     │   (nonce until        │                       │
     │    pattern match)     │                       │
     │                       │                       │
     │── submit_transaction()│                       │
     │                       │                       │
     │                       │── Block accepted ────▶│
     │                       │                       │── Proxy filters
     │                       │                       │   by pattern
     │                       │                       │
     │                       │                       │── Deserialize
     │                       │                       │   payload
     │                       │                       │
     │                       │                       │── Engine.execute()
     │                       │                       │   (verify sig,
     │                       │                       │    apply command,
     │                       │                       │    push rollback)
     │                       │                       │
     │                       │                       │── EventHandler
     │                       │                       │   .on_command()
```

---

## Component Interaction

### Backend Crate Dependencies

```
battle-api
    ├── battle-core    (models, state machine)
    ├── battle-kaspa   (blockchain interaction)
    └── battle-kdapp   (Episode integration — WIP)

battle-kaspa
    ├── battle-core    (shared types, constants)
    ├── kaspa-wrpc-client
    ├── kaspa-consensus-core
    └── kaspa-addresses

battle-core
    └── (no internal deps — pure domain logic)
```

### Key Interfaces

| Interface | Crate | Role |
|-----------|-------|------|
| `KaspaBackend` (trait) | battle-kaspa | Abstracts RPC calls (get_balance, get_utxos, submit_tx) |
| `EscrowService` | battle-kaspa | Escrow address derivation, deposit tracking, payout |
| `MultisigEscrowService` | battle-kaspa | 2-of-2 multisig escrow (production path) |
| `MatchState` + `transition()` | battle-core | Pure state machine for match lifecycle |
| `FaceitClient` | battle-core | FACEIT API wrapper for match data |
| `OracleService` | battle-kaspa | Monitors FACEIT matches, triggers resolution |

---

## Async Architecture

### Runtime

KaspaBattle uses **Tokio** as its async runtime. The backend is fully async with:

- **Axum** handlers for HTTP request processing
- **SQLx** for async PostgreSQL queries
- **Tokio tasks** for background workers (Oracle, FACEIT Watcher, Payout Worker)
- **Broadcast channels** for WebSocket event distribution

### Thread Model

```
Main Thread (Tokio)
    │
    ├── Axum HTTP Server (multi-threaded)
    │   ├── Route handlers (async)
    │   └── WebSocket connections (per-client tasks)
    │
    ├── Oracle Worker (spawned task)
    │   └── Polls FACEIT API for match results
    │
    ├── FACEIT Watcher (spawned task)
    │   └── Monitors active FACEIT matches
    │
    ├── Payout Worker (spawned task)
    │   └── Processes resolved matches → builds + submits payout TXs
    │
    └── Kaspa RPC Client (persistent connection)
        └── UTXO monitoring, balance queries, TX submission
```

### kdapp Thread Model

The kdapp framework uses a slightly different concurrency model:

- **Proxy** runs as an async Tokio task (polling the DAG)
- **Engine** runs on a blocking thread (`spawn_blocking`) because it uses `std::sync::mpsc`
- Communication between Proxy and Engine is via `std::sync::mpsc::Sender<EngineMsg>`
- Communication from Engine to application is via `tokio::sync::mpsc::UnboundedSender`

---

## Comparison with Other Frameworks

| Feature | kdapp (Kaspa) | Anchor (Solana) | Ink! (Polkadot) |
|---------|---------------|-----------------|-----------------|
| **State model** | Off-chain + UTXO rollbacks | On-chain accounts | On-chain storage |
| **Execution** | Node-side engine | Validator execution | Parachain runtime |
| **Speed** | ~10 blocks/sec (DAG) | ~2.5 blocks/sec | ~6 sec/block |
| **Smart contracts** | TX payload commands | Program instructions | Wasm contracts |
| **Rollback handling** | Built-in (DAG reorgs) | Not needed (finality) | Via parachain finality |
| **Developer interface** | `Episode` trait (Rust) | `#[program]` macro | `#[ink::contract]` |
| **Maturity** | Alpha (experimental) | Production | Production |

**Key difference**: kdapp does not have a general-purpose smart contract VM. Instead, it uses transaction payloads as serialized commands and interprets them in an off-chain engine with DAG-aware rollback support. This trades on-chain verifiability for development speed and flexibility.

---

## Next Steps

- [Kaspa Integration Guide](02-KASPA-INTEGRATION.md) — How the backend connects to Kaspa
- [Episode Framework](04-EPISODE-FRAMEWORK.md) — Building applications with the Episode trait
- [Security Model](03-SECURITY.md) — Trust assumptions and threat analysis

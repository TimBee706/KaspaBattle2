# Project Overview

KaspaBattle is a decentralized P2P esports wagering platform built on the Kaspa blockchain. It enables players to create and join challenges, wager KAS on match outcomes, and receive instant payouts in a non-custodial, trustless environment.

## Key Features

- **Full Lobby System**: Live overview of open challenges, easy creation and joining of matches.
- **P2P Esports Wagers**: Challenge other players and wager KAS on FACEIT match outcomes.
- **Non-Custodial Escrow**: Funds held in per-match escrow addresses via multisig contracts.
- **Automated FACEIT Integration**: Automatic match creation and result verification.
- **Oracle-Based Resolution**: Match results fetched from FACEIT API with verification.
- **Fast Settlements**: Instant payouts using Kaspa's high-throughput DAG.
- **Secure Architecture**: PKCE OAuth, comprehensive error handling, multi-layer security.

## Technology Stack

- **Frontend**: React 18, Vite, TypeScript, TailwindCSS, Kaspa WASM SDK
- **Backend**: Rust (Axum, PostgreSQL, battle-core, battle-kaspa)
- **Blockchain**: Kaspa (rusty-kaspa node, RPC, wallet)
- **External**: FACEIT API for player data and match results

## Project Structure

- `kaspabattle/`: Rust workspace (battle-api, battle-core, battle-kaspa)
- `battle-frontend/`: React SPA
- `rusty-kaspa/`: Full Kaspa node implementation
- `docs/`: Technical documentation

For detailed architecture, see [Architecture](architecture.md).
For getting started, see the [README](../README.md).

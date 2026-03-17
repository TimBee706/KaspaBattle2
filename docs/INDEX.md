# KaspaBattle Technical Documentation

## Overview

KaspaBattle is a competitive gaming platform that integrates with the Kaspa blockchain for secure wagering on matches. The system uses FACEIT for player authentication and match verification, implementing multisig escrow contracts for trustless betting.

The system consists of three main layers:

1. **Frontend Layer**: React SPA with Kaspa WASM integration
2. **Backend Layer**: Rust-based API server with domain logic and Kaspa integration
3. **Blockchain Layer**: Kaspa node with consensus, wallet, and RPC services

#### Main Components

- **battle-frontend**: React/TypeScript application with WASM Kaspa integration
- **battle-api**: Axum-based REST API server
- **battle-core**: Shared Rust library for domain logic
- **battle-kaspa**: Kaspa blockchain integration library
- **rusty-kaspa**: Full Kaspa node implementation in Rust

#### External Dependencies

- Kaspa blockchain network
- FACEIT API for player data and match results
- PostgreSQL database for application state

## Documentation Sections

- [Module Reference](modules/core.md) — Detailed reference for all major modules and crates.
- [API Reference](api/index.md) — REST API endpoints, RPC methods, and CLI commands.
- [Data Models](models/index.md) — Structs, enums, and data structures.
- [Use Cases & Flows](flows/index.md) — Typical user journeys and system processes.
- [Build & Deployment](build.md) — Setup, configuration, and deployment instructions.
- [Tests & Quality](tests.md) — Testing structure and quality assurance.

## Additional Documentation

- [Architecture](architecture.md) — High-level system architecture.
- [Backend](backend.md) — Backend services and API details.
- [Frontend](frontend.md) — Frontend components and structure.
- [Overview](overview.md) — Project overview and features.
- [Contributing](contributing.md) — Development guidelines.
- [Whitepaper](Whitepaper.md) — Original technical foundation.

## Module Reference

- [Core Modules](modules/core.md)
- [API Reference](api/index.md)
- [Data Models](models/index.md)
- [Use Cases & Flows](flows/index.md)
- [Build & Deployment](build.md)
- [Tests](tests.md)

## Quick Start

1. Start the backend services with Docker Compose
2. Build and run the frontend
3. Connect to a Kaspa node for blockchain operations

See [Build & Deployment](build.md) for detailed instructions.
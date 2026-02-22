# Architecture

KaspaBattle follows a layered architecture designed to separate concerns between user interaction, business logic, and blockchain state.

## High-Level Overview

The system consists of three primary layers:

1. **Client Layer (Frontend)**: A React-based Single Page Application (SPA).
2. **Logic Layer (Backend API & Services)**: A Rust-based backend powered by Actix-Web.
3. **Blockchain Layer (Kaspa)**: The underlying DAG for value transfer and state finality.

## Component Diagram

```mermaid
graph TD
    User((Player))
    
    subgraph Frontend [Client Layer]
        ReactApp[React/TypeScript SPA]
        KaspaWASM[Kaspa WASM SDK]
    end
    
    subgraph Backend [Logic Layer]
        ActixAPI[Actix-Web API]
        Watcher[Deposit Watcher Service]
        Payout[Payout Service]
        Oracle[Oracle API & Fed]
        DB[(SQLite / SQLx)]
    end
    
    subgraph Blockchain [Blockchain Layer]
        KaspaNode[Kaspa Node - kaspad]
        EscrowAddr[Escrow Addresses]
    end

    User <--> ReactApp
    ReactApp <--> ActixAPI
    ReactApp -- Sign/Inquire --> KaspaWASM
    KaspaWASM -- wRPC --> KaspaNode
    
    ActixAPI <--> DB
    Watcher -- Polling --> KaspaNode
    Watcher -- State Updates --> DB
    Payout -- Schnorr Sign --> EscrowAddr
    Payout -- RPC Submit --> KaspaNode
    Oracle -- Fetch Result --> GameAPIs[FACEIT/API]
```

## Data Flows

### Match Creation & Deposit

1. **Frontend** requests match creation.
2. **Backend** generates a unique keypair (using the master escrow seed) and records the address in the **DB**.
3. **Frontend** displays the address. Player sends KAS.
4. **Watcher Service** polls the Kaspa Node via RPC. When UTXOs are found for the address, it updates the match state in the DB.

### Oracle & Payout

1. **Oracle Service** monitors the game status.
2. Once the match finishes, it fetches the winner ID from FACEIT.
3. **Payout Service** retrieves the escrow private key from the secure store.
4. It builds a transaction splitting the balance:
   - **Winner**: 95%
   - **Treasury**: 5%
5. The transaction is signed via Schnorr and submitted to the **Kaspa RPC**.

## Technology Stack

- **Frontend**: React 18, Vite, TypeScript, TailwindCSS.
- **Web3 Interface**: Kaspa WASM SDK (providing wRPC and Wallet types).
- **Backend**: Rust (Edition 2021).
  - **API Framework**: Actix-Web 4.
  - **Database**: SQLite with SQLx (async).
  - **Kaspa SDK**: `rusty-kaspa`, `kaspa-rpc-core`, `kaspa-wallet-core`.
- **Infrastructure**: Dockerized services, kaspad node (Testnet-10).

---
[Overview ←](overview.md) | [Backend →](backend.md)

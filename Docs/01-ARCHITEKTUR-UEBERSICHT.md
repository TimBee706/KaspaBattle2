# Rusty-Kaspa: Architektur-Übersicht

**Version:** 1.0  
**Datum:** Februar 2026  
**Repository:** https://github.com/kaspanet/rusty-kaspa  

---

## Was ist Rusty-Kaspa?

Rusty-Kaspa ist eine vollständige Rust-Implementierung des Kaspa-Blockchain-Netzwerks und fungiert als Drop-in-Replacement zur ursprünglichen Golang-Implementierung. Das Projekt besteht aus mehreren Kernkomponenten:

- **Kaspad**: Vollständiger Node für das Kaspa-Netzwerk
- **Wallet-Framework**: Multi-Platform-Wallet mit Key-Management und TX-Handling
- **RPC-Stack**: WebSocket und gRPC basierte APIs für Node-Interaktion
- **WASM-Bindings**: JavaScript/TypeScript-Unterstützung für Web und Node.js
- **Brücken-Services**: Mining-Brücken, Indexer und Hilfskomponenten

---

## Workspace-Struktur

Das Repository ist als **Cargo Workspace** organisiert mit über 50 Crates:

```
rusty-kaspa/
├── kaspad/                          # Full-Node Binary
├── wallet/                          # Wallet-Framework
│   ├── core/                        # Wallet-Kern (Keys, UTXO, TX-Gen)
│   ├── keys/                        # Key-Management & BIP32
│   ├── native/                      # Native CLI Wallet
│   ├── wasm/                        # WASM Wallet API
│   ├── bip32/                       # BIP32 Derivation
│   ├── pskt/                        # Partially Signed Kaspa Transactions
│   └── macros/                      # Derive-Makros
├── rpc/                             # RPC Subsystem
│   ├── core/                        # RPC-Definitionen & Types
│   ├── service/                     # RPC-Service Implementierung
│   ├── grpc/                        # gRPC Transport
│   └── wrpc/                        # WebSocket RPC (Borsh/JSON)
├── wasm/                            # WASM32 SDK & Web
│   ├── core/                        # Core WASM Bindings
│   ├── web/                         # Browser-Demo & Doku
│   ├── nodejs/                      # Node.js Examples
│   └── examples/                    # JS/TS Beispiele
├── consensus/                       # Consensus-Engine
├── crypto/                          # Kryptographische Primitiven
├── indexes/                         # UTXO-Index & Processor
├── mining/                          # Mining-Framework
├── bridge/                          # Stratum-Mining-Bridge
└── [weitere Komponenten]            # Database, Networking, etc.
```

---

## High-Level-Architektur

```
┌─────────────────────────────────────────────────────────────┐
│                      Frontend-Layer                          │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐       │
│  │  Web-Client  │  │  Node.js App │  │ Tauri/Mobile │       │
│  └──────┬───────┘  └──────┬───────┘  └──────┬───────┘       │
└─────────┼────────────────┼────────────────┼────────────────┘
          │                │                │
    ┌─────▼────────────────▼────────────────▼─────┐
    │   WASM32 SDK (kaspa-wasm / @kasdk)          │
    │  ├─ RPC Client (WebSocket/Borsh)            │
    │  ├─ Wallet Core (Key, TX, UTXO)             │
    │  └─ Wallet API (Account, Storage)           │
    └─────────────┬────────────────────────────────┘
                  │ WebSocket (Borsh/JSON) oder
                  │ HTTPS/gRPC
    ┌─────────────▼────────────────────────────────┐
    │          Kaspad Full Node                     │
    │  ┌──────────────────────────────────────┐    │
    │  │      RPC Server (wRPC/gRPC)          │    │
    │  │  ├─ Chain Data Methods                │    │
    │  │  ├─ TX Broadcasting                   │    │
    │  │  ├─ Subscription/Notifications        │    │
    │  │  └─ Query Methods                     │    │
    │  ├──────────────────────────────────────┤    │
    │  │   Core Engine                         │    │
    │  │  ├─ Consensus Manager                 │    │
    │  │  ├─ P2P Network                       │    │
    │  │  ├─ Mempool                           │    │
    │  │  ├─ Block DAG                         │    │
    │  │  └─ Database (RocksDB)                │    │
    │  └──────────────────────────────────────┘    │
    └─────────────┬────────────────────────────────┘
                  │
         P2P Network
         (andere Nodes)
```

---

## Komponenten-Überblick

### 1. Kaspad Node
Das Herzstück des Netzwerks - ein vollständiger Kaspa-Node, der Folgendes bereitstellt:
- **Full Block DAG Validation**: Validierung aller Blöcke gegen Consensus-Regeln
- **Mempool Management**: TX-Pool-Verwaltung und Prioritäten
- **P2P Networking**: Kommunikation mit anderen Nodes
- **RPC API**: Schnittstellen für externe Clients (wRPC, gRPC)

### 2. RPC-Stack (wRPC & gRPC)
Mehrschichtige RPC-Architektur:
- **wRPC**: Leichtgewichtiges WebSocket RPC (empfohlen für Browser/Web)
- **gRPC**: High-Performance RPC über HTTP/2
- 150+ RPC-Methoden für Chain-Daten, TX, Subscriptions

### 3. Wallet Framework
Multi-Platform Wallet mit:
- **HD Key Derivation** (BIP32/BIP44)
- **UTXO Management**: Scanning und Tracking
- **TX Generator**: Automatische UTXO-Auswahl und Fee-Kalkulation
- **Storage Backends**: Filesystem, LocalStorage (Browser), IndexedDB

### 4. WASM SDK
Browser & Node.js Integration:
- **RPC Client**: WebSocket-Zugriff auf Kaspad
- **Wallet API**: Vollständige Wallet-Funktionalität in JavaScript/TypeScript
- **Performance**: Borsh-Serialisierung für Geschwindigkeit

### 5. Infrastructure Components
- **UTXO Indexer**: Schnelle UTXO-Queries (< 10ms)
- **Mining Bridge**: Stratum-Protokoll Support für Mining-Pools
- **Consensus Engine**: Block DAG Validierung

---

## Architektur-Patterns

### Schichtenprinzip

Die Architektur folgt einem **4-Schichten-Modell**:

```
┌────────────────────────────────────┐
│  4. High-Level API                 │
│  (WalletApi, RpcApi Traits)        │
├────────────────────────────────────┤
│  3. Service Implementation         │
│  (RpcCoreService, Wallet)          │
├────────────────────────────────────┤
│  2. Transports                     │
│  (wRPC, gRPC, Wallet Storage)      │
├────────────────────────────────────┤
│  1. Low-Level Primitives           │
│  (Keys, Addresses, Hashes, Crypto) │
└────────────────────────────────────┘
```

### Trait-basierte Abstraktion

Das System nutzt Rust Traits für flexible Implementierungen:

```rust
// Abstraktion für RPC-Implementierungen
#[async_trait]
pub trait RpcApi: Sync + Send {
    async fn get_info(&self) -> RpcResult<GetInfoResponse>;
    async fn submit_transaction(&self, tx: Transaction) -> RpcResult<SubmitTransactionResponse>;
    // ...150+ weitere Methoden
}

// Mehrere Implementierungen möglich:
// - wRPC Client
// - gRPC Client
// - In-Process (für Tests)
```

---

## Data Flow Beispiel: Simple Transaction

```
User-Code
    │
    ├─ wallet.send(address, amount)
    │
    ├─ select_utxos(amount)  ◄─── Wallet sucht genug UTXO
    │
    ├─ generate_transaction()  ◄─── TX erzeugen
    │   ├─ Inputs from UTXO
    │   ├─ Outputs (dest + change)
    │   ├─ Calculate fees
    │   └─ Sign with private keys
    │
    └─ rpc_client.submit_transaction(tx)  ◄─── über WebSocket/gRPC
       │
       └─ Kaspad Node
          ├─ Validate TX
          ├─ Add to Mempool
          ├─ Broadcast to P2P network
          └─ Include in next block
```

---

## Next Steps

- [**02-NODE-RPC-STACK.md**](02-NODE-RPC-STACK.md) - Kaspad, RPC-Architektur, Methoden
- [**03-WALLET-FRAMEWORK.md**](03-WALLET-FRAMEWORK.md) - Wallet-Funktionalität, Key Management, UTXO
- [**04-WASM-BROWSER-INTEGRATION.md**](04-WASM-BROWSER-INTEGRATION.md) - WASM SDK, TypeScript API
- [**05-INFRASTRUKTUR-KOMPONENTEN.md**](05-INFRASTRUKTUR-KOMPONENTEN.md) - UTXO Index, Mining, Examples
- [**06-INTEGRATION-PATTERNS.md**](06-INTEGRATION-PATTERNS.md) - Backend, Web Wallet, WaaS Szenarien
- [**07-BEST-PRACTICES-TROUBLESHOOTING.md**](07-BEST-PRACTICES-TROUBLESHOOTING.md) - Best Practices, Troubleshooting, Performance

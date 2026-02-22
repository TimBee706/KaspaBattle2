# Kaspa Node & RPC Stack

**Version:** 1.0  
**Fokus:** Kaspad Full Node, wRPC/gRPC Architektur, RPC-Methoden

---

## Die Kaspad Node-Binary

### Überblick

`kaspad` ist die Hauptkomponente des Netzwerks - ein vollständiger Kaspa-Node, der Folgendes bereitstellt:

- **Full Block DAG Validation**: Validierung aller Blöcke gegen Consensus-Regeln
- **Mempool Management**: TX-Pool-Verwaltung und Prioritäten
- **P2P Networking**: Kommunikation mit anderen Nodes
- **RPC API**: Schnittstellen für externe Clients (wRPC, gRPC)

### Build und Start

**Kompilieren:**
```bash
cd rusty-kaspa
cargo build --release -p kaspad
```

**Ausführen (Testnet):**
```bash
./target/release/kaspad --testnet
```

**Wichtige Flags:**
- `--testnet`: Testnet betreiben (nicht Mainnet)
- `--devnet`: Lokales Dev-Netzwerk
- `--utxoindex`: UTXO-Index aktivieren (für Wallet & schnelle UTXO-Queries)
- `--disable-upnp`: UPnP deaktivieren
- `--listen-address`: RPC Listen-Adresse (default: `0.0.0.0:16110`)

**Konfiguration:**
- Datei: `kaspad/config.yaml` (beispielhaft)
- Kommandozeilen-Argumente überschreiben Datei-Konfiguration

### Interne Architektur (kaspad_lib)

Das Herzstück ist in `kaspad/src/lib.rs` definiert:

```rust
pub mod args;          // CLI-Argumente
pub mod daemon;        // Core-Initialisierung & Event-Loop

// Abhängigkeiten (aus Cargo.toml):
kaspa_consensus        // Consensus-Engine
kaspa_consensusmanager // Manager für Consensus-Zustand
kaspa_p2p_flows        // P2P-Kommunikation
kaspa_rpc_service      // RPC-Service
kaspa_grpc_server      // gRPC Server
kaspa_database         // RocksDB Storage
kaspa_mining           // Mining-Templates
kaspa_notify           // Event-System
```

**Initialisierungsprozess:**

```rust
// aus kaspad/src/main.rs
init_allocator_with_default_settings();  // Memory-Allocator setup
let args = parse_args();                  // CLI args parsing
create_core(args, fd_budget);             // Initialisierung aller Subsysteme
Arc::new(Signals::new(&core)).init();     // Signal-Handler (graceful shutdown)
core.run();                               // Event-Loop
```

### RPC-Service im Kaspad

Der `RpcCoreService` (in `kaspad/src/daemon`) bindet alle RPC-Methoden an:

```rust
// Pseudocode - die echte Implementierung ist komplexer
pub struct RpcCoreService {
    consensus_manager: Arc<ConsensusManager>,
    p2p_manager: Arc<P2pManager>,
    mempool: Arc<Mempool>,
    database: Arc<Database>,
    // ...weitere Komponenten
}

impl RpcApi for RpcCoreService {
    // Implementiert alle RPC-Methoden:
    // - Chain-Queries: get_block(), get_blocks(), etc.
    // - Info: get_info(), get_server_info(), get_sync_status()
    // - TX Broadcasting: submit_transaction()
    // - Mining: get_block_template()
    // - Notifications: subscribe/unsubscribe
}
```

---

## RPC-Architektur: Das Schichtenmodell

Das Kaspa-RPC-System ist in Schichten aufgeteilt:

### Schicht 1: RPC-Definitionen (kaspa_rpc_core)

**Ort:** `rpc/core/src/`

Definiert die abstrakten RPC-API-Verträge:

```rust
// rpc/core/src/api/rpc.rs
#[async_trait]
pub trait RpcApi: Sync + Send {
    // Alle RPC-Methoden - abstrakte Definition
    async fn ping(&self) -> RpcResult<()>;
    async fn get_info(&self) -> RpcResult<GetInfoResponse>;
    async fn submit_transaction(&self, tx: Transaction) -> RpcResult<()>;
    // ...150+ weitere Methoden
}

// rpc/core/src/api/ops.rs
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum RpcApiOps {
    Ping = 0,
    GetInfo = 1,
    SubmitTransaction = 2,
    // ...alle RPC-Operations mit eindeutigen IDs
}
```

**Wichtige Types:**

```rust
// Request/Response Types für jede RPC-Methode
pub struct GetInfoRequest {}
pub struct GetInfoResponse {
    pub server_version: String,
    pub is_utxo_indexed: bool,
    pub is_synced: bool,
    pub p2p_id: String,
    pub rpc_api_version: u32,
    // ...
}

pub struct SubmitTransactionRequest {
    pub transaction: RpcTransaction,
}
pub struct SubmitTransactionResponse {
    pub transaction_id: String,
}
```

### Schicht 2: Transports (gRPC und wRPC)

Beide Transporte implementieren das `RpcApi`-Trait auf verschiedene Weise:

#### wRPC (WebSocket RPC) - Das empfohlene Protokoll

**Ort:** `rpc/wrpc/`

wRPC ist ein leichtgewichtiges RPC-Framework mit Borsh (Binary) oder JSON-Serialisierung:

**Server** (`rpc/wrpc/server/src/`):
```rust
// Startet WebSocket-Server
pub struct Server {
    rpc_service: Arc<RpcCoreService>,
    encoding: Encoding,  // Borsh oder JSON
}

impl Server {
    pub async fn connect(&self, peer: &SocketAddr, messenger: Arc<Messenger>) 
        -> Result<Connection>;
    
    pub async fn start_notify(&self, connection: &Connection, scope: Scope) 
        -> RpcResult<()>;
}

// wRPC Handler definiert RPC-Methode-Routing
pub struct KaspaRpcHandler { /* ... */ }

#[async_trait]
impl RpcHandler for KaspaRpcHandler {
    // Handshake, connect/disconnect handling
}
```

**Client** (`rpc/wrpc/client/src/`):
```rust
#[derive(Clone)]
pub struct KaspaRpcClient {
    inner: Arc<Inner>,
}

impl KaspaRpcClient {
    pub async fn connect_with_url(url: &str) -> Result<Self> {
        // Verbindung zu WebSocket-Server aufbauen
    }
    
    pub async fn call<R: RpcRequest>(&self, request: R) -> Result<R::Response> {
        // RPC-Aufruf über wRPC
    }
}

#[async_trait]
impl RpcApi for KaspaRpcClient {
    // Implementiert alle RPC-Methoden durch wRPC
}
```

**Protokoll-Beispiel (Borsh):**

```
Client -> Server: 
[Op: u32(1)=GetInfo, Encoded Request]
  ↓
Server processes
  ↓
Server -> Client:
[Status: u8, Encoded Response]
```

**Protokoll-Beispiel (JSON):**

```json
Client -> Server:
{"op": "getInfo", "params": {}}

Server -> Client:
{"status": 0, "result": {"serverVersion": "..."}}
```

#### gRPC - Alternative für höhere Durchsatzanforderungen

**Ort:** `rpc/grpc/`

gRPC ist ein HIGH-Performance RPC Framework auf Basis von Protocol Buffers:

```rust
// Server
pub struct GrpcServer {
    // Bindet RPC-Methoden über gRPC
}

// Client
#[derive(Clone)]
pub struct GrpcClient { /* ... */ }

#[async_trait]
impl RpcApi for GrpcClient {
    // Implementiert alle RPC-Methoden über gRPC
}
```

### Schicht 3: Abstraktion - Das RpcApi Trait

```rust
#[async_trait]
pub trait RpcApi {
    // High-Level-Methoden (convenience)
    async fn get_info(&self) -> RpcResult<GetInfoResponse> {
        self.get_info_call(None, GetInfoRequest {}).await
    }
    
    // Low-Level _call Methoden (für direkte Kontrolle)
    async fn get_info_call(
        &self, 
        connection: Option<&DynRpcConnection>,
        request: GetInfoRequest
    ) -> RpcResult<GetInfoResponse>;
}
```

### Schicht 4: Service-Implementierung (RpcCoreService)

**Ort:** `rpc/service/src/`

Hier werden alle RPC-Methoden konkret gegen den Node implementiert:

```rust
pub struct RpcCoreService {
    consensus_manager: Arc<ConsensusManager>,
    p2p_manager: Arc<P2pManager>,
    mempool: Arc<Mempool>,
    mining_manager: Arc<MiningManager>,
    database: Arc<Database>,
    notifier: Arc<Notifier>,
}

#[async_trait]
impl RpcApi for RpcCoreService {
    async fn get_info_call(
        &self,
        _connection: Option<&DynRpcConnection>,
        _request: GetInfoRequest
    ) -> RpcResult<GetInfoResponse> {
        // Echte Implementierung - Daten aus Consensus-Manager
        let server_version = env!("CARGO_PKG_VERSION").to_string();
        let is_synced = self.consensus_manager.is_synced();
        let tip_hashes = self.consensus_manager.get_sink();
        
        Ok(GetInfoResponse {
            server_version,
            is_synced,
            p2p_id: self.p2p_manager.peer_id(),
            // ...
        })
    }
    
    async fn submit_transaction_call(
        &self,
        _: Option<&DynRpcConnection>,
        request: SubmitTransactionRequest
    ) -> RpcResult<SubmitTransactionResponse> {
        // TX zum Mempool hinzufügen
        let tx = request.transaction.to_native()?;
        self.mempool.add_transaction(tx.clone()).await?;
        
        Ok(SubmitTransactionResponse {
            transaction_id: tx.id().to_string(),
        })
    }
}
```

---

## Konkrete Integration: RPC-Client Nutzung

### Rust-Beispiel (Natives Environment)

```rust
use kaspa_wrpc_client::{KaspaRpcClient, Resolver, WrpcEncoding};
use kaspa_rpc_core::api::rpc::RpcApi;
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<()> {
    // Client erstellen
    let client = KaspaRpcClient::connect_with_url(
        "ws://127.0.0.1:16110"
    ).await?;
    
    // Direkte Methoden aufrufen
    let info = client.get_info().await?;
    println!("Server Version: {}", info.server_version);
    println!("Synced: {}", info.is_synced);
    
    // UTXO abfragen
    let response = client.get_utxos_by_addresses(vec![
        "addr_1".to_string()
    ]).await?;
    
    println!("UTXO Count: {}", response.entries.len());
    
    // TX Broadcasting
    let tx_response = client.submit_transaction(
        transaction_object
    ).await?;
    
    println!("TX ID: {}", tx_response.transaction_id);
    
    Ok(())
}
```

### Von der RPC-Core Definition zur Implementierung

Das System nutzt Code-Generierung via Makros für Konsistenz:

```rust
// In RPC-Client
build_wrpc_client_interface!(
    RpcApiOps,
    [
        Ping,
        GetInfo,
        SubmitTransaction,
        GetUtxosByAddresses,
        // ...alle 150+ RPC-Methoden
    ]
);

// Makro generiert für jede Methode:
async fn ping_call(&self, request: PingRequest) -> RpcResult<PingResponse> {
    self.call(RpcApiOps::Ping, request).await
}

async fn get_info_call(&self, request: GetInfoRequest) -> RpcResult<GetInfoResponse> {
    self.call(RpcApiOps::GetInfo, request).await
}
// ...
```

---

## Wichtigste RPC-Methoden für Integration

### Chain-Daten Queries

```rust
// Block-Informationen
async fn get_block(&self, hash: String) -> RpcResult<GetBlockResponse>;
async fn get_blocks(&self, hashes: Vec<String>) -> RpcResult<GetBlocksResponse>;
async fn get_block_dag_info(&self) -> RpcResult<GetBlockDagInfoResponse>;

// UTXO-Queries
async fn get_utxos_by_addresses(&self, addresses: Vec<String>) 
    -> RpcResult<GetUtxosByAddressesResponse>;
    
// Balance
async fn get_balance_by_address(&self, address: String) 
    -> RpcResult<BalanceResponse>;
```

### TX Broadcasting

```rust
async fn submit_transaction(&self, tx: Transaction) 
    -> RpcResult<SubmitTransactionResponse>;
    
async fn submit_transaction_replacement(
    &self, 
    old_tx_id: String, 
    new_tx: Transaction
) -> RpcResult<SubmitTransactionReplacementResponse>;
```

### Mining & Templates

```rust
async fn get_block_template(&self, pay_address: String) 
    -> RpcResult<GetBlockTemplateResponse>;
```

### Subscriptions (Notifications)

```rust
// Listener registrieren
fn register_new_listener(&self, connection: ChannelConnection) -> ListenerId;

// Auf Benachrichtigungen abonnieren
async fn subscribe(&self, scope: Scope) -> RpcResult<()>;
async fn unsubscribe(&self, scope: Scope) -> RpcResult<()>;
```

---

## Performance Tipps für RPC

### Verbindung Management

**✅ Gut:**
```rust
// Verbindung wiederverwenden
let client = KaspaRpcClient::connect_with_url("ws://localhost:16110").await?;
let client = Arc::new(client);  // Share across tasks

// Mehrfach verwenden
for _ in 0..1000 {
    client.get_info().await?;  // Reuses same connection
}
```

**❌ Schlecht:**
```rust
// Neue Verbindung für jede Anfrage
for _ in 0..1000 {
    let client = KaspaRpcClient::connect_with_url("...").await?;  // INEFFIZIENT
    client.get_info().await?;
}
```

### Batch-Operationen

```rust
// ❌ Schlecht: Sequenziell
for address in addresses {
    let balance = client.get_balance_by_address(&address).await?;
    balances.push(balance);
}

// ✅ Gut: Parallel (10x schneller)
let futures: Vec<_> = addresses.iter()
    .map(|addr| client.get_balance_by_address(addr))
    .collect();
let results = futures::future::join_all(futures).await;
```

### Encoding auswählen

```rust
// Borsh (Binary) - schneller für große Datenmengen
let client = KaspaRpcClient::connect_with_options(
    "ws://localhost:16110",
    ConnectOptions::new().with_encoding(WrpcEncoding::Borsh)
).await?;

// JSON - besser für Debugging
let client = KaspaRpcClient::connect_with_options(
    "ws://localhost:16110",
    ConnectOptions::new().with_encoding(WrpcEncoding::Json)
).await?;
```

---

## Next Steps

- [**01-ARCHITEKTUR-UEBERSICHT.md**](01-ARCHITEKTUR-UEBERSICHT.md) - Überblick & Workspace-Struktur
- [**03-WALLET-FRAMEWORK.md**](03-WALLET-FRAMEWORK.md) - Wallet-Funktionalität, Key Management
- [**04-WASM-BROWSER-INTEGRATION.md**](04-WASM-BROWSER-INTEGRATION.md) - WASM SDK, TypeScript API

# Infrastruktur-Komponenten

**Version:** 1.0  
**Fokus:** UTXO-Indexer, Mining-Support, Examples, weitere Services

---

## UTXO-Indexer (indexes/utxoindex)

Der UTXO-Indexer ist essenziell für schnelle UTXO-Abfragen ohne vollständigen Blockchain-Scan:

### Architektur

```rust
// indexes/utxoindex/src/lib.rs
pub struct UtxoIndex {
    store: RocksDB,  // Persistente UTXO-Datenbank
    
    // Index: address -> Vec<Outpoint>
    address_index: HashMap<Address, Vec<Outpoint>>,
    
    // Index: outpoint -> UtxoEntry
    utxo_store: HashMap<Outpoint, UtxoEntry>,
}

impl UtxoIndex {
    pub async fn start(consensus_manager: Arc<ConsensusManager>) -> Result<Self> {
        let store = Self::new()?;
        
        // Auf neue Blöcke abonnieren
        consensus_manager.subscribe_blocks(move |block| {
            store.process_block(block).await
        });
        
        Ok(store)
    }
    
    pub async fn get_utxos_by_address(
        &self, 
        address: &Address
    ) -> Result<Vec<UtxoEntry>> {
        // O(1) Lookup! Sehr schnell
        let outpoints = self.address_index.get(address)?;
        let utxos: Vec<_> = outpoints
            .iter()
            .map(|op| self.utxo_store.get(op))
            .collect();
        Ok(utxos)
    }
}
```

### Verwendung im Kaspad

```bash
# Index aktivieren beim Start
./kaspad --utxoindex

# Der Node wird dann automatisch:
# 1. Alle bestehenden Blöcke indexieren
# 2. Neue Blöcke bei Erhalt indexieren
# 3. RPC-Methoden beschleunigen:
#    - get_utxos_by_addresses()  # < 10ms statt 100ms+
```

### RPC-Integration

```rust
// rpc/service/src/service.rs
impl RpcApi for RpcCoreService {
    async fn get_utxos_by_addresses_call(
        &self,
        _: Option<&DynRpcConnection>,
        request: GetUtxosByAddressesRequest
    ) -> RpcResult<GetUtxosByAddressesResponse> {
        let mut entries = Vec::new();
        
        for address_str in &request.addresses {
            let address = Address::from_str(address_str)?;
            
            // Mit Index: < 10ms
            // Ohne Index: 100ms+ (vollständiger Blockchain-Scan)
            if let Some(index) = &self.utxo_index {
                entries.extend(index.get_utxos_by_address(&address).await?);
            } else {
                // Fallback: Teuer, aber funktioniert
                entries.extend(self.scan_all_utxos(&address).await?);
            }
        }
        
        Ok(GetUtxosByAddressesResponse { entries })
    }
}
```

### Performance Vergleich

```
Query: 100 Adressen UTXO Scan

Mit UTXO-Index (--utxoindex):
- Initiale Indexierung: ~30 Minuten (einmalig)
- Query-Zeit: ~10ms
- Speichernutzung: ~5-10 GB (je nach Chain-Größe)

Ohne Index:
- Initiale Indexierung: nicht nötig
- Query-Zeit: ~500-1000ms (vollständiger Scan)
- Speichernutzung: minimal
```

---

## Mining Support

### Mining Bridge (bridge/)

Der Stratum-Mining-Bridge verbindet Mining-Pools mit Kaspa:

**Konfiguration:** `bridge/config.yaml`

```yaml
# Pool Connection
pool:
  host: pool.kaspa.org
  port: 3333
  protocol: stratum

# Kaspa Node
kaspa_node:
  host: localhost
  port: 16110
  network: testnet

# Mining Config
mining:
  min_share_difficulty: 1024
  timeout: 30
```

**Start:**

```bash
cd rusty-kaspa/bridge
cargo build --release
./target/release/bridge --config config.yaml
```

**Features:**
- Stratum Protocol Support (kompatibel mit bestehenden Mining-Pools)
- Share Tracking & Difficulty Adjustment
- Mining Pool Payouts
- Performance-optimiert für viele Worker

### Block Template & Mining

```rust
// RPC-Methode: Block Template abrufen
pub async fn get_block_template(
    &self, 
    pay_address: &str
) -> Result<GetBlockTemplateResponse> {
    let template = self.mining_manager
        .get_block_template(pay_address)?;
    
    Ok(GetBlockTemplateResponse {
        block: template.block,
        difficulty: template.difficulty,
        headers: template.block.header,
        extra_data: template.extra_data,
    })
}

// Miner verwendet dann:
// 1. Template-Daten als Basis
// 2. Ändert Nonce
// 3. Findet einen Hash, der unter der Difficulty liegt
// 4. Submits Block über submit_block()
```

---

## Node/Wallet Examples (wasm/examples & wasm/nodejs)

Die Examples zeigen praktische Integrationsmuster:

### Examples Struktur

```
wasm/
├── examples/
│   ├── javascript/
│   │   ├── general/
│   │   │   ├── 01-connect.js         # RPC Connection
│   │   │   ├── 02-wallet-create.js   # Wallet Creation
│   │   │   ├── 03-addresses.js       # Address Generation
│   │   │   ├── 04-keys.js            # Key Management
│   │   │   └── 05-encryption.js      # Wallet Encryption
│   │   ├── transactions/
│   │   │   ├── 01-send-simple.js     # Simple TX
│   │   │   ├── 02-send-batch.js      # Batch TX
│   │   │   ├── 03-estimate-fee.js    # Fee Estimation
│   │   │   └── 04-transaction-history.js
│   │   └── wallet/
│   │       ├── 01-wallet-load.js     # Load Wallet
│   │       ├── 02-account-sync.js    # Account Sync
│   │       └── 03-notifications.js   # Event Listeners
│   ├── typescript/
│   │   ├── advanced-wallet.ts
│   │   ├── custom-derivation.ts
│   │   └── rpc-subscriptions.ts
│   └── data/
│       ├── wallet.json               # Test Wallet Data
│       └── config.json               # Test Config
```

### Beispiel: Batch Transactions

```javascript
// wasm/examples/javascript/transactions/02-send-batch.js
import * as kaspa from '../../../../nodejs/kaspa/index.js';

async function main() {
    const wallet = await kaspa.Wallet.load('./wallet.json', 'password');
    const account = await wallet.getAccount(0);
    
    // Mehrere TX in Batch erstellen
    const recipients = [
        { address: 'kaspa1q...1', amount: 100_000 },
        { address: 'kaspa1q...2', amount: 50_000 },
        { address: 'kaspa1q...3', amount: 75_000 },
    ];
    
    for (const recipient of recipients) {
        try {
            const txId = await account.send({
                destination: recipient.address,
                amount: recipient.amount
            });
            console.log(`Sent to ${recipient.address}: ${txId}`);
        } catch (err) {
            console.error(`Failed to send to ${recipient.address}:`, err);
        }
    }
}

main().catch(console.error);
```

### Beispiel: Fee Estimation

```javascript
// wasm/examples/javascript/transactions/03-estimate-fee.js
import * as kaspa from '../../../../nodejs/kaspa/index.js';

async function main() {
    const client = new kaspa.RpcClient('localhost:16110');
    await client.connect();
    
    // Aktuelle Netzwerk-Statistiken
    const info = await client.getInfo();
    const feeEstimate = await client.getFeeEstimate();
    
    console.log(`Network Fee Rate: ${feeEstimate.fee_rate} sats/byte`);
    console.log(`Median TX Size: ${feeEstimate.median_tx_size} bytes`);
    
    // TX Fee Berechnung
    const txSize = 250;  // bytes
    const estimatedFee = txSize * feeEstimate.fee_rate;
    
    console.log(`Estimated Fee for ${txSize}B TX: ${estimatedFee} sats`);
    
    // Verschiedene Szenarien
    const scenarios = [
        { size: 200, priority: 'low' },
        { size: 250, priority: 'normal' },
        { size: 300, priority: 'high' }
    ];
    
    for (const scenario of scenarios) {
        const fee = scenario.size * feeEstimate.fee_rate;
        console.log(`${scenario.priority}: ${scenario.size}B = ${fee} sats`);
    }
}

main().catch(console.error);
```

### Beispiel: Wallet Notifications

```javascript
// wasm/examples/javascript/wallet/03-notifications.js
import * as kaspa from '../../../../nodejs/kaspa/index.js';

async function main() {
    const wallet = await kaspa.Wallet.load('./wallet.json', 'password');
    const account = await wallet.getAccount(0);
    
    // Neue Blöcke
    wallet.addEventListener('blockAdded', (block) => {
        console.log(`Block: ${block.hash} (DAA: ${block.daaScore})`);
    });
    
    // UTXO Änderungen
    wallet.addEventListener('utxoChanged', (change) => {
        console.log(`UTXO Update: +${change.added.length}, -${change.removed.length}`);
    });
    
    // TX Events
    account.addEventListener('transactionSent', (tx) => {
        console.log(`TX Sent: ${tx.id}`);
    });
    
    account.addEventListener('transactionReceived', (tx) => {
        console.log(`TX Received: ${tx.id}`);
    });
    
    account.addEventListener('transactionConfirmed', (tx) => {
        console.log(`TX Confirmed: ${tx.id} (${tx.confirmations} confirmations)`);
    });
    
    // Wallet starten (beginnt mit Subscriptions)
    await wallet.start();
    
    console.log('Listening for events... (Ctrl+C to exit)');
}

main().catch(console.error);
```

---

## Weitere Service-Komponenten

### Consensus Engine

```rust
// consensus/src/lib.rs
pub struct ConsensusManager {
    // Block DAG Management
    dag: Arc<BlockDag>,
    
    // Transaction Validation
    validator: Arc<TransactionValidator>,
    
    // State Management
    state: Arc<ConsensusState>,
}

impl ConsensusManager {
    pub async fn add_block(&self, block: Block) -> Result<()> {
        // 1. Validate block against consensus rules
        self.validator.validate_block(&block)?;
        
        // 2. Add to DAG
        self.dag.add_block(block).await?;
        
        // 3. Update state if needed
        if self.is_sink_affected() {
            self.state.update_from_dag().await?;
        }
        
        Ok(())
    }
}
```

### P2P Network

```rust
// components/connectionmanager/ - Verbindungsverwaltung
// components/addressmanager/ - Peer-Discovery
// protocol/p2p/ - P2P Message Protocol

pub struct P2pManager {
    peers: Arc<Mutex<Vec<Peer>>>,
    message_handler: Arc<MessageHandler>,
}

impl P2pManager {
    pub async fn broadcast_transaction(&self, tx: Transaction) -> Result<()> {
        // TX an alle Peers broadcast
        for peer in self.peers.lock().await.iter() {
            peer.send_transaction(&tx).await.ok();
        }
        Ok(())
    }
}
```

### Mempool Management

```rust
// protocol/mining/ - Mempool
pub struct Mempool {
    transactions: Arc<Mutex<HashMap<TxId, Transaction>>>,
    priorities: Arc<Mutex<BinaryHeap<TxPriority>>>,
}

impl Mempool {
    pub async fn add_transaction(&self, tx: Transaction) -> Result<()> {
        // 1. Validate TX
        if self.is_invalid(&tx).await? {
            return Err("Invalid TX".into());
        }
        
        // 2. Check for double-spend
        if self.has_conflicting_inputs(&tx).await? {
            return Err("Double spend detected".into());
        }
        
        // 3. Add to mempool
        let priority = calculate_priority(&tx);
        self.transactions.lock().await.insert(tx.id(), tx);
        self.priorities.lock().await.push(priority);
        
        Ok(())
    }
}
```

---

## Debugging & Logging

### Logging im Kaspad

```bash
# Mit RUST_LOG einstellen
RUST_LOG=debug ./kaspad --testnet
RUST_LOG=kaspa_consensus=debug,kaspa_rpc=info ./kaspad --testnet

# Log Levels: trace, debug, info, warn, error
```

### Log-Ausgabe verstehen

```
[2026-02-13 15:23:45.123] INFO  kaspa_kaspad - Starting Kaspad v0.10.1
[2026-02-13 15:23:46.456] DEBUG kaspa_consensus - Loading DAG state from disk
[2026-02-13 15:23:47.789] INFO  kaspa_p2p - Connecting to 8 peers
[2026-02-13 15:23:50.123] DEBUG kaspa_rpc - RPC server listening on 0.0.0.0:16110
[2026-02-13 15:23:52.456] INFO  kaspa_consensus - DAG synced (height: 1234567)
```

### Debugging Tipps

```rust
// 1. Print Debugging
eprintln!("Block hash: {:?}", block.hash);
println!("Balance: {}", balance);

// 2. Structured Logging mit tracing
use tracing::{debug, info, error};

debug!("Processing block: {}", block.id);
info!("Synced to height {}", height);
error!("Failed to add transaction: {}", error);

// 3. Conditional Debugging
#[cfg(debug_assertions)]
println!("Debug: {:?}", value);
```

---

## Performance Monitoring

### Metriken auslesen

```rust
// metrics/core/ - Metriken-System
pub struct Metrics {
    blocks_processed: Counter,
    transactions_mempool: Gauge,
    rpc_requests: Counter,
    peer_connections: Gauge,
}

impl Metrics {
    pub fn record_block_processed(&self) {
        self.blocks_processed.increment();
    }
    
    pub fn set_mempool_size(&self, size: usize) {
        self.transactions_mempool.set(size as u64);
    }
}
```

### Monitoring Setup

```bash
# Metriken auf Port 9090 freigeben
./kaspad --testnet --metrics-listen 127.0.0.1:9090

# Dann abrufen:
curl http://127.0.0.1:9090/metrics
```

---

## Next Steps

- [**01-ARCHITEKTUR-UEBERSICHT.md**](01-ARCHITEKTUR-UEBERSICHT.md) - Überblick
- [**02-NODE-RPC-STACK.md**](02-NODE-RPC-STACK.md) - Node & RPC
- [**06-INTEGRATION-PATTERNS.md**](06-INTEGRATION-PATTERNS.md) - Integration Szenarien

# Best Practices & Troubleshooting

**Version:** 1.0  
**Fokus:** Best Practices, Fehlerbehandlung, Performance-Tuning, Troubleshooting

---

## Best Practices für Entwickler

### 1. RPC-Verbindung Management

**✅ Gut - Verbindung wiederverwenden:**
```rust
let client = KaspaRpcClient::connect_with_url("ws://localhost:16110").await?;
let client = Arc::new(client);

// Share across tasks
for _ in 0..1000 {
    client.get_info().await?;  // Reuses same connection
}
```

**❌ Schlecht - Neue Verbindung pro Anfrage:**
```rust
for _ in 0..1000 {
    let client = KaspaRpcClient::connect_with_url("...").await?;  // INEFFIZIENT!
    client.get_info().await?;
}
```

### 2. UTXO Selection (Coin Selection)

**✅ Gut - intelligente Auswahl:**
```rust
let mut utxos = available_utxos.clone();
utxos.sort_by_key(|u| u.amount);  // Kleinste zuerst (minimieren Inputs)

let mut selected = Vec::new();
let mut total = 0u64;
for utxo in utxos {
    selected.push(utxo.clone());
    total += utxo.amount;
    if total >= required_amount {
        break;
    }
}
```

**❌ Schlecht - alle UTXO verwenden:**
```rust
let selected = available_utxos.clone();  // 500+ Inputs!
// Probleme:
// - Sehr hohe TX Fees
// - Längere Bestätigungszeit
// - TX kann zu groß für Blöcke sein
```

### 3. Fee Estimation

**✅ Gut - dynamische Fee Berechnung:**
```rust
let fee_estimate = client.get_fee_estimate()?;
let recommended_fee_rate = fee_estimate.fee_rate;

let tx_mass = calculate_transaction_mass(&inputs, &outputs);
let estimated_fee = (tx_mass as f64 * recommended_fee_rate).ceil() as u64;

generator.set_fee_rate(recommended_fee_rate);
```

**❌ Schlecht - hardcoded Fees:**
```rust
generator.set_fee_rate(0.5);  // Kann zu teuer oder zu billig sein
// Führt zu: TX-Rejections bei hohem Netzwerk-Traffic
```

### 4. Async/Await Patterns

**✅ Gut - parallele Anfragen:**
```rust
let (info, sync_status) = tokio::join!(
    client.get_info(),
    client.get_sync_status()
);  // ~2x schneller
```

**❌ Schlecht - sequenzielle Anfragen:**
```rust
let info = client.get_info().await?;
let sync_status = client.get_sync_status().await?;
// Blockiert unnötig
```

### 5. Error Handling & Recovery

**✅ Gut - spezifische Fehlerbehandlung:**
```rust
match client.submit_transaction(tx).await {
    Ok(response) => println!("TX {}", response.transaction_id),
    Err(e) => {
        if e.to_string().contains("UTXO not found") {
            eprintln!("Invalid input - UTXO was already spent");
            // Retry mit neuer UTXO-Selection
        } else if e.to_string().contains("double spend") {
            eprintln!("TX conflict - TX replacement erforderlich");
            // TX mit höherer Fee erneut senden
        } else if e.to_string().contains("mempool full") {
            eprintln!("Mempool überlastet - warten und erneut versuchen");
            tokio::time::sleep(Duration::from_secs(5)).await;
        } else {
            eprintln!("Unknown error: {}", e);
        }
    }
}
```

**❌ Schlecht - generisches Error Handling:**
```rust
client.submit_transaction(tx).await.unwrap();  // Panik!

// oder
client.submit_transaction(tx).await.ok();  // Fehler ignoriert!
```

### 6. Coinbase UTXO Handling

Coinbase UTXOs (Mining-Rewards) müssen erst reifen:

```rust
const COINBASE_MATURITY: u64 = 110;  // DAA Scores

// Filter bei UTXO-Auswahl
for utxo in &available_utxos {
    if utxo.is_coinbase {
        let age = current_daa_score - utxo.block_daa_score;
        if age < COINBASE_MATURITY {
            continue;  // Skip - nicht spendbar
        }
    }
}
```

### 7. Network Awareness

**Prüfen ob Node synchronisiert:**
```rust
let info = client.get_info().await?;
if !info.is_synced {
    eprintln!("Node not synced - waiting...");
    // Warten oder abbrechen
    return Err("Node not synced".into());
}
```

---

## Troubleshooting Guide

### Problem 1: "Connection refused" beim RPC

**Ursachen & Lösungen:**

```bash
# 1. Kaspad läuft nicht
ps aux | grep kaspad

# Lösung: Kaspad starten
./kaspad --testnet

# 2. Falscher Port (default: 16110)
# Überprüfen Sie kaspad.conf oder CLI-Flags
./kaspad --testnet --listen-address 0.0.0.0:16110

# 3. Firewall blockiert
sudo ufw allow 16110

# 4. RPC deaktiviert
# Überprüfen Sie kaspad.conf:
# [rpclisten]
# 0.0.0.0:16110

# 5. Binding-Adresse Probleme
# Statt localhost, verwenden Sie 127.0.0.1 oder IP-Adresse
let client = KaspaRpcClient::connect_with_url(
    "ws://127.0.0.1:16110"  // Statt "localhost"
).await?;
```

### Problem 2: "UTXO not found" beim Send

**Ursachen & Lösungen:**

```rust
// 1. UTXO wurde bereits ausgegeben
// Wallet neu synchronisieren
wallet.start_utxo_tracking(None).await?;

// 2. UTXO ist nicht reif (Coinbase)
// Kaspa: Coinbase UTXO brauchen ~110 Blocks
for utxo in &utxos {
    if utxo.is_coinbase {
        let age = current_daa_score - utxo.block_daa_score;
        if age < 110 {
            println!("Coinbase UTXO still maturing: {} blocks remaining", 110 - age);
            continue;  // Skip
        }
    }
}

// 3. UTXO-Index deaktiviert
// Kaspad muss mit --utxoindex laufen
// oder: Vollständiger Blockchain-Scan erforderlich

// 4. Falsche Adresse / falscher Account
let correct_account = wallet.get_account(account_index).await?;
let utxos = correct_account.get_utxos().await?;
```

### Problem 3: TX wird nicht bestätigt

**Symptom:** TX sent, aber nicht in Block enthalten

**Ursachen & Lösungen:**

```rust
// 1. Fee zu niedrig
let current_fee_rate = client.get_fee_estimate().await?;
if fee_rate < current_fee_rate * 0.8 {
    eprintln!("Fee too low! Current rate: {}", current_fee_rate);
    eprintln!("Your rate: {}", fee_rate);
    // Solution: TX mit höherer Fee erneut senden
    let new_tx = create_tx_with_fee_rate(destination, amount, current_fee_rate * 1.2);
    client.submit_transaction(new_tx).await?;
}

// 2. TX ist zu groß (max_transaction_mass)
let mass = tx.calculate_mass();
if mass > 100_000 {
    eprintln!("TX too large ({} bytes), needs to be split", mass);
    // Solution: Mehrere kleinere TX erstellen
}

// 3. Double-spend versucht
// Eine UTXO wird von zwei TX gleichzeitig ausgegeben
// Solution: TX mit höherer Fee ersetzen
client.submit_transaction_replacement(
    old_tx_id,
    new_tx_with_higher_fee
).await?;

// 4. Netzwerk nicht synchronisiert
let info = client.get_info().await?;
if !info.is_synced {
    eprintln!("Node not synced yet, wait...");
}

// 5. Mempool temporarily full
// Solution: Warten, dann erneut versuchen
tokio::time::sleep(Duration::from_secs(10)).await;
client.submit_transaction(tx).await?;
```

### Problem 4: Langsame UTXO Queries

**Symptom:** get_utxos_by_addresses() dauert 500ms+

**Analyse:**

```rust
// BEVOR: Ohne UTXO-Index (~500ms für 100 Adressen)
let start = Instant::now();
let utxos = client.get_utxos_by_addresses(addresses).await?;
println!("Time: {}ms", start.elapsed().as_millis());  // ~500-1000ms

// NACHHER: Mit UTXO-Index (~10ms)
// Kaspad mit --utxoindex starten:
// ./kaspad --testnet --utxoindex

let start = Instant::now();
let utxos = client.get_utxos_by_addresses(addresses).await?;
println!("Time: {}ms", start.elapsed().as_millis());  // ~10ms
```

**Lösung:**

```bash
# Kaspad mit UTXO-Index starten
./kaspad --testnet --utxoindex

# Beim Start wird der Index erstellt (~30 Minuten)
# Danach: sehr schnelle Queries
```

### Problem 5: WASM Memory Issues

**Symptom:** "WebAssembly memory exhausted"

**Ursache:** Zu viele UTXOs im Memory (1000000+ UTXOs)

**Lösungen:**

```javascript
// Lösung 1: Pagination verwenden
const pageSize = 100;
for (let i = 0; i < totalAddresses; i += pageSize) {
    const batch = addresses.slice(i, i + pageSize);
    const utxos = await client.getUtxosByAddresses(batch);
    // Process and discard
}

// Lösung 2: Nur notwendige Adressen tracken
const trackedAddresses = account.getUsedAddresses();
// Nicht alle möglichen Adressen

// Lösung 3: Regelmäßig cleanup
if (utxos.length > 100_000) {
    location.reload();  // Restart WASM
}

// Lösung 4: Memory Limits setzen
const limits = {
    max_utxos_in_memory: 50_000,
    auto_cleanup_interval: 300_000  // 5 minutes
};
```

### Problem 6: Private Key Security Issues

**Warnung:** Niemals Private Keys in Logs oder über HTTP senden!

**✅ Sicher:**
```rust
// 1. Keys nur in Memory halten
let private_key = PrivateKey::from_mnemonic(phrase)?;
// Schnell nach Gebrauch wegwerfen

// 2. Mit Passwort verschlüsselt speichern
wallet.save_encrypted("name", password).await?;

// 3. In nativen Anwendungen: HSM/Secure Enclave
// Hardware Security Module nutzen

// 4. In Web: localStorage mit AES-256
// oder: IndexedDB mit Web Crypto API
```

**❌ Unsicher:**
```rust
// 1. Private Key loggen
eprintln!("Key: {}", private_key);

// 2. Über HTTP senden (unverschlüsselt)
reqwest::Client::new()
    .post(url)
    .json(&json!({ "private_key": key }))
    .send()
    .await?;

// 3. Plain-Text speichern
std::fs::write("wallet.json", wallet_data)?;
```

---

## Performance-Tuning

### 1. Batch-Operationen

```rust
// ❌ Schlecht: Sequenziell (10 Sekunden)
for address in addresses {
    let balance = client.get_balance_by_address(&address).await?;
    balances.push(balance);
}

// ✅ Gut: Parallel (1 Sekunde) - 10x schneller!
let futures: Vec<_> = addresses.iter()
    .map(|addr| client.get_balance_by_address(addr))
    .collect();
let results = futures::future::join_all(futures).await;
```

### 2. Encoding-Auswahl

```rust
// Borsh (Binary) - schneller für große Datenmengen
let client = KaspaRpcClient::connect_with_options(
    "ws://localhost:16110",
    ConnectOptions::new().with_encoding(WrpcEncoding::Borsh)
).await?;

// Vergleich:
// - Borsh: 40% kleiner, 200ms
// - JSON: 100% größer, 300ms

// ✓ Borsh für Production
// ✓ JSON für Debugging
```

### 3. Connection Pooling

```rust
// Pool für viele Clients
let client = KaspaRpcClient::connect_with_options(
    "ws://localhost:16110",
    ConnectOptions::new()
        .with_connect_strategy(ConnectStrategy::FastestReachable)
        .with_timeout(Duration::from_secs(10))
        .with_keep_alive(Duration::from_secs(30))
).await?;
```

### 4. Caching

```rust
use std::sync::Arc;
use parking_lot::RwLock;

pub struct CachedRpcClient {
    cache: Arc<RwLock<HashMap<String, CacheEntry>>>,
    client: Arc<KaspaRpcClient>,
}

impl CachedRpcClient {
    pub async fn get_info(&self) -> Result<GetInfoResponse> {
        // Prüfe Cache
        if let Some(entry) = self.cache.read().get("info") {
            if entry.age_secs() < 60 {  // 1 Minute TTL
                return Ok(entry.data.clone());
            }
        }
        
        // Cache miss - abfragen
        let info = self.client.get_info().await?;
        
        // Cachen
        self.cache.write().insert("info", CacheEntry::new(info.clone()));
        Ok(info)
    }
}
```

### 5. Database Query Optimization

Bei Verwendung von RocksDB:

```rust
// ❌ Schlecht: Viele einzelne Queries
for address in &addresses {
    let utxos = db.get_utxos(address)?;  // N Queries!
    for utxo in utxos {
        // Process
    }
}

// ✅ Gut: Batch Query
let all_utxos = db.get_utxos_batch(&addresses)?;  // 1 Query!
for utxo in all_utxos {
    // Process
}
```

---

## Deployment Checklist

### Pre-Production Checklist

```
Security
─────────
☐ Alle Private Keys sind verschlüsselt
☐ HTTPS/TLS für alle RPC-Verbindungen
☐ Input Validation auf allen Inputs
☐ Rate Limiting auf API Endpoints
☐ No Secrets in Logs

Performance
───────────
☐ UTXO Index aktiviert (--utxoindex)
☐ Borsh Encoding für RPC
☐ Connection Pooling konfiguriert
☐ Caching für häufige Queries
☐ Database Indexes optimiert

Reliability
───────────
☐ Error Handling & Retry-Logik
☐ Monitoring & Alerting Setup
☐ Backup Strategy
☐ Disaster Recovery Plan
☐ Health Checks implementiert

Testing
───────
☐ Unit Tests für Core Logic
☐ Integration Tests mit Testnet
☐ Load Tests (Mehrere Concurrent Users)
☐ Security Audit durchgeführt
☐ Fuzz Testing für Input Handling
```

### Monitoring Setup

```bash
# Node Metriken exportieren
./kaspad --testnet --metrics-listen 127.0.0.1:9090

# Dann abfragen:
curl http://127.0.0.1:9090/metrics

# Wichtige Metriken:
# - blocks_processed: Anzahl verarbeiteter Blöcke
# - transactions_mempool: Größe des Mempool
# - rpc_requests: Anzahl RPC Requests
# - peer_connections: Aktive P2P Connections
# - sync_status: Block DAG Synchronization
```

---

## Häufige Fehler vermeiden

### 1. Address Format Fehler

```rust
// ❌ Schlecht
let address = "1A1z7agoat";  // Bitcoin Address!

// ✅ Gut
let address = "kaspa1qz2ptjk67k2twpvhcqx2fpe3n24xklngrpsatdq4c4l5czll";
// Kaspa nutzt kas:// oder kaspa1q... Format
```

### 2. Transaction Signing Fehler

```rust
// ❌ Schlecht - TX nicht signiert
let tx = generator.generate()?;  // Unsigniert!
client.submit_transaction(tx).await?;

// ✅ Gut - TX signiert
let tx = generator.generate()?.sign(&private_keys)?;
client.submit_transaction(tx).await?;
```

### 3. Type Conversion Fehler

```rust
// ❌ Schlecht - Overflow möglich
let sats: u32 = amount_in_kas * 100_000;  // kann overflow!

// ✅ Gut - sichere Konvertierung
let sats: u64 = (amount_in_kas as f64 * 100_000.0).ceil() as u64;
```

### 4. Race Conditions

```rust
// ❌ Schlecht - Race Condition
let balance_1 = account.get_balance().await?;
account.send(addr, 50_000).await?;
let balance_2 = account.get_balance().await?;
// balance_2 könnte noch alt sein!

// ✅ Gut - Lock verwenden
let mut account = account_mutex.lock().await;
let balance_1 = account.get_balance().await?;
account.send(addr, 50_000).await?;
let balance_2 = account.get_balance().await?;
// Jetzt ist es sequenziell
```

---

## Offizielle Ressourcen

### Dokumentation & Links

- **Rust API Docs**: https://docs.rs/kaspa-wasm/
- **TypeScript Docs**: https://kaspa.aspectron.org/docs/
- **GitHub Repository**: https://github.com/kaspanet/rusty-kaspa
- **WASM Releases**: https://github.com/kaspanet/rusty-kaspa/releases
- **Discord Community**: https://discord.gg/kaspa
- **GitHub Issues**: https://github.com/kaspanet/rusty-kaspa/issues

### Testing Networks

```
Testnet10:
- Faucet: https://faucet.testnet.kaspanet.io
- Explorer: https://testnet10.kaspanet.io
- RPC Default: ws://localhost:16210

Devnet (lokal):
- Erstellen: ./kaspad --devnet
- RPC: ws://localhost:16210
```

---

## Kontakt & Support

- **GitHub Issues**: Bug Reports & Feature Requests
- **Discord**: https://discord.gg/kaspa - Real-time Support
- **Forum**: Community Diskussionen
- **Email**: development@kaspanet.io

---

**Version History:**
- v1.0 (Feb 2026): Initiale umfassende Dokumentation

**Letztes Update:** 13. Februar 2026

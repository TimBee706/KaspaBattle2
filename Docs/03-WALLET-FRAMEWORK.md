# Wallet Framework & Funktionalität

**Version:** 1.0  
**Fokus:** Key Management, UTXO Processing, Transaction Generator, Storage

---

## Wallet Core Framework (kaspa_wallet_core)

### Architektur-Übersicht

Das Wallet-Framework (`wallet/core/`) bietet:

```
┌─────────────────────────────────────────────────┐
│         High-Level Wallet API                    │
│  ┌─────────────────────────────────────────┐   │
│  │  WalletApi Trait                         │   │
│  │  ├─ Account (create, list, import)       │   │
│  │  ├─ Transactions (send, estimate)        │   │
│  │  ├─ UTXO (sync, track)                   │   │
│  │  └─ Keys (derive, import)                │   │
│  └─────────────────────────────────────────┘   │
│                     ▲                            │
│  ┌──────────────────┴──────────────────┐       │
│  │  Mid-Level Primitives               │       │
│  │  ├─ UtxoProcessor (UTXO-Scan)       │       │
│  │  ├─ UtxoContext (UTXO-State)        │       │
│  │  ├─ Generator (TX-Erstellung)       │       │
│  │  └─ Derivation (BIP32)              │       │
│  └──────────────────┬──────────────────┘       │
│  ┌──────────────────▼──────────────────┐       │
│  │  Low-Level Crypto Primitives        │       │
│  │  ├─ keys (kaspa_wallet_keys)         │       │
│  │  ├─ addresses (kaspa_addresses)      │       │
│  │  ├─ hashes (kaspa_hashes)            │       │
│  │  └─ scripts (kaspa_txscript)         │       │
│  └─────────────────────────────────────┘       │
└─────────────────────────────────────────────────┘
```

### Modul-Struktur

```rust
// wallet/core/src/lib.rs
pub mod account;        // Account Management
pub mod api;            // WalletApi Trait & Implementation
pub mod cryptobox;      // Verschlüsselung
pub mod derivation;     // HD-Key Derivation
pub mod encryption;     // Wallet Storage Encryption
pub mod error;          // Error Types
pub mod events;         // Event System
pub mod factory;        // Factory Patterns
pub mod rpc;            // RPC Integration
pub mod storage;        // Storage Backend
pub mod tx;             // TX Generator & Signing
pub mod utxo;           // UTXO Processor & Manager
pub mod wallet;         // Wallet Container
pub mod settings;       // Wallet Settings
```

---

## Key Management (kaspa_wallet_keys & kaspa_wallet_bip32)

### BIP32 HD Wallet Derivation

```rust
use kaspa_wallet_keys::{
    Mnemonic, PrivateKey, PublicKey, 
    derivation::{Derivation, DerivationPath}
};

// Mnemonic erstellen oder importieren
let mnemonic = Mnemonic::random(MnemonicType::Words12)?;
let phrase = mnemonic.to_string();  // "word1 word2 ..."

// Master Key dari Mnemonic
let master_key = PrivateKey::from_mnemonic(&phrase)?;

// HD Derivation (BIP44 Pattern für Kaspa)
// m / 44' / 111' / account' / change / index
// 111 ist Kaspa's BIP44 coin type

let derivation_path = DerivationPath::new(44, 111, 0, 0, 0)?;
let derived_key = master_key.derive_private(&derivation_path)?;
let public_key = derived_key.public_key();

// Adresse generieren
let address = public_key.to_address(NetworkType::Mainnet)?;
println!("Address: {}", address);  // kaspa1qxxxxxxxxx
```

### Account-basierte Ableitung

```rust
pub struct Account {
    name: String,
    index: u32,  // Account-Index für BIP44
    derivation_path: DerivationPath,
    keys: Vec<(u32, PrivateKey)>,  // Index -> Key Mapping
}

impl Account {
    // Adresse an Index generieren
    pub fn address_at_index(&self, index: u32) -> Result<Address> {
        let path = self.derivation_path.with_index(index);
        let key = self.master_key.derive_private(&path)?;
        Ok(key.public_key().to_address()?)
    }
}
```

### Wichtige Key-Typen

```rust
pub struct PrivateKey {
    scalar: Scalar,  // Kryptographischer Scalar
    network: NetworkId,
}

impl PrivateKey {
    pub fn sign(&self, message: &[u8]) -> Signature {
        // schnorr signature (Kaspa nutzt Schnorr, nicht ECDSA)
    }
    
    pub fn public_key(&self) -> PublicKey {
        // Aus Private-Key ableiten
    }
}

pub struct PublicKey {
    point: Point,  // Kryptographischer Punkt
}

impl PublicKey {
    pub fn to_address(&self, network: NetworkId) -> Result<Address> {
        // Blake2b(public_key) -> Kaspa-Adresse
    }
}
```

---

## UTXO Management (utxo:: Modul)

Das UTXO-Management ist das Herzstück des Wallet-Frameworks:

### UtxoProcessor - Scannt die Blockchain nach UTXO

```rust
use kaspa_wallet_core::utxo::{UtxoProcessor, UtxoContext};

// Processor erstellen - Verbindung zu Node erforderlich
let rpc = KaspaRpcClient::connect_with_url("ws://localhost:16110").await?;
let processor = UtxoProcessor::new(rpc.clone()).await?;

// Accounts hinzufügen zum Tracking
for index in 0..20 {
    let account = Account::new(index);
    processor.add_account(account)?;
}

// UTXO-Scan starten
let context = processor.scan_addresses(
    &addresses,
    16110  // Scan-Tiefe (Blocks)
).await?;

// Ergebnis: UtxoContext mit allen bekannten UTXO
let utxos = context.get_utxos()?;
println!("Found {} UTXOs", utxos.len());
```

### UtxoContext - Verwaltet UTXO-Set in Memory

```rust
pub struct UtxoContext {
    utxos: Arc<Mutex<HashMap<Outpoint, UtxoEntry>>>,
    // Outpoint = (tx_id, output_index)
}

impl UtxoContext {
    pub async fn get_utxos(&self) -> Result<Vec<UtxoEntry>> {
        let guard = self.utxos.lock().await;
        Ok(guard.values().cloned().collect())
    }
    
    pub async fn get_balance(&self) -> Result<u64> {
        let utxos = self.get_utxos().await?;
        Ok(utxos.iter().map(|u| u.amount).sum())
    }
    
    pub async fn spend_utxo(&self, outpoint: &Outpoint) -> Result<()> {
        let mut guard = self.utxos.lock().await;
        guard.remove(outpoint);
        Ok(())
    }
}

pub struct UtxoEntry {
    pub outpoint: Outpoint,      // (tx_id, output_index)
    pub amount: u64,              // in Sats
    pub script_public_key: Vec<u8>,
    pub is_coinbase: bool,        // Coinbase UTXO?
    pub block_daa_score: u64,     // DAA Score bei UTXO-Erstellung
}
```

### UTXO-Synchronisation mit Updates

```rust
// Bei neuen Blöcken (über Subscriptions)
async fn on_block_added(
    context: &UtxoContext,
    block: &Block,
    addresses_to_track: &[Address]
) -> Result<()> {
    for (tx_idx, tx) in block.transactions.iter().enumerate() {
        // Inputs verarbeiten (UTXO-Ausgaben)
        for input in &tx.inputs {
            if let Some(utxo) = context.get_utxo(&input.previous_outpoint).await? {
                context.spend_utxo(&input.previous_outpoint).await?;
            }
        }
        
        // Outputs verarbeiten (neue UTXO)
        for (out_idx, output) in tx.outputs.iter().enumerate() {
            if addresses_to_track.contains(&output.address) {
                let outpoint = Outpoint::new(tx.id(), out_idx as u32);
                context.add_utxo(UtxoEntry {
                    outpoint,
                    amount: output.value,
                    script_public_key: output.script.clone(),
                    is_coinbase: tx_idx == 0 && block.is_coinbase_transaction,
                    block_daa_score: block.daa_score,
                }).await?;
            }
        }
    }
    Ok(())
}
```

---

## Transaction Generator (tx:: Modul)

Das TX-Generierungs-System ist flexibel und unterstützt einfache und komplexe TX:

### Einfache TX-Erstellung

```rust
use kaspa_wallet_core::tx::generator::Generator;

// Generator erstellen
let mut generator = Generator::new(
    network_id: NetworkId::Mainnet,
    fee_rate: 1.0,  // Sats/Byte
)?;

// Quellen-UTXO hinzufügen
for utxo in &available_utxos {
    generator.add_input(utxo)?;
}

// Ziele definieren
generator.add_output(
    address: "kaspa1q...",
    amount: 100_000,  // 1 KAS (100000 sats)
    script: None,     // Standard P2PKH
)?;

// Change-Adresse
generator.set_change_address("kaspa1q...")?;

// TX generieren & signieren
let tx = generator.generate()?.sign(&private_keys)?;

// Broadcasten
rpc_client.submit_transaction(tx).await?;
```

### Komplexe TX mit Multi-Input

```rust
// TX mit vielen Inputs (wenn UTXO-Set fragmentiert)
// Das System erstellt automatisch mehrere TX wenn nötig
let mut generator = Generator::new_with_config(
    GeneratorConfig {
        max_transaction_mass: 100_000,  // Maximum Größe
        fee_rate: 1.0,
        prefer_fewer_inputs: true,
    }
)?;

// Alle verfügbaren UTXO hinzufügen
generator.add_inputs(&available_utxos)?;

// Große Ausgabe
generator.add_output("kaspa1q...", 1_000_000_000)?;

// Generator erstellt automatisch mehrere TX falls nötig
let transactions = generator.generate_all()?;

for tx in transactions {
    let signed_tx = tx.sign(&private_keys)?;
    rpc_client.submit_transaction(signed_tx).await?;
}
```

### TX-Mass Kalkulation (wichtig für Fees)

```rust
pub struct TransactionMass {
    // Kaspa nutzt "Mass" statt "Vsize" für Größe-Metriken
    pub input_mass: u64,      // Pro Input ~30 Bytes
    pub output_mass: u64,     // Pro Output ~26 Bytes
    pub script_mass: u64,     // Script-Größe
}

impl TransactionMass {
    pub fn total(&self) -> u64 {
        self.input_mass + self.output_mass + self.script_mass
    }
    
    pub fn calculate_fee(&self, fee_rate: f64) -> u64 {
        // fee = mass * rate
        (self.total() as f64 * fee_rate).ceil() as u64
    }
}
```

---

## Wallet-Storage (storage:: Modul)

Das Wallet-Framework unterstützt mehrere Storage-Backends:

```rust
pub trait WalletStorage: Send + Sync {
    // Persistierung von Wallet-Daten
    
    async fn create(&self, name: &str, config: WalletConfig) -> Result<()>;
    async fn load(&self, name: &str) -> Result<WalletData>;
    async fn save(&self, name: &str, data: &WalletData) -> Result<()>;
    async fn delete(&self, name: &str) -> Result<()>;
}

// Implementierungen:
pub struct FileSystemStorage { /* ... */ }     // Native Filesystem
pub struct LocalStorageBackend { /* ... */ }   // Browser LocalStorage (WASM)
pub struct IndexedDbBackend { /* ... */ }      // IndexedDB (WASM)
```

### Verschlüsselte Speicherung

```rust
pub struct EncryptedStorage {
    backend: Arc<dyn WalletStorage>,
    cipher: Arc<Aes256Gcm>,
}

impl EncryptedStorage {
    pub async fn save_encrypted(
        &self, 
        name: &str, 
        data: &WalletData,
        password: &str
    ) -> Result<()> {
        // 1. Serialize WalletData
        let json = serde_json::to_string(data)?;
        
        // 2. Derive Key from Password (Argon2)
        let salt = rand::random::<[u8; 16]>();
        let key = argon2::hash(password, &salt)?;
        
        // 3. Encrypt with AES256-GCM
        let nonce = rand::random::<[u8; 12]>();
        let ciphertext = cipher.encrypt(&nonce, json.as_bytes())?;
        
        // 4. Save (salt || nonce || ciphertext)
        self.backend.save(name, &[salt, nonce, ciphertext]).await?;
        Ok(())
    }
}
```

---

## Das WalletApi Trait - High-Level Interface

```rust
use kaspa_wallet_core::api::WalletApi;

#[async_trait]
pub trait WalletApi: Send + Sync {
    // Account Management
    async fn create_account(
        &self, 
        account_name: &str, 
        derivation_hint: Option<String>
    ) -> Result<AccountDescriptor>;
    
    async fn get_accounts(&self) -> Result<Vec<AccountDescriptor>>;
    
    // Transactions
    async fn send(
        &self,
        account_index: u32,
        destination: &Address,
        amount: u64
    ) -> Result<String>;  // TX ID
    
    // UTXO Tracking
    async fn start_utxo_tracking(
        &self,
        account_indices: Option<Vec<u32>>
    ) -> Result<()>;
    
    async fn get_balance(
        &self,
        account_index: u32
    ) -> Result<u64>;
    
    // Notifications
    async fn on_utxo_changed(&self) -> Result<()>;
    async fn on_transaction_received(&self) -> Result<()>;
}

// Implementierung
pub struct Wallet {
    accounts: Arc<Mutex<Vec<Account>>>,
    utxo_context: Arc<UtxoContext>,
    rpc_client: Arc<KaspaRpcClient>,
    storage: Arc<dyn WalletStorage>,
}

#[async_trait]
impl WalletApi for Wallet {
    async fn send(
        &self,
        account_index: u32,
        destination: &Address,
        amount: u64
    ) -> Result<String> {
        let accounts = self.accounts.lock().await;
        let account = accounts.get(account_index)
            .ok_or("Account not found")?;
        
        // 1. Auswählen von UTXO
        let utxos = self.utxo_context.get_utxos().await?;
        let selected = select_utxos(&utxos, amount)?;
        
        // 2. TX generieren
        let mut generator = Generator::new(account.network_id, 1.0)?;
        for utxo in &selected {
            generator.add_input(utxo)?;
        }
        generator.add_output(destination, amount, None)?;
        generator.set_change_address(&account.change_address())?;
        
        // 3. Signieren
        let tx = generator.generate()?
            .sign(&account.private_keys())?;
        
        // 4. Broadcasten
        let response = self.rpc_client.submit_transaction(tx).await?;
        
        // 5. Speichern
        self.storage.save_transaction(&response.transaction_id).await?;
        
        Ok(response.transaction_id)
    }
}
```

---

## Account und Portfolio Management

### Account-Struktur

```rust
pub struct Account {
    pub index: u32,
    pub name: String,
    pub network_id: NetworkId,
    pub master_key: PrivateKey,
    pub derivation_path: DerivationPath,
    pub metadata: AccountMetadata,
}

pub struct AccountMetadata {
    pub created_at: u64,
    pub last_sync: u64,
    pub description: String,
    pub balance: u64,
    pub transaction_count: u64,
}

impl Account {
    // Address generieren für externe Adressen
    pub fn external_address(&self, index: u32) -> Result<Address> {
        let path = self.derivation_path
            .with_change(0)
            .with_index(index);
        let key = self.master_key.derive_private(&path)?;
        Ok(key.public_key().to_address(self.network_id)?)
    }
    
    // Change-Adresse (für TX-Change)
    pub fn change_address(&self, index: u32) -> Result<Address> {
        let path = self.derivation_path
            .with_change(1)
            .with_index(index);
        let key = self.master_key.derive_private(&path)?;
        Ok(key.public_key().to_address(self.network_id)?)
    }
}
```

### Portfolio Management

```rust
pub struct Portfolio {
    accounts: HashMap<u32, Account>,
    balances: HashMap<u32, u64>,
    total_balance: u64,
}

impl Portfolio {
    pub fn total_balance(&self) -> u64 {
        self.balances.values().sum()
    }
    
    pub fn accounts_by_balance(&self) -> Vec<(u32, u64)> {
        let mut sorted: Vec<_> = self.balances.iter()
            .map(|(idx, balance)| (*idx, *balance))
            .collect();
        sorted.sort_by(|(_, a), (_, b)| b.cmp(a));
        sorted
    }
}
```

---

## Best Practices für Wallet-Integration

### UTXO Selection (Coin Selection)

**✅ Gut - intelligente Auswahl:**
```rust
let mut utxos = available_utxos.clone();
utxos.sort_by_key(|u| u.amount);  // Kleinste zuerst

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
// Führt zu: höhere Fees, längere Bestätigungszeit
```

### Fee Estimation

**✅ Gut - dynamisch:**
```rust
let fee_estimate = client.get_fee_estimate()?;
let recommended_fee_rate = fee_estimate.fee_rate;
generator.set_fee_rate(recommended_fee_rate);
```

**❌ Schlecht - hardcoded:**
```rust
generator.set_fee_rate(0.5);  // Kann zu teuer oder billig sein
```

### Coinbase UTXO Handling

```rust
// Coinbase UTXOs brauchen ~110 Blocks um spendbar zu sein
for utxo in utxos {
    if utxo.is_coinbase {
        let age = current_daa_score - utxo.block_daa_score;
        if age < 110 {
            continue;  // Skip - nicht spendbar
        }
    }
}
```

---

## Next Steps

- [**01-ARCHITEKTUR-UEBERSICHT.md**](01-ARCHITEKTUR-UEBERSICHT.md) - Überblick
- [**02-NODE-RPC-STACK.md**](02-NODE-RPC-STACK.md) - RPC-Architektur
- [**04-WASM-BROWSER-INTEGRATION.md**](04-WASM-BROWSER-INTEGRATION.md) - WASM SDK

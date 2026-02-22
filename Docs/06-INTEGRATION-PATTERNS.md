# Integration Patterns & Szenarien

**Version:** 1.0  
**Fokus:** Praktische Integration-Szenarien mit vollständigen Code-Beispielen

---

## Szenario 1: Full-Node Backend mit Custom RPC

Ihr Backend komplett in Rust mit Custom RPC-Server für Ihre Anwendung.

### Architektur

```
┌──────────────────┐
│  Ihre App        │
│  (JavaScript)    │
└────────┬─────────┘
         │ HTTP/REST
         │
┌────────▼──────────────────┐
│  Ihr Custom Backend        │
│  (Rust/Actix-Web)         │
├────────────────────────────┤
│ ├─ /api/balance/{addr}    │
│ ├─ /api/send              │
│ ├─ /api/utxos/{addr}      │
│ └─ /api/tx/{id}           │
└────────┬──────────────────┘
         │ WebSocket (wRPC)
         │
┌────────▼──────────────────┐
│  Kaspad Full Node         │
│  (--testnet)              │
└───────────────────────────┘
```

### Implementation

```rust
use kaspa_wrpc_client::KaspaRpcClient;
use kaspa_rpc_core::api::rpc::RpcApi;
use std::sync::Arc;
use actix_web::{web, App, HttpServer, HttpResponse};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize)]
struct BalanceResponse {
    address: String,
    balance: u64,
    balance_kas: f64,
}

#[derive(Debug, Serialize)]
struct SendResponse {
    transaction_id: String,
    status: String,
}

#[derive(Debug, Deserialize)]
struct SendRequest {
    destination: String,
    amount: u64,  // in sats
    fee_rate: Option<f64>,
}

pub struct KaspaBackend {
    node_url: String,
    rpc_client: Arc<KaspaRpcClient>,
}

impl KaspaBackend {
    pub async fn new(node_url: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let client = KaspaRpcClient::connect_with_url(node_url).await?;
        Ok(KaspaBackend {
            node_url: node_url.to_string(),
            rpc_client: Arc::new(client),
        })
    }
    
    pub async fn get_balance(&self, address: &str) -> Result<u64, Box<dyn std::error::Error>> {
        let response = self
            .rpc_client
            .get_balance_by_address(address.to_string())
            .await?;
        Ok(response.balance)
    }
    
    pub async fn get_utxos(&self, addresses: Vec<&str>) -> Result<Vec<(String, u64)>, Box<dyn std::error::Error>> {
        let response = self
            .rpc_client
            .get_utxos_by_addresses(
                addresses.iter().map(|s| s.to_string()).collect()
            )
            .await?;
        
        let utxos = response.entries
            .iter()
            .map(|u| (format!("{}", u.outpoint), u.amount))
            .collect();
        
        Ok(utxos)
    }
}

// Actix-Web Handler
async fn get_balance(
    backend: web::Data<Arc<KaspaBackend>>,
    path: web::Path<String>
) -> HttpResponse {
    let address = path.into_inner();
    
    match backend.get_balance(&address).await {
        Ok(balance) => {
            let response = BalanceResponse {
                address,
                balance,
                balance_kas: balance as f64 / 100_000.0,
            };
            HttpResponse::Ok().json(response)
        },
        Err(e) => {
            HttpResponse::InternalServerError()
                .json(serde_json::json!({ "error": e.to_string() }))
        }
    }
}

async fn send_transaction(
    backend: web::Data<Arc<KaspaBackend>>,
    req: web::Json<SendRequest>
) -> HttpResponse {
    // TODO: Implementiert TX-Signing hier
    // Für dieses Beispiel nehmen wir an, TX ist pre-signed
    
    HttpResponse::NotImplemented()
        .json(serde_json::json!({ "error": "TX signing not implemented in example" }))
}

async fn get_utxos(
    backend: web::Data<Arc<KaspaBackend>>,
    path: web::Path<String>
) -> HttpResponse {
    let address = path.into_inner();
    
    match backend.get_utxos(vec![&address]).await {
        Ok(utxos) => {
            HttpResponse::Ok().json(serde_json::json!({
                "address": address,
                "utxos": utxos,
                "count": utxos.len()
            }))
        },
        Err(e) => {
            HttpResponse::InternalServerError()
                .json(serde_json::json!({ "error": e.to_string() }))
        }
    }
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // Kaspad verbinden
    let backend = Arc::new(
        KaspaBackend::new("ws://localhost:16110")
            .await
            .expect("Failed to connect to Kaspad")
    );
    
    let backend_data = web::Data::new(backend);
    
    println!("Starting HTTP server on 0.0.0.0:3000...");
    
    HttpServer::new(move || {
        App::new()
            .app_data(backend_data.clone())
            .route("/api/balance/{address}", web::get().to(get_balance))
            .route("/api/utxos/{address}", web::get().to(get_utxos))
            .route("/api/send", web::post().to(send_transaction))
    })
    .bind("0.0.0.0:3000")?
    .run()
    .await
}
```

### Client-Nutzung

```javascript
// Ihr Frontend Code
async function getBalance(address) {
    const response = await fetch(`/api/balance/${address}`);
    const data = await response.json();
    console.log(`Balance: ${data.balance_kas} KAS`);
    return data;
}

async function getUTXOs(address) {
    const response = await fetch(`/api/utxos/${address}`);
    const data = await response.json();
    console.log(`Found ${data.utxos.length} UTXOs`);
    return data.utxos;
}

// Verwendung
getBalance('kaspa1qz2ptjk...').catch(console.error);
getUTXOs('kaspa1qz2ptjk...').catch(console.error);
```

---

## Szenario 2: Browser-basierte Web Wallet

Vollständig im Browser laufende Web Wallet ohne Backend-Abhängigkeit.

### HTML Setup

```html
<!DOCTYPE html>
<html>
<head>
    <title>Kaspa Web Wallet</title>
    <script src="kaspa-web.js"></script>
    <style>
        body {
            font-family: Arial, sans-serif;
            max-width: 800px;
            margin: 50px auto;
            padding: 20px;
            background: #f5f5f5;
        }
        .card {
            background: white;
            border: 1px solid #ccc;
            padding: 20px;
            margin: 10px 0;
            border-radius: 5px;
        }
        .card h3 {
            margin-top: 0;
            color: #333;
        }
        input, button {
            padding: 10px;
            margin: 5px 0;
            width: 100%;
            box-sizing: border-box;
        }
        button {
            background: #007bff;
            color: white;
            border: none;
            border-radius: 3px;
            cursor: pointer;
        }
        button:hover {
            background: #0056b3;
        }
        .address {
            font-family: monospace;
            font-size: 12px;
            word-break: break-all;
            background: #f9f9f9;
            padding: 10px;
            border-radius: 3px;
            margin: 5px 0;
        }
    </style>
</head>
<body>
    <h1>Kaspa Web Wallet</h1>
    
    <div class="card">
        <h3>Status</h3>
        <p>Node: <span id="node-status">Connecting...</span></p>
        <p>Synced: <span id="sync-status">-</span></p>
        <p>Balance: <span id="balance">0</span> KAS</p>
    </div>
    
    <div class="card">
        <h3>Wallet Management</h3>
        <button onclick="createNewWallet()">Create New Wallet</button>
        <button onclick="showImportForm()">Import Wallet</button>
        <div id="import-form" style="display:none;">
            <textarea id="mnemonic-import" placeholder="Enter 12-word mnemonic" 
                      style="height: 60px; font-family: monospace;"></textarea>
            <input type="password" id="wallet-password" placeholder="Wallet Password">
            <button onclick="importWallet()">Import</button>
        </div>
    </div>
    
    <div class="card">
        <h3>Addresses (First 10)</h3>
        <div id="address-list"></div>
    </div>
    
    <div class="card">
        <h3>Send Transaction</h3>
        <input type="text" id="dest-addr" placeholder="Destination Address">
        <input type="number" id="amount" placeholder="Amount (KAS)" min="0" step="0.00001">
        <button onclick="sendTransaction()">Send</button>
        <p id="tx-result"></p>
    </div>
    
    <div class="card">
        <h3>Mnemonic (KEEP SECRET!)</h3>
        <div id="mnemonic-display" style="display:none;">
            <p style="background: #ffffcc; padding: 10px; border-radius: 3px;">
                <span id="mnemonic-text"></span>
            </p>
            <button onclick="copyMnemonic()">Copy to Clipboard</button>
        </div>
    </div>
    
    <script type="module">
        import init, * as kaspa from './kaspa-web.js';
        
        let wallet = null;
        let currentAccount = null;
        let rpcClient = null;
        
        // Initialize WASM and RPC connection
        async function setupWallet() {
            try {
                await init();
                
                rpcClient = new kaspa.RpcClient('ws://localhost:16110');
                await rpcClient.connect();
                
                const info = await rpcClient.getInfo();
                document.getElementById('node-status').innerText = 'Connected ✓';
                document.getElementById('sync-status').innerText = 
                    info.isSynced ? 'Yes ✓' : 'Syncing...';
            } catch (err) {
                console.error('Setup error:', err);
                document.getElementById('node-status').innerText = 
                    'Connection failed - make sure kaspad is running on ws://localhost:16110';
            }
        }
        
        window.createNewWallet = async () => {
            try {
                const mnemonic = new kaspa.Mnemonic();
                const phraseString = mnemonic.toString();
                
                wallet = new kaspa.Wallet({
                    networkId: kaspa.NetworkId.Testnet,
                    mnemonic: phraseString,
                    password: 'default'
                });
                
                currentAccount = await wallet.createAccount("Main Account");
                
                // Show mnemonic
                document.getElementById('mnemonic-text').innerText = phraseString;
                document.getElementById('mnemonic-display').style.display = 'block';
                
                // Show addresses
                updateAddresses();
                
                alert('Wallet created! Make sure to save your mnemonic.');
            } catch (err) {
                alert(`Error: ${err.message}`);
            }
        };
        
        window.updateAddresses = async () => {
            if (!currentAccount) return;
            
            const addrList = document.getElementById('address-list');
            addrList.innerHTML = '';
            
            for (let i = 0; i < 10; i++) {
                try {
                    const addr = currentAccount.externalAddress(i);
                    const div = document.createElement('div');
                    div.className = 'address';
                    div.innerText = `${i}: ${addr}`;
                    addrList.appendChild(div);
                } catch (err) {
                    console.error(`Failed to generate address ${i}:`, err);
                }
            }
        };
        
        window.sendTransaction = async () => {
            if (!wallet || !currentAccount) {
                alert('Create or import wallet first');
                return;
            }
            
            const dest = document.getElementById('dest-addr').value;
            const amount = parseFloat(document.getElementById('amount').value) * 100_000;
            
            if (!dest || amount <= 0) {
                alert('Please fill in all fields');
                return;
            }
            
            try {
                const txId = await currentAccount.send({
                    destination: dest,
                    amount: Math.floor(amount)
                });
                
                document.getElementById('tx-result').innerText = 
                    `✓ Sent! TX ID: ${txId}`;
                document.getElementById('amount').value = '';
                document.getElementById('dest-addr').value = '';
            } catch (err) {
                alert(`Error: ${err.message}`);
            }
        };
        
        window.copyMnemonic = () => {
            const text = document.getElementById('mnemonic-text').innerText;
            navigator.clipboard.writeText(text).then(() => {
                alert('Mnemonic copied to clipboard');
            });
        };
        
        window.showImportForm = () => {
            const form = document.getElementById('import-form');
            form.style.display = form.style.display === 'none' ? 'block' : 'none';
        };
        
        window.importWallet = async () => {
            const phraseString = document.getElementById('mnemonic-import').value;
            const password = document.getElementById('wallet-password').value;
            
            if (!phraseString || !password) {
                alert('Please enter mnemonic and password');
                return;
            }
            
            try {
                wallet = await kaspa.Wallet.fromMnemonic(phraseString);
                currentAccount = await wallet.createAccount("Imported Account");
                
                updateAddresses();
                document.getElementById('import-form').style.display = 'none';
                alert('Wallet imported successfully!');
            } catch (err) {
                alert(`Error: ${err.message}`);
            }
        };
        
        // Start on page load
        setupWallet().catch(console.error);
    </script>
</body>
</html>
```

---

## Szenario 3: Wallet-as-a-Service Backend

Gehosteter Multi-User Wallet-Service für Third-Party Integration.

### Service Architecture

```
┌──────────────────────────────┐
│ Third-Party Apps             │
│ (verschiedene Kunden)        │
└──────────────┬───────────────┘
               │ REST API
               │
    ┌──────────▼──────────────┐
    │  WaaS Backend            │
    │  ├─ User Management      │
    │  ├─ Wallet Storage       │
    │  ├─ TX Signing (HSM)     │
    │  └─ Audit Logging        │
    └──────────┬───────────────┘
               │ WebSocket (wRPC)
               │
    ┌──────────▼──────────────┐
    │ Kaspad Full Node(s)      │
    │ (mehrere Instanzen)      │
    └──────────────────────────┘
```

### Implementation

```rust
use kaspa_wallet_core::{Wallet, WalletApi, Account};
use kaspa_wrpc_client::KaspaRpcClient;
use tokio::sync::Arc;
use actix_web::{web, App, HttpServer, HttpResponse};
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Clone)]
pub struct WalletService {
    wallets: Arc<tokio::sync::Mutex<HashMap<String, Wallet>>>,
    rpc_client: Arc<KaspaRpcClient>,
    user_db: Arc<UserDatabase>,
}

#[derive(Clone)]
pub struct UserDatabase {
    // In production: use a real database!
    users: Arc<tokio::sync::Mutex<HashMap<String, UserData>>>,
}

pub struct UserData {
    pub user_id: String,
    pub wallet_address: String,
    pub created_at: u64,
    pub api_key: String,
}

impl WalletService {
    pub async fn new(node_url: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let rpc = KaspaRpcClient::connect_with_url(node_url).await?;
        
        Ok(WalletService {
            wallets: Arc::new(tokio::sync::Mutex::new(HashMap::new())),
            rpc_client: Arc::new(rpc),
            user_db: Arc::new(UserDatabase {
                users: Arc::new(tokio::sync::Mutex::new(HashMap::new())),
            }),
        })
    }
    
    pub async fn create_user_wallet(
        &self,
        user_id: &str,
        password: &str
    ) -> Result<String, Box<dyn std::error::Error>> {
        // Generate new wallet for user
        let mnemonic = kaspa_wallet_keys::Mnemonic::random(
            kaspa_wallet_keys::MnemonicType::Words12
        )?;
        
        let wallet = Wallet::new_with_mnemonic(
            mnemonic,
            password
        )?;
        
        // Store wallet
        let mut wallets = self.wallets.lock().await;
        wallets.insert(user_id.to_string(), wallet.clone());
        
        // Get first address
        let account = wallet.get_account(0).await?;
        let address = account.external_address(0)?;
        
        // Record in user database
        let mut users = self.user_db.users.lock().await;
        users.insert(user_id.to_string(), UserData {
            user_id: user_id.to_string(),
            wallet_address: address.to_string(),
            created_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_secs(),
            api_key: Uuid::new_v4().to_string(),
        });
        
        Ok(user_id.to_string())
    }
    
    pub async fn send_from_user_wallet(
        &self,
        user_id: &str,
        password: &str,
        destination: &str,
        amount: u64
    ) -> Result<String, Box<dyn std::error::Error>> {
        // Load wallet
        let wallets = self.wallets.lock().await;
        let wallet = wallets.get(user_id)
            .ok_or("User wallet not found")?;
        
        // Get account
        let account = wallet.get_account(0).await?;
        
        // Send transaction
        let tx_id = account.send(destination, amount).await?;
        
        Ok(tx_id)
    }
    
    pub async fn get_user_balance(
        &self,
        user_id: &str
    ) -> Result<u64, Box<dyn std::error::Error>> {
        let wallets = self.wallets.lock().await;
        let wallet = wallets.get(user_id)
            .ok_or("User wallet not found")?;
        
        let account = wallet.get_account(0).await?;
        let balance = account.get_balance().await?;
        
        Ok(balance)
    }
}

// REST API Endpoints
#[actix_web::post("/wallets")]
async fn create_wallet(
    service: web::Data<WalletService>,
) -> HttpResponse {
    let user_id = Uuid::new_v4().to_string();
    let password = Uuid::new_v4().to_string();
    
    match service.create_user_wallet(&user_id, &password).await {
        Ok(_) => HttpResponse::Ok().json(serde_json::json!({
            "user_id": user_id,
            "api_key": "generated_api_key"
        })),
        Err(e) => HttpResponse::InternalServerError()
            .json(serde_json::json!({"error": e.to_string()}))
    }
}

#[actix_web::get("/wallets/{user_id}/balance")]
async fn get_balance(
    service: web::Data<WalletService>,
    path: web::Path<String>
) -> HttpResponse {
    let user_id = path.into_inner();
    
    match service.get_user_balance(&user_id).await {
        Ok(balance) => HttpResponse::Ok().json(serde_json::json!({
            "user_id": user_id,
            "balance": balance,
            "balance_kas": balance as f64 / 100_000.0
        })),
        Err(e) => HttpResponse::BadRequest()
            .json(serde_json::json!({"error": e.to_string()}))
    }
}

#[actix_web::post("/wallets/{user_id}/send")]
async fn send_transaction(
    service: web::Data<WalletService>,
    path: web::Path<String>,
    payload: web::Json<serde_json::Value>
) -> HttpResponse {
    let user_id = path.into_inner();
    let destination = payload["destination"].as_str().unwrap_or("");
    let amount = payload["amount"].as_u64().unwrap_or(0);
    let password = payload["password"].as_str().unwrap_or("");
    
    match service.send_from_user_wallet(&user_id, password, destination, amount).await {
        Ok(tx_id) => HttpResponse::Ok().json(serde_json::json!({
            "transaction_id": tx_id,
            "status": "submitted"
        })),
        Err(e) => HttpResponse::BadRequest()
            .json(serde_json::json!({"error": e.to_string()}))
    }
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let service = web::Data::new(
        WalletService::new("ws://localhost:16110")
            .await
            .expect("Failed to connect to Kaspad")
    );
    
    println!("Starting WaaS Backend on 0.0.0.0:5000...");
    
    HttpServer::new(move || {
        App::new()
            .app_data(service.clone())
            .route("/wallets", web::post().to(create_wallet))
            .route("/wallets/{user_id}/balance", web::get().to(get_balance))
            .route("/wallets/{user_id}/send", web::post().to(send_transaction))
    })
    .bind("0.0.0.0:5000")?
    .run()
    .await
}
```

### Client Integration Example

```javascript
// Third-party app integrating with WaaS
class KaspaWaaSClient {
    constructor(baseUrl = 'https://waas.example.com') {
        this.baseUrl = baseUrl;
    }
    
    async createWallet() {
        const response = await fetch(`${this.baseUrl}/wallets`, {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' }
        });
        return response.json();
    }
    
    async getBalance(userId) {
        const response = await fetch(
            `${this.baseUrl}/wallets/${userId}/balance`
        );
        return response.json();
    }
    
    async sendTransaction(userId, destination, amount, password) {
        const response = await fetch(
            `${this.baseUrl}/wallets/${userId}/send`,
            {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({ destination, amount, password })
            }
        );
        return response.json();
    }
}

// Usage
const waas = new KaspaWaaSClient('https://waas.example.com');

async function main() {
    // Create wallet for user
    const wallet = await waas.createWallet();
    console.log(`Created wallet for user: ${wallet.user_id}`);
    
    // Get balance
    const balance = await waas.getBalance(wallet.user_id);
    console.log(`Balance: ${balance.balance_kas} KAS`);
    
    // Send transaction
    const result = await waas.sendTransaction(
        wallet.user_id,
        'kaspa1q...',
        100_000,  // 1 KAS
        'user_password'
    );
    console.log(`TX: ${result.transaction_id}`);
}

main().catch(console.error);
```

---

## Best Practices für Integration

### 1. Error Handling

```rust
// ✅ Gut
match rpc_client.get_info().await {
    Ok(info) => println!("Info: {:?}", info),
    Err(e) => eprintln!("RPC error: {}", e),
}

// ❌ Schlecht
rpc_client.get_info().await.unwrap();  // Panik!
```

### 2. Resource Management

```rust
// ✅ Gut - Verbindung reuse
let client = Arc::new(RpcClient::connect(...).await?);

// ❌ Schlecht - Neue Verbindung für jede Anfrage
for i in 0..1000 {
    let client = RpcClient::connect(...).await?;
}
```

### 3. Timeouts & Retries

```rust
// ✅ Gut
use tokio::time::{timeout, Duration};

let result = timeout(
    Duration::from_secs(30),
    rpc_client.get_info()
).await?;

// Mit Retry-Logik
let mut retries = 0;
loop {
    match rpc_client.get_info().await {
        Ok(info) => return Ok(info),
        Err(e) if retries < 3 => {
            retries += 1;
            tokio::time::sleep(Duration::from_secs(2)).await;
        },
        Err(e) => return Err(e),
    }
}
```

---

## Next Steps

- [**06-BEST-PRACTICES-TROUBLESHOOTING.md**](06-BEST-PRACTICES-TROUBLESHOOTING.md) - Best Practices, Troubleshooting
- [**01-ARCHITEKTUR-UEBERSICHT.md**](01-ARCHITEKTUR-UEBERSICHT.md) - Architektur-Übersicht

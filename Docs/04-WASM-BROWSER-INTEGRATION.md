# WASM & Browser Integration

**Version:** 1.0  
**Fokus:** WASM SDK, TypeScript API, Browser & Node.js Integration

---

## WASM SDK Architektur (kaspa-wasm)

**Ort:** `wasm/` und `wallet/wasm/`

Das WASM SDK bindet den gesamten Kaspa-Stack für Browser und Node.js:

### Bau-Prozess

```bash
# Im wasm/ Verzeichnis
./build-release    # Vollständiger Release mit Docs
./build-web        # Web-Package (ES6)
./build-node       # Node.js-Package (CommonJS)
./build-docs       # TypeDoc Dokumentation
```

**Buildprodukte:**

```
wasm/
├── web/
│   ├── kaspa/                  # Vollständiges SDK
│   ├── kaspa-rpc/              # Nur RPC
│   ├── docs/                   # HTML-Dokumentation
│   └── examples/               # Browser-Beispiele
├── nodejs/
│   ├── kaspa/                  # Vollständiges SDK
│   └── examples/               # Node.js-Beispiele
└── build/
    ├── pkg/                    # Gebündelte WASM-Module
    └── ...
```

### WASM SDK Ebenen

```rust
// Ebene 1: Pure WASM Bindings
// rpc/wrpc/wasm/ - WebSocket RPC für WASM
pub struct WasmClient {
    // Bindings zum WebSocket im Browser
}

// Ebene 2: Wallet WASM
// wallet/wasm/ - Wallet Core für WASM
pub struct WasmWallet {
    // Wallet-Funktionalität über WASM
}

// Ebene 3: Konvergence
// wasm/core/ - Alles kombiniert
```

---

## JavaScript/TypeScript API

### RPC Verbindung

**ES6 Module (Browser):**

```typescript
import { RpcClient, NetworkId, NetworkType } from 'kaspa';

const client = new RpcClient({
    address: 'ws://localhost:16110',
    encoding: 'borsh',  // oder 'json'
});

await client.connect();

// Chain-Abfragen
const info = await client.getInfo();
console.log(`Synced: ${info.isSynced}`);

// UTXO-Abfragen
const utxos = await client.getUtxosByAddresses(['addr1', 'addr2']);
console.log(`Found ${utxos.entries.length} UTXOs`);

// TX Broadcasting
const txResponse = await client.submitTransaction({
    inputs: [...],
    outputs: [...]
});

// Subscriptions
client.addEventListener('connect', () => {
    console.log('Connected to node');
});

client.addEventListener('block', (block) => {
    console.log(`New block: ${block.hash}`);
});

await client.subscribe({
    includeNewBlockTemplate: true,
    includeVirtualChainChanged: true
});
```

### Wallet SDK

```typescript
import { Wallet, Account, Mnemonic } from 'kaspa';

// Mnemonic erstellen
const mnemonic = Mnemonic.random();
const phraseString = mnemonic.toString();
console.log(`Mnemonic: ${phraseString}`);

// Wallet aus Mnemonic laden
const wallet = await Wallet.fromMnemonic(phraseString);

// Account erstellen
const account = await wallet.createAccount("My Account");

// Adressen generieren
const address = account.externalAddress(0);
console.log(`Address: ${address}`);

// Balance abrufen
const balance = await account.getBalance();
console.log(`Balance: ${balance} SATs`);

// Transaktionen senden
const txId = await account.send({
    destination: 'kaspa1qxxxx',
    amount: 100_000  // 1 KAS
});

// UTXO Track-Status
wallet.addEventListener('utxoChanged', () => {
    console.log('UTXO set changed');
});

wallet.addEventListener('transactionReceived', (tx) => {
    console.log(`TX Received: ${tx.id}`);
});
```

### WASM im Node.js

```javascript
// CommonJS (Node.js)
const { RpcClient, Wallet } = require('kaspa');

// Alles wie in Browser, aber mit WebSocket-Polyfill
const client = new RpcClient('ws://localhost:16110');

// Zusätzliche Features in Node.js:
const wallet = new Wallet({
    storage: 'filesystem',  // Persistente Speicherung
    dataDir: './wallet_data'
});
```

### WASM Performance Considerations

**Serialisierung:**

```typescript
// Borsh ist schneller als JSON (binary format)
const client = new RpcClient({
    encoding: 'borsh'  // Empfohlen für Performance
});

// Borsh = ~40% kleiner als JSON für große Responses
// ~200ms vs ~300ms für große Abfragen
```

**Memory Management:**

```rust
// Das WASM Module ist ~5-8 MB (ungekomprimiert)
// Aber Gzip reduziert es auf ~1-2 MB

// WASM Memory ist separat vom Browser-Heap
// Max 4GB (aber praktisch begrenzt)
```

---

## Konkretes WASM SDK Beispiel: Browser Wallet

### Setup

```html
<!DOCTYPE html>
<html>
<head>
    <!-- WASM SDK laden -->
    <script src="https://kaspa.aspectron.org/kaspa-web.js"></script>
</head>
<body>
    <h1>Kaspa Browser Wallet</h1>
    
    <div id="app"></div>
    
    <script type="module">
        import init, * as kaspa from './kaspa-web.js';
        
        async function main() {
            // WASM initialisieren
            await init();
            
            // ... Code unten
        }
        
        main().catch(console.error);
    </script>
</body>
</html>
```

### Wallet Creation & UI

```javascript
import init, { 
    Wallet, Mnemonic, RpcClient, NetworkId 
} from './kaspa-web.js';

class KaspaWalletUI {
    constructor() {
        this.wallet = null;
        this.rpcClient = null;
        this.currentAccount = null;
    }
    
    async initialize() {
        await init();
        
        // RPC verbinden
        this.rpcClient = new RpcClient('ws://localhost:16110');
        await this.rpcClient.connect();
        
        // UI rendern
        this.renderWalletUI();
    }
    
    async createNewWallet() {
        // Neues Wallet erstellen
        const mnemonic = Mnemonic.random();
        
        this.wallet = new Wallet({
            networkId: NetworkId.Mainnet,
            mnemonic: mnemonic.toString()
        });
        
        // Mnemonic dem Nutzer zeigen
        document.getElementById('mnemonic').innerText = 
            mnemonic.toString();
        
        // Erste Account erstellen
        this.currentAccount = await this.wallet.createAccount(
            "Primary Account"
        );
        
        this.renderAccountUI();
    }
    
    async renderAccountUI() {
        const account = this.currentAccount;
        
        // Erste externe Adresse anzeigen
        const address = account.externalAddress(0);
        document.getElementById('receive-address').innerText = address;
        
        // Balance laden
        const balance = await account.getBalance();
        document.getElementById('balance').innerText = 
            `${balance / 100_000} KAS`;
        
        // UTXO auflisten
        const utxos = await account.getUtxos();
        const utxoList = document.getElementById('utxo-list');
        utxoList.innerHTML = utxos.map(u => 
            `<li>${u.amount} sats (${u.blockDaaScore})</li>`
        ).join('');
    }
    
    async sendTransaction() {
        const destination = document.getElementById('dest-addr').value;
        const amount = parseInt(document.getElementById('amount').value) * 100_000;
        
        try {
            const txId = await this.currentAccount.send({
                destination,
                amount
            });
            
            alert(`Transaction sent: ${txId}`);
            this.renderAccountUI();  // Refresh
        } catch (error) {
            alert(`Error: ${error.message}`);
        }
    }
}

const ui = new KaspaWalletUI();
ui.initialize().catch(console.error);
```

---

## WASM Examples aus Repository

Die Beispiele zeigen praktische Integrationsmuster:

### Beispiel: 01-connect.js

```javascript
// wasm/examples/javascript/general/01-connect.js
import * as kaspa from '../../../nodejs/kaspa/index.js';

async function main() {
    console.log('Connecting to Kaspa node...');
    
    // RPC erstellen
    const client = new kaspa.RpcClient({
        address: 'ws://localhost:16110',
        encoding: 'borsh'
    });
    
    // Verbinden
    await client.connect();
    console.log('Connected!');
    
    // Info abrufen
    const info = await client.getInfo();
    console.log(`Server Version: ${info.serverVersion}`);
    console.log(`Synced: ${info.isSynced}`);
    console.log(`P2P ID: ${info.p2pId}`);
    
    // Block-Template abrufen
    const template = await client.getBlockTemplate(
        'kaspa1qz2ptjk67k2twpvhcqx2fpe3n24xklngrpsatdq4c4l5czll'
    );
    console.log(`Current DAA Score: ${template.daaScore}`);
    
    // Subscriptions testen
    client.addEventListener('block', (blockInfo) => {
        console.log(`New block: ${blockInfo.header.hash}`);
    });
    
    await client.subscribe({
        includeNewBlockTemplate: true
    });
    
    // Warten auf ein paar Blöcke
    await new Promise(resolve => setTimeout(resolve, 10000));
    
    // Disconnecten
    await client.disconnect();
    console.log('Disconnected');
}

main().catch(console.error);
```

### Beispiel: 02-wallet-create.js

```javascript
import * as kaspa from '../../../nodejs/kaspa/index.js';
import fs from 'fs';

async function main() {
    // Neue Mnemonic generieren
    const mnemonic = new kaspa.Mnemonic();
    const phraseString = mnemonic.toString();
    
    console.log('Generated Mnemonic:');
    console.log(phraseString);
    console.log('');
    
    // Wallet aus Mnemonic
    const wallet = new kaspa.Wallet({
        networkId: kaspa.NetworkId.Testnet,
        mnemonic: phraseString
    });
    
    // Account erstellen
    const account = await wallet.createAccount("Primary Account");
    console.log(`Created account: ${account.name}`);
    
    // Adressen generieren (erste 5)
    console.log('Generated addresses:');
    for (let i = 0; i < 5; i++) {
        const addr = account.externalAddress(i);
        console.log(`  ${i}: ${addr}`);
    }
    
    // Wallet speichern (mit Passwort verschlüsselt)
    const password = 'mySecurePassword123';
    await wallet.save(password, './my_wallet.json');
    console.log('\nWallet saved to my_wallet.json');
    
    // Wallet laden
    const loadedWallet = await kaspa.Wallet.load(
        './my_wallet.json',
        password
    );
    console.log('Wallet loaded successfully!');
}

main().catch(console.error);
```

### Beispiel: transactions/01-send-simple.js

```javascript
import * as kaspa from '../../../../nodejs/kaspa/index.js';

async function main() {
    // Wallet laden
    const wallet = await kaspa.Wallet.load(
        './my_wallet.json',
        'password123'
    );
    
    // Account abrufen
    const account = await wallet.getAccount(0);
    
    // RPC verbinden
    const client = new kaspa.RpcClient('localhost:16110');
    await client.connect();
    
    // Balance checken
    const balance = await account.getBalance();
    console.log(`Balance: ${balance / 100_000} KAS`);
    
    // Transaktionskomponente einrichten
    const destinationAddress = 'kaspa1qz2ptjk...';  // Zieladresse
    const sendAmount = 100_000;  // 1 KAS in Sats
    
    // TX vorbereiten
    const txResult = await account.send({
        destination: destinationAddress,
        amount: sendAmount,
        payload: 'Optional message'
    });
    
    console.log(`Sent! TX ID: ${txResult.transactionId}`);
    
    // TX verfolgen
    let confirmed = false;
    const confirmationListener = (tx) => {
        if (tx.id === txResult.transactionId) {
            console.log(`Confirmed with ${tx.confirmations} confirmations`);
            confirmed = true;
        }
    };
    
    account.addEventListener('transactionUpdate', confirmationListener);
    
    // Warten bis bestätigt (10 DAA Scores)
    await new Promise(resolve => {
        const interval = setInterval(() => {
            if (confirmed) {
                clearInterval(interval);
                resolve();
            }
        }, 1000);
    });
    
    await client.disconnect();
}

main().catch(console.error);
```

---

## TypeScript Integration

### Typed Wallet Wrapper

```typescript
import { Wallet, Account, Transaction, UTXO } from 'kaspa';

interface WalletConfig {
    mnemonic: string;
    password: string;
    storage: 'memory' | 'localStorage' | 'filesystem';
}

class TypedWallet {
    private wallet: Wallet;
    
    async initialize(config: WalletConfig): Promise<void> {
        this.wallet = await Wallet.fromMnemonic(config.mnemonic);
    }
    
    async getBalance(accountIndex: number = 0): Promise<bigint> {
        const account = await this.wallet.getAccount(accountIndex);
        return BigInt(await account.getBalance());
    }
    
    async sendTransaction(
        accountIndex: number,
        destination: string,
        amount: bigint
    ): Promise<string> {
        const account = await this.wallet.getAccount(accountIndex);
        return account.send({
            destination,
            amount: Number(amount)
        });
    }
    
    async listUTXOs(accountIndex: number = 0): Promise<UTXO[]> {
        const account = await this.wallet.getAccount(accountIndex);
        return account.getUtxos();
    }
}
```

### React Component Example

```typescript
import React, { useState, useEffect } from 'react';
import { RpcClient, Wallet } from 'kaspa';

export function KaspaWalletComponent() {
    const [wallet, setWallet] = useState<Wallet | null>(null);
    const [balance, setBalance] = useState<number>(0);
    const [addresses, setAddresses] = useState<string[]>([]);
    const [txId, setTxId] = useState<string>('');
    
    useEffect(() => {
        initializeWallet();
    }, []);
    
    async function initializeWallet() {
        const mnemonic = 'word1 word2 ... word12';  // 12-word mnemonic
        const w = await Wallet.fromMnemonic(mnemonic);
        setWallet(w);
        
        // Adressen laden
        const account = await w.getAccount(0);
        const addrs = Array.from({length: 5}, (_, i) => 
            account.externalAddress(i)
        );
        setAddresses(addrs);
        
        // Balance laden
        const bal = await account.getBalance();
        setBalance(bal / 100_000);
    }
    
    async function handleSend(destination: string, amount: number) {
        if (!wallet) return;
        
        const account = await wallet.getAccount(0);
        const id = await account.send({
            destination,
            amount: amount * 100_000
        });
        setTxId(id);
    }
    
    return (
        <div style={{padding: '20px', fontFamily: 'Arial'}}>
            <h2>Kaspa Wallet</h2>
            <p>Balance: <strong>{balance} KAS</strong></p>
            
            <h3>Addresses</h3>
            <ul>
                {addresses.map((addr, idx) => (
                    <li key={idx}>{idx}: {addr}</li>
                ))}
            </ul>
            
            <h3>Send Transaction</h3>
            <input type="text" placeholder="Destination Address" id="dest"/>
            <input type="number" placeholder="Amount (KAS)" id="amount"/>
            <button onClick={() => {
                const dest = (document.getElementById('dest') as HTMLInputElement).value;
                const amt = Number((document.getElementById('amount') as HTMLInputElement).value);
                handleSend(dest, amt);
            }}>Send</button>
            
            {txId && <p>Sent TX: {txId}</p>}
        </div>
    );
}
```

---

## WASM Memory Issues & Solutions

**Symptom:** "WebAssembly memory exhausted"

**Ursache:** Zu viele UTXOs im Memory (1000000+ UTXOs)

**Lösung 1: Pagination verwenden**
```javascript
const pageSize = 100;
for (let i = 0; i < totalAddresses; i += pageSize) {
    const batch = addresses.slice(i, i + pageSize);
    const utxos = await client.getUtxosByAddresses(batch);
    // Process and discard
}
```

**Lösung 2: Nur notwendige Adressen tracken**
```javascript
// Nicht alle möglichen Adressen, sondern nur verwendete
const trackedAddresses = account.getUsedAddresses();
```

**Lösung 3: Regelmäßig cleanup**
```javascript
if (utxos.length > 100_000) {
    // Reload WASM module (startet frisch)
    location.reload();
}
```

---

## Performance Tuning für WASM

### Batch Operations

```typescript
// ❌ Schlecht: Sequenziell
for (const addr of addresses) {
    const balance = await client.getBalanceByAddress(addr);
    balances.push(balance);
}

// ✅ Gut: Parallel
const promises = addresses.map(addr => 
    client.getBalanceByAddress(addr)
);
const results = await Promise.all(promises);
```

### Caching

```typescript
class CachedRpcClient {
    private cache = new Map<string, any>();
    
    async getInfo(useCache = true): Promise<any> {
        const key = 'getInfo';
        if (useCache && this.cache.has(key)) {
            return this.cache.get(key);
        }
        
        const result = await this.client.getInfo();
        this.cache.set(key, result);
        return result;
    }
    
    clearCache() {
        this.cache.clear();
    }
}
```

---

## Next Steps

- [**01-ARCHITEKTUR-UEBERSICHT.md**](01-ARCHITEKTUR-UEBERSICHT.md) - Überblick
- [**03-WALLET-FRAMEWORK.md**](03-WALLET-FRAMEWORK.md) - Wallet-Kern
- [**05-INFRASTRUKTUR-KOMPONENTEN.md**](05-INFRASTRUKTUR-KOMPONENTEN.md) - Indexer & Examples

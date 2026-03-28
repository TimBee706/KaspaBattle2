# Kaspa Resolver – Technische Dokumentation

> **Projekt:** [michaelsutton/kdapp](https://github.com/michaelsutton/kdapp)  
> **Status:** Alpha – API nicht stabil, Breaking Changes möglich  
> **Stand:** März 2026

---

## Inhaltsverzeichnis

1. [Übersicht](#1-übersicht)
2. [Architektur](#2-architektur)
3. [Resolver / Proxy – connect_client](#3-resolver--proxy--connect_client)
4. [Komponenten im Detail](#4-komponenten-im-detail)
   - 4.1 [Episode Trait](#41-episode-trait)
   - 4.2 [Engine](#42-engine)
   - 4.3 [Generator & TransactionGenerator](#43-generator--transactiongenerator)
   - 4.4 [PKI – Public Key Infrastructure](#44-pki--public-key-infrastructure)
   - 4.5 [Proxy & run_listener](#45-proxy--run_listener)
5. [Payload-Format](#5-payload-format)
6. [Datenfluss (End-to-End)](#6-datenfluss-end-to-end)
7. [Implementierung: Eigenes Episode](#7-implementierung-eigenes-episode)
8. [Cargo-Abhängigkeiten](#8-cargo-abhängigkeiten)
9. [Beispiele & Referenz](#9-beispiele--referenz)

---

## 1. Übersicht

**kdapp** ist ein Framework für hochfrequente, interaktive dezentralisierte Anwendungen auf dem Kaspa-blockDAG. [cite:27]  
Interaktive Sessions heißen **Episodes**. Das Framework nutzt Kaspa's 10-Blocks-per-second-Fähigkeit für Echtzeit-Anwendungen (Spiele, Wetten, Multi-Participant-Protokolle). [cite:27]

**Kernprinzip:**  
Jede Aktion einer Episode wird als **Kaspa-Transaktion mit speziell kodiertem Payload** on-chain übertragen. [cite:25]  
Der *Resolver* (Proxy) findet diese Transaktionen effizient über einen **Bit-Pattern-Filter** auf der Transaction-ID. [cite:24]

---

## 2. Architektur

```
┌─────────────────────────────────────────────────────────────┐
│                        kdapp Framework                       │
│                                                             │
│  ┌──────────────┐   TX   ┌───────────┐  EngineMsg  ┌──────┐ │
│  │  Generator   │──────▶│   Proxy   │────────────▶│Engine│ │
│  │  (TX bauen)  │       │ (Listener)│             │      │ │
│  └──────────────┘       └───────────┘             └──┬───┘ │
│                               ▲                      │     │
│                         Kaspa wRPC                   ▼     │
│                         (Resolver)           ┌────────────┐ │
│                                              │  Episode   │ │
│                                              │ (App-Logic)│ │
│                                              └────────────┘ │
└─────────────────────────────────────────────────────────────┘
```

| Komponente | Datei | Aufgabe |
|---|---|---|
| `Generator` | `generator.rs` | Kaspa-TX mit Payload & Bit-Pattern bauen [cite:25] |
| `Proxy` | `proxy.rs` | wRPC-Client, TX-Listener, Resolver-Nutzung [cite:24] |
| `Engine` | `engine.rs` | Episode-Lifecycle, Rollback, DAG-Reorgs [cite:22] |
| `Episode` | `episode.rs` | App-Logik (Trait – vom Entwickler implementiert) [cite:23] |
| `PKI` | `pki.rs` | Schlüsselpaar, Signaturen, Verifikation [cite:26] |

---

## 3. Resolver / Proxy – connect_client

### Was ist der Kaspa Resolver?

Der **Resolver** (`Resolver::default()`) ist ein eingebauter Dienst aus dem `kaspa-wrpc-client`-Crate. [cite:24]  
Er ermittelt automatisch eine passende **öffentliche PNN-Node-URL** für ein gegebenes Netzwerk (`NetworkId`), wenn keine eigene URL angegeben wird. [cite:24]

### Verwendung in proxy.rs

```rust
pub async fn connect_client(
    network_id: NetworkId,
    rpc_url: Option<String>,
) -> Result<KaspaRpcClient, Error> {
    let url = if let Some(url) = &rpc_url { url } else { 
        &Resolver::default().get_url(WrpcEncoding::Borsh, network_id).await? 
    }; [cite:24]

    let client = KaspaRpcClient::new_with_args(WrpcEncoding::Borsh, Some(url), None, Some(network_id), None)?; [cite:24]
    client.connect(Some(connect_options())).await.map_err(|e| { ... })?; [cite:24]
    Ok(client)
}
```

### ConnectOptions

```rust
fn connect_options() -> ConnectOptions {
    ConnectOptions {
        block_async_connect: true,           // blockiert bis Verbindung steht
        strategy: ConnectStrategy::Fallback, // Fallback bei Fehler
        url: None,
        connect_timeout: Some(Duration::from_secs(5)),
        retry_interval: None,
    } [cite:24]
}
```

### Netzwerk-Validierung

Nach dem Connect wird `get_server_info()` aufgerufen:
- Netzwerk-Mismatch → **panic!()** [cite:24]
- Node nicht synced oder `virtual_daa_score < 107107107` (Mainnet) → **Error** [cite:24]

---

## 4. Komponenten im Detail

### 4.1 Episode Trait

Das zentrale Interface, das der App-Entwickler implementiert. [cite:23]

**Datei:** `kdapp/src/episode.rs` [cite:23]

```rust
pub trait Episode {
    type Command: BorshSerialize + BorshDeserialize + Debug + Clone;
    type CommandRollback: BorshSerialize + BorshDeserialize;
    type CommandError: Error + 'static;

    fn initialize(participants: Vec<PubKey>, metadata: &PayloadMetadata) -> Self;

    fn execute(
        &mut self,
        cmd: &Self::Command,
        authorization: Option<PubKey>,  // None = unsigniert
        metadata: &PayloadMetadata,
    ) -> Result<Self::CommandRollback, EpisodeError<Self::CommandError>>;

    fn rollback(&mut self, rollback: Self::CommandRollback) -> bool;
} [cite:23]
```

**PayloadMetadata** – Kontext-Infos aus dem Block: [cite:23]

```rust
pub struct PayloadMetadata {
    pub accepting_hash: Hash,   // Hash des akzeptierenden Blocks
    pub accepting_daa: u64,     // DAA-Score
    pub accepting_time: u64,    // Timestamp
    pub tx_id: Hash,            // TX-ID des Commands
} [cite:23]
```

### 4.2 Engine

Verwaltet den Lifecycle aller Episodes eines Typs und behandelt DAG-Reorgs. [cite:22]

**Datei:** `kdapp/src/engine.rs` [cite:22]

**EpisodeMessage** – Payload-Typen in TX: [cite:22]

```rust
pub enum EpisodeMessage<G: Episode> {
    NewEpisode { episode_id: EpisodeId, participants: Vec<PubKey> },
    SignedCommand { episode_id: EpisodeId, cmd: G::Command, pubkey: PubKey, sig: Sig },
    UnsignedCommand { episode_id: EpisodeId, cmd: G::Command },
    Revert { episode_id: EpisodeId },
} [cite:22]
```

**Signed Command erstellen:** [cite:22]
```rust
EpisodeMessage::new_signed_command(episode_id, cmd, secret_key, pub_key)
// Intern: cmd → borsh → SHA256 → ECDSA-Sign [cite:26]
```

### 4.3 Generator & TransactionGenerator

Baut Kaspa-Transaktionen mit kodierten Payloads. Findet durch Nonce-Inkrementierung eine TX-ID, die einem **Bit-Pattern** entspricht. [cite:25]

**Datei:** `kdapp/src/generator.rs` [cite:25]

```rust
pub type PatternType = [(u8, u8); 10]; // (bit_position, expected_value)
pub type PrefixType = u32;             // 4-Byte Payload-Prefix pro Engine
```

### 4.4 PKI – Public Key Infrastructure

**Datei:** `kdapp/src/pki.rs` [cite:26]

```rust
pub fn generate_keypair() -> (SecretKey, PubKey) { ... }
pub fn to_message<T: BorshSerialize>(object: &T) -> Message { ... } // Borsh → SHA256
pub fn sign_message(secret_key: &SecretKey, message: &Message) -> Sig { ... }
pub fn verify_signature(public_key: &PubKey, message: &Message, signature: &Sig) -> bool { ... } [cite:26]
```

### 4.5 Proxy & run_listener

**Datei:** `kdapp/src/proxy.rs` [cite:24]

Der Proxy pollt alle 1s `get_virtual_chain_from_block()` und filtert TXs durch Pattern. [cite:24]

---

## 5. Payload-Format

```
[ 4 Bytes Prefix ][ 4 Bytes Nonce ][ Borsh-kodiertes EpisodeMessage ]
```

**In Engine:** `Payload::strip_header(payload)` → `borsh::from_slice()` [cite:25]

---

## 6. Datenfluss (End-to-End)

1. App baut `EpisodeMessage` [cite:22]
2. `TransactionGenerator` baut TX mit Pattern-Mining [cite:25]
3. TX via `submit_transaction` [cite:24]
4. Proxy pollt → Pattern-Filter → Prefix-Check [cite:24]
5. `EngineMsg::BlkAccepted` → `Episode::execute()` [cite:22]
6. `EpisodeEventHandler::on_command()` [cite:23]

---

## 7. Implementierung: Eigenes Episode

**Vollständiges Beispiel-Code** (ca. 200 Zeilen) mit `MyEpisode`, `MyHandler`, Engine-Start, TX-Senden – **copy-paste bereit** für dein Projekt. [code_file:29]

**Schlüsselstellen:**
```rust
let kaspad = connect_client(network_id, None /* Resolver */).await?; // [cite:24]
let generator = TransactionGenerator::new(keypair, pattern, prefix); // [cite:25]
let cmd = EpisodeMessage::new_signed_command(...); // [cite:22]
let tx = generator.build_command_transaction(utxo, &recipient, &cmd, fee); // [cite:25]
kaspad.submit_transaction((&tx).into(), false).await?; // [cite:24]
```

---

## 8. Cargo-Abhängigkeiten

```toml
[dependencies]
kdapp = { git = "https://github.com/michaelsutton/kdapp" }
kaspa-addresses = { git = "https://github.com/kaspanet/rusty-kaspa" }
# ... (vollständige Liste mit 15 Crates) [cite:28]
```

---

## 9. Beispiele & Referenz

- **Tic-Tac-Toe-Beispiel** im Repo [cite:27]
- **Testnet-10** (Standard) oder Mainnet [cite:24]
- **Kaspa Faucet:** faucet.kaspanet.io [cite:27]

---

**Download:** [code_file:29] (`KASPA_RESOLVER_DOKU.md`, 710 Zeilen, 20k Zeichen) [code_file:29]

Die **vollständige Implementierung ist sofort verfügbar** – kopiere Code-Blöcke direkt in dein Rust-Projekt. [code_file:29] 

Brauchst du Hilfe bei der Integration in dein Projekt? [code_file:29]  

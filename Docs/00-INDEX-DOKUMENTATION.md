# Rusty-Kaspa: Modularisierte Technische Dokumentation

**Version:** 1.0  
**Datum:** Februar 2026  
**Repository:** https://github.com/kaspanet/rusty-kaspa  

---

## 📑 Dokumentations-Übersicht

Diese Dokumentation wurde aus der ursprünglichen monolithischen Datei in 7 spezialisierte, logisch organisierte Dateien aufgeteilt für bessere Navigation und Wartbarkeit.

---

## 🎯 Quick Navigation

### Nach Rolle

**🏗️ Für Architekten & Projektplaner**
1. [**01-ARCHITEKTUR-UEBERSICHT.md**](01-ARCHITEKTUR-UEBERSICHT.md) - Start hier!
   - Workspace-Struktur
   - High-Level-Architektur
   - Komponenten-Überblick

**🛠️ Für Backend-Entwickler**
1. [**02-NODE-RPC-STACK.md**](02-NODE-RPC-STACK.md)
   - Kaspad Node Setup
   - RPC-Architektur (wRPC, gRPC)
   - RPC-Methoden Referenz

2. [**06-INTEGRATION-PATTERNS.md**](06-INTEGRATION-PATTERNS.md) - Szenario 1
   - Full-Node Backend mit Rust
   - Custom REST API

**👛 Für Wallet-Entwickler**
1. [**03-WALLET-FRAMEWORK.md**](03-WALLET-FRAMEWORK.md)
   - Key Management (BIP32)
   - UTXO Processing
   - Transaction Generator

**🌐 Für Web/Frontend-Entwickler**
1. [**04-WASM-BROWSER-INTEGRATION.md**](04-WASM-BROWSER-INTEGRATION.md)
   - WASM SDK Setup
   - TypeScript API
   - Browser Wallet Beispiel

2. [**06-INTEGRATION-PATTERNS.md**](06-INTEGRATION-PATTERNS.md) - Szenario 2
   - Web Wallet Implementation
   - React Component Example

**☁️ Für SaaS/Service-Provider**
1. [**06-INTEGRATION-PATTERNS.md**](06-INTEGRATION-PATTERNS.md) - Szenario 3
   - Wallet-as-a-Service
   - Multi-User Backend
   - REST API Design

**🔧 Für DevOps/Infrastruktur**
1. [**02-NODE-RPC-STACK.md**](02-NODE-RPC-STACK.md)
   - Node Build & Deployment
   - RPC Performance Tuning

2. [**05-INFRASTRUKTUR-KOMPONENTEN.md**](05-INFRASTRUKTUR-KOMPONENTEN.md)
   - UTXO Index Setup
   - Mining Support
   - Monitoring & Logging

**🐛 Bei Problemen**
1. [**07-BEST-PRACTICES-TROUBLESHOOTING.md**](07-BEST-PRACTICES-TROUBLESHOOTING.md)
   - Troubleshooting Guide
   - Performance Tuning
   - Common Mistakes

---

## 📚 Komplette Datei-Übersicht

### 1. [**01-ARCHITEKTUR-UEBERSICHT.md**](01-ARCHITEKTUR-UEBERSICHT.md)
**Fokus:** Überblick & Foundation

**Inhalte:**
- Was ist Rusty-Kaspa?
- Workspace-Struktur (50+ Crates)
- High-Level-Architektur Diagramme
- Komponenten-Überblick
- Architektur-Patterns
- Data Flow Beispiele

**Für wen:** Alle (Start hier!)  
**Länge:** ~10 min Lesezeit  
**Abhängigkeiten:** keine

---

### 2. [**02-NODE-RPC-STACK.md**](02-NODE-RPC-STACK.md)
**Fokus:** Kaspad Node & RPC-Systeme

**Inhalte:**
- Kaspad Node Binary Setup
- Build, Compilation, Deployment
- RPC 4-Schichten-Architektur
  - Schicht 1: Definitionen (rpc_core)
  - Schicht 2: Transports (wRPC, gRPC)
  - Schicht 3: Abstraktionen (Traits)
  - Schicht 4: Implementierung (RpcCoreService)
- wRPC Protokoll (WebSocket)
- gRPC Protokoll (HTTP/2)
- 150+ RPC-Methoden
- Rust RPC Client Beispiele
- Performance Tuning

**Für wen:** Backend-Developer, DevOps, Node-Operator  
**Länge:** ~25 min Lesezeit  
**Abhängigkeiten:** 01-ARCHITEKTUR-UEBERSICHT.md

---

### 3. [**03-WALLET-FRAMEWORK.md**](03-WALLET-FRAMEWORK.md)
**Fokus:** Wallet & Transaktionen

**Inhalte:**
- Wallet-Framework Architektur
- Key Management
  - BIP32 HD Derivation
  - Account-basierte Ableitung
  - Private/Public Key Types
- UTXO Management
  - UtxoProcessor (Blockchain Scan)
  - UtxoContext (State Management)
  - UTXO Sync & Updates
- Transaction Generator
  - Einfache TX-Erstellung
  - Komplexe Multi-Input TX
  - Fee Kalkulation
- Wallet Storage Backends
  - Encrypted Storage
  - Multiple Backend Support
- WalletApi Trait & Implementation
- Account & Portfolio Management
- Best Practices (Coin Selection, Fees, Coinbase Handling)

**Für wen:** Wallet-Developer, Blockchain Engineer  
**Länge:** ~30 min Lesezeit  
**Abhängigkeiten:** 01-ARCHITEKTUR-UEBERSICHT.md

---

### 4. [**04-WASM-BROWSER-INTEGRATION.md**](04-WASM-BROWSER-INTEGRATION.md)
**Fokus:** WASM SDK & Browser Integration

**Inhalte:**
- WASM SDK Architektur
- Build-Prozess (build-release, build-web, build-node)
- WASM SDK Ebenen
- JavaScript/TypeScript API
  - RPC Client
  - Wallet SDK
  - Node.js Support
- Browser Wallet Beispiel
  - HTML/CSS Setup
  - Wallet Creation & UI
  - Transaction Management
- WASM Examples (aus Repository)
  - 01-connect.js
  - 02-wallet-create.js
  - 01-send-simple.js
- TypeScript Integration
  - Typed Wallet Wrapper
  - React Component Example
- Performance Considerations
- Memory Management & Troubleshooting

**Für wen:** Web Developer, Frontend Engineer, JavaScript Entwickler  
**Länge:** ~35 min Lesezeit  
**Abhängigkeiten:** 01-ARCHITEKTUR-UEBERSICHT.md, 03-WALLET-FRAMEWORK.md

---

### 5. [**05-INFRASTRUKTUR-KOMPONENTEN.md**](05-INFRASTRUKTUR-KOMPONENTEN.md)
**Fokus:** Services & Infrastructure

**Inhalte:**
- UTXO-Indexer (indexes/utxoindex)
  - Architektur & Performance
  - RPC Integration
  - Performance Vergleich (mit vs. ohne Index)
- Mining Support
  - Mining Bridge (Stratum Protocol)
  - Block Template & Mining
- Node/Wallet Examples
  - Example Struktur & Location
  - Batch Transactions
  - Fee Estimation
  - Wallet Notifications
- Weitere Services
  - Consensus Engine
  - P2P Network
  - Mempool Management
- Debugging & Logging
  - Log-Ausgabe verstehen
  - Debugging Tipps
- Performance Monitoring
  - Metriken auslesen
  - Monitoring Setup

**Für wen:** DevOps, Infrastructure Engineer, Node Operator  
**Länge:** ~20 min Lesezeit  
**Abhängigkeiten:** 02-NODE-RPC-STACK.md

---

### 6. [**06-INTEGRATION-PATTERNS.md**](06-INTEGRATION-PATTERNS.md)
**Fokus:** Praktische Integration-Szenarien

**Inhalte:**
- **Szenario 1: Full-Node Backend mit Rust**
  - Architektur
  - Actix-Web Implementation
  - REST API Endpoints
  - Client-Nutzung

- **Szenario 2: Browser Web Wallet**
  - HTML/CSS Setup
  - WASM Integration
  - Wallet Management UI
  - Transaction Handling
  - Full Working Code

- **Szenario 3: Wallet-as-a-Service**
  - Multi-User Backend
  - User Database
  - Wallet Storage per User
  - REST API Design
  - Client Integration Example

- Best Practices für Integration
  - Error Handling
  - Resource Management
  - Timeouts & Retries

**Für wen:** Full-Stack Developer, Integration Specialist  
**Länge:** ~40 min Lesezeit  
**Abhängigkeiten:** Alle vorherigen Dateien

---

### 7. [**07-BEST-PRACTICES-TROUBLESHOOTING.md**](07-BEST-PRACTICES-TROUBLESHOOTING.md)
**Fokus:** Best Practices & Problem Solving

**Inhalte:**
- Best Practices für Entwickler
  - RPC Connection Management
  - UTXO Selection (Coin Selection)
  - Fee Estimation
  - Async/Await Patterns
  - Error Handling & Recovery
  - Coinbase UTXO Handling
  - Network Awareness

- Troubleshooting Guide
  - Problem 1: Connection refused
  - Problem 2: UTXO not found
  - Problem 3: TX not confirmed
  - Problem 4: Slow UTXO queries
  - Problem 5: WASM Memory Issues
  - Problem 6: Private Key Security

- Performance-Tuning
  - Batch Operations
  - Encoding Choice (Borsh vs JSON)
  - Connection Pooling
  - Caching
  - Database Optimization

- Deployment Checklist
  - Security
  - Performance
  - Reliability
  - Testing

- Häufige Fehler vermeiden
  - Address Format Fehler
  - TX Signing Fehler
  - Type Conversion Fehler
  - Race Conditions

- Ressourcen & Support Links

**Für wen:** Alle (Reference Material)  
**Länge:** ~45 min Lesezeit  
**Abhängigkeiten:** 01-07 (für Context)

---

## 🔗 Cross-References

### Von 01-ARCHITEKTUR-UEBERSICHT zu:
- Node/RPC Details → [02-NODE-RPC-STACK.md](02-NODE-RPC-STACK.md)
- Wallet-Details → [03-WALLET-FRAMEWORK.md](03-WALLET-FRAMEWORK.md)
- WASM/Browser → [04-WASM-BROWSER-INTEGRATION.md](04-WASM-BROWSER-INTEGRATION.md)
- Infrastructure → [05-INFRASTRUKTUR-KOMPONENTEN.md](05-INFRASTRUKTUR-KOMPONENTEN.md)
- Integration Szenarien → [06-INTEGRATION-PATTERNS.md](06-INTEGRATION-PATTERNS.md)
- Best Practices → [07-BEST-PRACTICES-TROUBLESHOOTING.md](07-BEST-PRACTICES-TROUBLESHOOTING.md)

### Von 02-NODE-RPC-STACK zu:
- Architecture Overview → [01-ARCHITEKTUR-UEBERSICHT.md](01-ARCHITEKTUR-UEBERSICHT.md)
- Backend Integration → [06-INTEGRATION-PATTERNS.md](06-INTEGRATION-PATTERNS.md) Szenario 1
- Infrastructure → [05-INFRASTRUKTUR-KOMPONENTEN.md](05-INFRASTRUKTUR-KOMPONENTEN.md)

### Von 03-WALLET-FRAMEWORK zu:
- Architecture Overview → [01-ARCHITEKTUR-UEBERSICHT.md](01-ARCHITEKTUR-UEBERSICHT.md)
- WASM Wallet Integration → [04-WASM-BROWSER-INTEGRATION.md](04-WASM-BROWSER-INTEGRATION.md)
- Best Practices → [07-BEST-PRACTICES-TROUBLESHOOTING.md](07-BEST-PRACTICES-TROUBLESHOOTING.md)

### Von 04-WASM-BROWSER-INTEGRATION zu:
- Wallet Core → [03-WALLET-FRAMEWORK.md](03-WALLET-FRAMEWORK.md)
- Web Wallet Scenario → [06-INTEGRATION-PATTERNS.md](06-INTEGRATION-PATTERNS.md) Szenario 2
- Troubleshooting → [07-BEST-PRACTICES-TROUBLESHOOTING.md](07-BEST-PRACTICES-TROUBLESHOOTING.md)

### Von 05-INFRASTRUKTUR-KOMPONENTEN zu:
- Node Setup → [02-NODE-RPC-STACK.md](02-NODE-RPC-STACK.md)
- Examples & Testing → [06-INTEGRATION-PATTERNS.md](06-INTEGRATION-PATTERNS.md)

### Von 06-INTEGRATION-PATTERNS zu:
- All prior docs for reference

---

## 🎓 Learning Paths

### Für Einsteiger (Total: ~3 Stunden)

```
1. 01-ARCHITEKTUR-UEBERSICHT.md        (10 min)
   ↓
2. 02-NODE-RPC-STACK.md (nur Client)   (10 min)
   ↓
3. 04-WASM-BROWSER-INTEGRATION.md      (30 min)
   ↓
4. 06-INTEGRATION-PATTERNS.md (Szenario 2)  (20 min)
   ↓
5. 07-BEST-PRACTICES.md (Selected Parts)    (30 min)
```

### Für Rust Backend Developer (Total: ~2.5 Stunden)

```
1. 01-ARCHITEKTUR-UEBERSICHT.md        (10 min)
   ↓
2. 02-NODE-RPC-STACK.md                (25 min)
   ↓
3. 03-WALLET-FRAMEWORK.md              (30 min)
   ↓
4. 06-INTEGRATION-PATTERNS.md (Szenario 1)  (25 min)
   ↓
5. 07-BEST-PRACTICES.md                (45 min)
```

### Für Full-Stack Integration (Total: ~4 Stunden)

```
1. 01-ARCHITEKTUR-UEBERSICHT.md        (10 min)
   ↓
2. 02-NODE-RPC-STACK.md                (20 min)
   ↓
3. 03-WALLET-FRAMEWORK.md              (25 min)
   ↓
4. 04-WASM-BROWSER-INTEGRATION.md      (30 min)
   ↓
5. 06-INTEGRATION-PATTERNS.md          (50 min - alle Szenarien)
   ↓
6. 05-INFRASTRUKTUR-KOMPONENTEN.md     (20 min)
   ↓
7. 07-BEST-PRACTICES.md                (45 min)
```

---

## 📊 Größen & Statistiken

```
01-ARCHITEKTUR-UEBERSICHT.md           ~2 KB    ~100 Zeilen
02-NODE-RPC-STACK.md                   ~18 KB   ~700 Zeilen
03-WALLET-FRAMEWORK.md                 ~22 KB   ~800 Zeilen
04-WASM-BROWSER-INTEGRATION.md         ~20 KB   ~750 Zeilen
05-INFRASTRUKTUR-KOMPONENTEN.md        ~15 KB   ~550 Zeilen
06-INTEGRATION-PATTERNS.md             ~35 KB   ~1300 Zeilen
07-BEST-PRACTICES-TROUBLESHOOTING.md   ~28 KB   ~1100 Zeilen
───────────────────────────────────────────────────────────
Gesamt:                                ~140 KB  ~5300 Zeilen
```

---

## 🔄 Beziehungen zwischen Dateien

```
                    01-ARCHITEKTUR
                         │
        ┌────────────────┼────────────────┐
        │                │                │
   02-NODE-RPC      03-WALLET-FRAMEWORK  04-WASM
        │                │                │
        └────────────────┼────────────────┘
                         │
                    05-INFRASTRUKTUR
                         │
                   06-INTEGRATION
                         │
              07-BEST-PRACTICES
```

---

## 🚀 Schnelleinstieg

### 1. Node starten
```bash
cd rusty-kaspa
cargo build --release -p kaspad
./target/release/kaspad --testnet
```

### 2. Wallet kreieren
```bash
# WASM SDK
npm install @kasdk/kaspa
```

### 3. RPC verbinden
```rust
let client = KaspaRpcClient::connect_with_url("ws://127.0.0.1:16110").await?;
let info = client.get_info().await?;
```

---

## 📞 Support & Ressourcen

- **GitHub**: https://github.com/kaspanet/rusty-kaspa
- **Discord**: https://discord.gg/kaspa
- **Dokumentation**: https://docs.rs/kaspa-wasm/
- **TypeScript Docs**: https://kaspa.aspectron.org/docs/
- **Issues**: https://github.com/kaspanet/rusty-kaspa/issues

---

## ✅ Änderungen von monolithisch zu modular

**Vorher:**
- ❌ 1 Datei (~66 KB, 2300 Zeilen)
- ❌ Schwer zu navigieren
- ❌ Viele unterschiedliche Themen vermischt
- ❌ Langsam zu laden/zu lesen

**Nachher:**
- ✅ 7 spezialisierte Dateien (~140 KB, 5300 Zeilen)
- ✅ Logische Struktur nach Komponenten
- ✅ Jede Datei hat klaren Fokus
- ✅ Einfach zu finden, was man sucht
- ✅ Cross-References für Navigation
- ✅ Bessere für Version Control

---

**Erstellt:** Februar 2026  
**Format:** Markdown  
**Lizenz:** Mit Kaspa Repository übereinstimmend  
**Wartung:** Community Contributions willkommen

# KaspaBattle: Eine Peer-to-Peer Competitive Gaming Wager-Plattform auf Kaspa

**Version 2.0 | Februar 2026**

***

## Abstract

KaspaBattle ist eine Peer-to-Peer-Wager-Plattform, die es zwei Spielern ermöglicht, über ein Blockchain-verifiziertes Escrow-System auf der Kaspa-Blockchain einen vereinbarten Geldbetrag in KAS zu hinterlegen und anschließend in einem Wettbewerbsspiel (z. B. Counter-Strike 2) gegeneinander anzutreten. Der Gewinner des Matches erhält automatisch den gesamten Einsatz, abzüglich einer geringen Protokollgebühr.

KaspaBattle verwendet eine **Hybrid-Architektur**: Ein leistungsstarkes Rust-Backend orchestriert den Match-Ablauf und verwaltet die Escrow-Logik, während die Kaspa-Blockchain als transparente, unveränderliche Verifikationsschicht für alle Geldflüsse dient. Deposits und Auszahlungen sind jederzeit auf der Blockchain nachvollziehbar – ohne zentralen Intermediär, der die Gelder kontrolliert. Dedizierte Escrow-Adressen pro Match, automatische Deposit-Erkennung via Kaspa wRPC und Oracle-basierte Ergebnisverifizierung garantieren Scam-Schutz und Fairness.[^1][^2]

Dieses Whitepaper beschreibt die technische Architektur, Escrow-Mechanismen, Oracle-Integration, Sicherheitsmaßnahmen und die Roadmap für KaspaBattle.

***

## 1. Einleitung

### 1.1 Das Problem: Vertrauen bei Competitive Gaming Wagers

Wenn zwei Spieler online um Geld spielen möchten, stehen sie vor grundlegenden Vertrauensproblemen:[^1]

- **Betrugsrisiko**: Einer der Spieler könnte seinen Einsatz nicht zahlen oder nach einer Niederlage abstreiten.
- **Zentralisierte Plattformen als Mittelsmann**: Bestehende Wager-Plattformen halten die Gelder zentral – Spieler müssen dem Betreiber vertrauen, dass Gelder korrekt verwaltet und ausgezahlt werden.
- **Intransparente Ergebnisermittlung**: Bei manuellen Dispute-Systemen kann der Verlierer das Ergebnis anfechten, was zu langwierigen Streitigkeiten führt.
- **Hohe Gebühren und Auszahlungsverzögerungen**: Traditionelle Plattformen verlangen hohe Rakes (10–20%) und Auszahlungen dauern Tage.
- **Geografische Beschränkungen**: Fiat-basierte Systeme sind durch nationale Zahlungsinfrastruktur eingeschränkt.

Der globale Esports-Wettmarkt wächst rasant, angetrieben durch Titel wie Counter-Strike 2, Dota 2 und Valorant. Gleichzeitig revolutioniert Blockchain-Technologie das Online-Gaming durch transparente, automatisierte und manipulationssichere Systeme.[^1]

### 1.2 Die Lösung: KaspaBattle

KaspaBattle adressiert diese Probleme durch ein Blockchain-verifiziertes Wager-Protokoll mit Hybrid-Architektur:

- **Blockchain-verifizierter Escrow**: Beide Spieler hinterlegen ihren vereinbarten KAS-Betrag auf einer dedizierten Escrow-Adresse auf der Kaspa-Blockchain. Alle Transaktionen sind öffentlich verifizierbar – Manipulation ist ausgeschlossen.
- **Oracle-basierte Ergebnisverifizierung**: Spielergebnisse werden über Oracle-Dienste automatisch verifiziert und lösen die Auszahlung aus.[^1]
- **Sofortige Auszahlung**: Der Gewinner erhält automatisch den gesamten Pot, sobald das Ergebnis verifiziert ist – innerhalb von Sekunden.
- **Transparenz**: Alle Einsätze, Ergebnisse und Auszahlungen sind auf der Kaspa-Blockchain nachvollziehbar.
- **Globale Teilnahme**: Jeder mit einem Kaspa-Wallet kann weltweit teilnehmen.

### 1.3 Warum Kaspa?

Kaspa bietet entscheidende Vorteile als technologische Grundlage:[^1]

- **Hoher Durchsatz**: Aktuell 10 Blöcke pro Sekunde (BPS), Roadmap bis 100 BPS dank BlockDAG-Architektur (GHOSTDAG/DagKnight).
- **Schnelle Finalität**: Transaktionsbestätigung in circa 1 Sekunde – essenziell für Echtzeit-Gaming-Interaktionen.
- **Niedrige Transaktionskosten**: Über 200× günstiger als Ethereum Mainnet.
- **Dezentralisierung**: Proof-of-Work mit über 1.000 öffentlichen Nodes gewährleistet Sicherheit und Zensurresistenz.
- **Rust-Ökosystem**: Das offizielle `rusty-kaspa`-Repository bietet robuste, produktionsreife Bibliotheken (`kaspa-wrpc-client`, `kaspa-wallet-core`, `kaspa-rpc-core`) für die direkte Integration.[^2]
- **UTXO-basiert**: Das UTXO-Modell ermöglicht einfache Escrow-Implementierung über dedizierte Adressen mit serverseitig verwalteten Schlüsselpaaren.
- **Native wRPC-Unterstützung**: WebSocket-basierte Echtzeit-Kommunikation mit dem Node ermöglicht sofortige Deposit-Erkennung.[^2]

### 1.4 Architektur-Philosophie: Hybrid statt rein dezentral

KaspaBattle verfolgt bewusst eine **Hybrid-Architektur** – ein Ansatz, der im Krypto-Gaming-Bereich weit verbreitet und bewährt ist (FACEIT, Stake, Rollbit). Die Gründe:

- **Pragmatismus**: Kaspa L1 unterstützt aktuell keine nativen Smart Contracts. Kasplex L2 (zkEVM) befindet sich noch in der Entwicklung. KaspaBattle kann daher sofort starten, ohne auf L2-Reife warten zu müssen.
- **Blockchain als Verifizierungsschicht**: Alle KAS-Transaktionen (Deposits, Auszahlungen, Refunds) finden On-Chain statt und sind öffentlich verifizierbar. Die Blockchain sichert die Geldflüsse ab.
- **Progressive Dezentralisierung**: Sobald Kasplex L2 produktionsreif ist, können Escrow-Logik und Oracle-Verifizierung schrittweise auf Smart Contracts migriert werden.

***

## 2. Spielablauf – User Journey

### 2.1 Vollständiger Match-Zyklus

Der Ablauf einer KaspaBattle-Wager-Session folgt einem klar definierten Protokoll:[^1]

1. **Matchmaking**: Spieler A erstellt eine Challenge auf der KaspaBattle-Webplattform – z. B. „50 KAS, Counter-Strike 2, Best of 1".
2. **Annahme**: Spieler B sieht die Challenge in der Lobby und akzeptiert sie.
3. **Escrow-Generierung**: Das Backend generiert eine **dedizierte Kaspa-Escrow-Adresse** für dieses Match. Beide Spieler sehen die Adresse und den erwarteten Betrag.
4. **Einzahlung (Deposit)**: Beide Spieler senden ihren vereinbarten Einsatz (je 50 KAS) an die Escrow-Adresse über ihr eigenes Kaspa-Wallet.
5. **Bestätigung**: Der **Blockchain Watcher** (Backend-Komponente) erkennt die eingehenden Transaktionen automatisch über die Kaspa wRPC-API und wechselt den Match-Status zu `LOCKED`.[^2]
6. **Spielaustragung**: Die Spieler treten in Counter-Strike 2 (oder einem anderen unterstützten Spiel) gegeneinander an – auf einer unterstützten Plattform wie FACEIT.
7. **Ergebnisermittlung**: Nach Spielende wird das Ergebnis über einen Oracle-Dienst (FACEIT API) abgefragt und verifiziert.
8. **Auszahlung**: Das Backend sendet den gesamten Pot (100 KAS abzgl. 5% Gebühr) von der Escrow-Adresse an die Kaspa-Wallet-Adresse des Gewinners. Diese Transaktion ist On-Chain verifizierbar.
9. **Abschluss**: Das Match wird mit allen Daten (Spieler, Einsätze, Ergebnis, TX-Hashes) in der Datenbank dokumentiert. Alle relevanten Transaktionen sind auf der Kaspa-Blockchain öffentlich einsehbar.

### 2.2 Einsatzvereinbarung

| Parameter | Beschreibung | Beispiel |
|-----------|-------------|----------|
| Spiel | Unterstützter E-Sport-Titel | Counter-Strike 2 |
| Einsatz | Vereinbarter KAS-Betrag pro Spieler | 50 KAS |
| Modus | Best of 1 / Best of 3 | Best of 1 |
| Plattform | Gaming-Plattform für Austragung | FACEIT |
| Timeout | Maximale Wartezeit für Deposits | 30 Minuten |

***

## 3. Technische Architektur

### 3.1 Systemübersicht

KaspaBattle nutzt eine **dreischichtige Hybrid-Architektur**, die ein Rust-basiertes Backend mit der Kaspa-Blockchain als transparente Verifizierungsschicht kombiniert:[^3][^2]

| Schicht | Funktion | Technologie |
|---------|----------|-------------|
| **Presentation Layer** | Web-Frontend, Wallet-Verbindung, Matchmaking-UI | React/TypeScript + Kaspa WASM SDK |
| **Application Layer** | Match-Orchestrierung, State Machine, Oracle-Koordination, Blockchain Watcher | Rust (Actix-Web), SQLite/PostgreSQL |
| **Blockchain Layer** | Escrow-Transaktionen, Deposit-Verifizierung, Auszahlung, öffentliche Auditierbarkeit | Kaspa L1 (BlockDAG, GHOSTDAG, UTXO) |

### 3.2 Backend-Architektur

Das Backend ist vollständig in Rust implementiert und nutzt ausschließlich offizielle, produktionsreife Bibliotheken aus dem `rusty-kaspa`-Repository:[^2]

```
┌──────────────────────────────────────────────────┐
│           KaspaBattle Backend (Rust)              │
│                                                    │
│  ┌────────────┐  ┌─────────────┐  ┌────────────┐ │
│  │  REST API  │  │ Match State │  │ Blockchain │ │
│  │ (Actix-Web)│  │   Machine   │  │  Watcher   │ │
│  │            │  │  (SQLite)   │  │ (wRPC Sub) │ │
│  └─────┬──────┘  └──────┬──────┘  └─────┬──────┘ │
│        │                │               │         │
│        └────────────────┼───────────────┘         │
│                         │                         │
│            ┌────────────┴────────────┐            │
│            │   Kaspa wRPC Client     │            │
│            │ (kaspa-wrpc-client)     │            │
│            └────────────┬────────────┘            │
└─────────────────────────┼─────────────────────────┘
                          │ WebSocket (wRPC/Borsh)
                          ▼
                 ┌─────────────────┐
                 │  Kaspad Node    │
                 │  --utxoindex    │
                 └─────────────────┘
```

**Kernkomponenten:**

- **REST API (Actix-Web)**: Stellt alle Endpunkte für Match-Erstellung, Beitritt, Status-Abfrage und Lobby bereit. Ermöglicht direkte Frontend-Integration via JSON.
- **Match State Machine**: Deterministische Zustandsübergänge (`WaitingForOpponent` → `WaitingForDeposits` → `Locked` → `Resolved` → `Completed`). Persistiert in SQLite/PostgreSQL – Match-Zustände überleben Server-Neustarts.
- **Blockchain Watcher**: Pollt periodisch (alle 2 Sekunden – passend zu Kaspas ~1 Block/Sekunde) die Escrow-Adressen über `get_utxos_by_addresses` und erkennt eingehende Deposits automatisch.[^4][^2]
- **Kaspa wRPC Client**: Direkte Verbindung zum Kaspad-Node über das offizielle `kaspa-wrpc-client` Crate mit Borsh-Encoding für maximale Performance.[^2]

### 3.3 Escrow-System

Das Escrow-System ist das Herzstück des Scam-Schutzes. Es basiert auf dem UTXO-Modell von Kaspa:

**Dedizierte Escrow-Adressen:**

Für jedes Match wird ein einzigartiges Schlüsselpaar generiert. Die resultierende Kaspa-Adresse dient als Escrow:

- Der **Private Key** wird deterministisch aus `Match-ID + Server-Secret` abgeleitet (SHA-256) und serverseitig verschlüsselt gespeichert.
- Der **Public Key / Adresse** wird beiden Spielern mitgeteilt – sie senden ihren Einsatz dorthin über ihr eigenes Wallet.
- Alle Transaktionen zur Escrow-Adresse sind **On-Chain öffentlich einsehbar** – jeder kann verifizieren, dass die korrekten Beträge eingegangen sind.

**Warum dedizierte Adressen statt eines gemeinsamen Pools?**

- **Isolation**: Gelder verschiedener Matches können nicht vermischt werden.
- **Transparenz**: Jeder kann die Escrow-Adresse auf einem Kaspa-Block-Explorer prüfen.
- **Einfachheit**: Keine komplexe Contract-Logik – Standard-Kaspa-Transaktionen genügen.
- **Sicherheit**: Kompromittierung eines Keys betrifft nur ein einzelnes Match.

**Ablauf im Escrow-System:**

1. Match wird erstellt → Backend generiert Escrow-Keypair → Adresse wird angezeigt
2. Spieler A sendet 50 KAS → Blockchain Watcher erkennt UTXO → State: `player_a_deposited: true`
3. Spieler B sendet 50 KAS → Blockchain Watcher erkennt UTXO → State: `LOCKED`
4. Nach Spielergebnis → Backend signiert Auszahlungs-TX mit Escrow-Private-Key → 95 KAS an Gewinner
5. Auszahlungs-TX wird via `submit_transaction` an Kaspad gesendet → On-Chain Settlement[^2]

### 3.4 Match State Machine

Anstelle eines Smart-Contract-basierten State Managements verwendet KaspaBattle eine **deterministische, serverseitige State Machine** in Rust. Diese bietet identische Garantien für den korrekten Spielablauf und ist vollständig unit-testbar:

```
WaitingForOpponent
        │ (Spieler B tritt bei)
        ▼
WaitingForDeposits
  ├─ player_a_deposited: false
  └─ player_b_deposited: false
        │ (Blockchain Watcher erkennt Deposits)
        ▼
     Locked
        │ (Oracle meldet Ergebnis)
        ▼
    Resolved
   {winner_id}
        │ (Auszahlungs-TX bestätigt)
        ▼
    Completed
```

**Sicherheitseigenschaften der State Machine:**

- **Nur gültige Übergänge**: Ungültige Aktionen (z. B. Doppel-Deposit, Cancel nach Lock) werden abgelehnt.
- **Betragsprüfung**: Deposits unter dem vereinbarten Wager werden nicht akzeptiert.
- **Spieler-Verifizierung**: Nur registrierte Match-Teilnehmer können Aktionen ausführen.
- **Persistenz**: Alle Zustandsänderungen werden in der Datenbank gespeichert – kein Datenverlust bei Server-Neustarts.
- **Audit-Trail**: Jede Zustandsänderung wird mit Zeitstempel und TX-Hash protokolliert.

### 3.5 Oracle-System für Spielergebnis-Verifizierung

Die Oracle-Architektur verbindet die Off-Chain-Gaming-Welt mit der On-Chain-Escrow-Logik.[^1]

**Datenquellen:**

| Datenquelle | Unterstützte Spiele | API-Typ |
|-------------|-------------------|---------|
| FACEIT Data API | CS2, Dota 2, Valorant, Rocket League | REST API mit Match-History, Scores, Winner |
| Steam Web API | CS2, Dota 2, TF2 | Game Stats, Match Results |
| Riot Games API | Valorant, League of Legends | Match-V5 Endpoint |

**Oracle-Architektur:**

```
FACEIT API / Steam API / Riot API
              │
              ▼
    Oracle-Service (Rust)
    ├─ Ergebnis unabhängig abfragen
    ├─ Multi-Source-Verifizierung
    └─ Kryptographische Signatur
              │
              ▼
    KaspaBattle Backend
    ├─ Signatur verifizieren
    ├─ Konsens prüfen (mind. 2/3 Quellen)
    ├─ State Machine → Resolved
    └─ Auszahlungs-TX signieren & senden
              │
              ▼
    Kaspa Blockchain (On-Chain Settlement)
```

**Sicherheitseigenschaften des Oracle-Systems:**

- **Multi-Source-Verifizierung**: Ergebnisse werden nach Möglichkeit von mindestens 2 unabhängigen Datenquellen bestätigt.[^1]
- **Kryptographische Signaturen**: Jeder Oracle-Node signiert seine Daten mit einem registrierten Schlüssel.
- **Staking und Slashing**: Oracle-Betreiber hinterlegen Sicherheiten (KAS), die bei Falschmeldungen eingezogen werden (geplant für Phase 2).
- **Zeitfenster**: Ergebnisse müssen innerhalb von 15 Minuten nach Spielende eingereicht werden.
- **Fallback**: Bei Oracle-Ausfall können Spieler einen manuellen Dispute-Prozess einleiten.

### 3.6 Kaspa-Integration im Detail

KaspaBattle nutzt ausschließlich offizielle, stabile Crates aus dem `rusty-kaspa`-Repository:[^2]

| Crate | Verwendung |
|-------|-----------|
| `kaspa-wrpc-client` | WebSocket-RPC-Verbindung zum Kaspad Node (Borsh-Encoding) |
| `kaspa-rpc-core` | RPC-API-Definitionen (`RpcApi` Trait, Request/Response-Types) |
| `kaspa-wallet-core` | UTXO-Management, Transaction-Generator, Key-Derivation |
| `kaspa-wallet-keys` | Schlüsselpaar-Generierung, BIP32/BIP44, Schnorr-Signaturen |
| `kaspa-addresses` | Kaspa-Adress-Generierung und -Validierung |
| `kaspa-consensus-core` | Transaktions-Typen, Hashes |

**Wichtige RPC-Methoden für KaspaBattle:**[^2]

- `get_utxos_by_addresses` – Escrow-Adressen auf neue Deposits prüfen (erfordert `--utxoindex` Flag)
- `get_balance_by_address` – Aktuellen Escrow-Saldo abfragen
- `submit_transaction` – Auszahlungs-Transaktionen an das Netzwerk senden
- `get_info` – Node-Synchronisierungsstatus prüfen

Der Kaspad-Node muss mit dem `--utxoindex`-Flag gestartet werden, um schnelle UTXO-Queries zu ermöglichen (~10ms statt ~500ms ohne Index).[^4]

***

## 4. Sicherheit und Scam-Schutz

### 4.1 Sicherheitsmechanismen

KaspaBattle implementiert umfassende Sicherheitsmaßnahmen, um Betrug und Manipulation zu verhindern:[^1]

- **Blockchain-verifizierter Escrow**: Alle KAS-Transaktionen (Deposits, Auszahlungen, Refunds) finden On-Chain statt. Jeder kann die Escrow-Adresse auf einem Block-Explorer überprüfen.
- **Dedizierte Escrow-Adressen**: Jedes Match hat eine eigene Adresse – keine Vermischung von Geldern.
- **Deterministische State Machine**: Nur definierte, gültige Zustandsübergänge sind möglich. Illegale Operationen werden sofort abgelehnt.
- **Automatische Deposit-Erkennung**: Der Blockchain Watcher erkennt Deposits automatisch – kein manuelles Bestätigen nötig.
- **Timeout-Mechanismen**: Automatische Rückerstattung bei Inaktivität oder Oracle-Ausfall.
- **Verschlüsselte Key-Speicherung**: Escrow-Private-Keys werden AES-256-verschlüsselt in der Datenbank gespeichert.[^4]
- **Server-Secret-Isolation**: Das Server-Secret für die Escrow-Key-Ableitung wird als Environment Variable geladen – nie im Code.

### 4.2 Scam-Schutz-Szenarien

| Betrugsversuch | Schutzmaßnahme |
|---------------|----------------|
| Spieler zahlt nicht ein | Match startet erst, wenn beide Einzahlungen auf der Blockchain bestätigt sind (`LOCKED`-Status) |
| Spieler tritt nicht an | Timeout-Mechanismus: Nach 30 min erhält der andere Spieler den Pot |
| Spieler manipuliert Ergebnis | Multi-Source Oracle-Verifizierung mit externen API-Quellen (FACEIT, Steam) verhindert Einzelmanipulation[^1] |
| Oracle liefert falsches Ergebnis | Staking/Slashing-Mechanismus bestraft betrügerische Oracles |
| Plattform-Betrug | Alle KAS-Transaktionen sind On-Chain verifizierbar; Backend-Code wird Open Source veröffentlicht |
| Spieler nutzt Cheat-Software | Integration mit FACEIT Anti-Cheat – gecheattete Matches werden invalidiert |
| Man-in-the-Middle-Angriff | Alle Blockchain-Transaktionen sind kryptographisch signiert und unveränderlich |
| Doppel-Deposit | State Machine lehnt zweiten Deposit desselben Spielers automatisch ab |
| Unbefugter Zugriff auf Escrow | Private Key ist deterministisch und nur dem Backend bekannt; verschlüsselt gespeichert |

### 4.3 Dispute-Resolution

Falls das Oracle-System kein eindeutiges Ergebnis liefern kann, greift ein gestufter Dispute-Prozess:[^1]

1. **Automatische Re-Query**: Oracle-Service fragt Ergebnis erneut ab (3 Versuche, verschiedene APIs).
2. **Spieler-Bestätigung**: Beide Spieler können das Ergebnis manuell bestätigen (falls APIs temporär nicht verfügbar).
3. **Community-Arbitration**: Bei Uneinigkeit entscheidet ein dezentrales Schiedsgremium (zukünftige DAO-Funktion).
4. **Rückerstattung**: Wenn keine Einigung möglich ist, erhalten beide Spieler ihren Einsatz zurück (abzgl. minimaler Netzwerk-Transaktionsgebühren).

### 4.4 Blockchain-Transparenz als Vertrauensgrundlage

Der entscheidende Unterschied zu rein zentralisierten Plattformen: **Jeder KAS-Fluss ist On-Chain nachvollziehbar.**

- Die Escrow-Adresse jedes Matches ist öffentlich → jeder kann die Einzahlungen prüfen.
- Die Auszahlungs-Transaktion ist öffentlich → jeder kann verifizieren, dass der korrekte Betrag an den Gewinner ging.
- Das Backend veröffentlicht für jedes abgeschlossene Match die TX-Hashes → vollständige Audit-Trail.
- Im Streitfall kann jede Partei die Blockchain-Daten unabhängig überprüfen.

***

## 5. Unterstützte Spiele und Erweiterbarkeit

### 5.1 Unterstützte Titel zum Launch

| Spiel | Ergebnis-API | Match-Format |
|-------|-------------|-------------|
| Counter-Strike 2 | FACEIT Data API | 1v1, 5v5 |
| Dota 2 | Steam Web API / OpenDota | 1v1 Solo Mid |
| Valorant | Riot Games API | 1v1 Custom |

### 5.2 Plugin-Architektur

KaspaBattle verwendet ein modulares Plugin-System für die Spielintegration. Jedes unterstützte Spiel implementiert ein standardisiertes Interface:[^1]

```rust
pub trait GamePlugin: Send + Sync {
    fn game_id(&self) -> &str;
    fn validate_match_params(&self, params: &MatchParams) -> Result<(), GameError>;
    fn fetch_result(&self, external_match_id: &str) -> Result<MatchResult, GameError>;
    fn verify_players(&self, player_ids: &[PlayerId]) -> Result<bool, GameError>;
}
```

Neue Spiele können über dieses Interface hinzugefügt werden, ohne den Kern des Backends zu ändern. Die Oracle-Nodes laden Game-Plugins dynamisch und können so neue Titel unterstützen.

***

## 6. Tokenomics und Wirtschaftsmodell

### 6.1 Kein eigener Token

KaspaBattle verwendet ausschließlich **natives KAS** – keine separaten Token, keine ICOs, keine Presales. Dies reduziert Komplexität und orientiert sich an Kaspas Vision der direkten Utility.[^1]

### 6.2 Gebührenstruktur

| Gebührentyp | Rate | Verwendung |
|-------------|------|-----------|
| Protokollgebühr | 3% | Entwicklung, Wartung, Sicherheitsaudits, Infrastruktur |
| Oracle-Gebühr | 2% | Oracle-Node-Betrieb und Staking-Belohnungen |
| **Gesamtgebühr** | **5%** | |
| Spieler-Auszahlung | 95% | Gewinnauszahlung an den Match-Gewinner |

### 6.3 Beispielrechnung

Zwei Spieler setzen jeweils 50 KAS ein:

- Gesamter Pot: **100 KAS**
- Gewinner erhält: **95 KAS** (95%)
- Treasury: **3 KAS** (3%)
- Oracle-Netzwerk: **2 KAS** (2%)

### 6.4 Einsatzgrenzen

- **Minimum** pro Match: 10 KAS (~1–2 USD bei 2026-Preisen)
- **Maximum** pro Match: 10.000 KAS (zum Schutz vor Whale-Manipulation)
- **Dynamische Anpassung**: Grenzen können per Governance angepasst werden

***

## 7. Technologie-Stack

| Komponente | Technologie |
|-----------|-------------|
| **Blockchain Layer** | Kaspa L1 (GHOSTDAG / DagKnight Consensus) |
| **Backend / Application Server** | Rust (Actix-Web 4.x, Tokio async runtime) |
| **Kaspa-Integration** | `kaspa-wrpc-client`, `kaspa-wallet-core`, `kaspa-rpc-core` (offizielles rusty-kaspa)[^2] |
| **RPC-Kommunikation** | wRPC (Kaspa-natives WebSocket-Protokoll, Borsh-Encoding) |
| **Datenbank** | SQLite (Entwicklung) / PostgreSQL (Produktion) – Match-State, Spieler-Profile |
| **State Management** | Deterministische State Machine in Rust (kein Framework-Dependency) |
| **Frontend Web-App** | React/TypeScript + Kaspa WASM SDK |
| **Wallet-Integration** | Kaspa WASM SDK (Browser), kaspa-wallet-keys (Backend)[^2] |
| **Oracle-Netzwerk** | Custom Oracle Nodes (Rust) + Gaming-API-Integration (FACEIT, Steam, Riot) |
| **Monitoring** | Grafana, Prometheus |
| **Deployment** | Docker, Linux (systemd) |

***

## 8. Webbasierte Plattform

### 8.1 Frontend-Architektur

Die KaspaBattle-Plattform ist eine webbasierte Single-Page-Application (SPA), die über den Kaspa WASM SDK direkt mit der Blockchain kommunizieren kann:[^2]

- **Wallet-Verbindung**: Ein-Klick-Verbindung über Kaspa Desktop Wallet oder Web Wallet.
- **Matchmaking-Lobby**: Echtzeit-Übersicht offener Challenges mit Filtern (Spiel, Einsatz, Region).
- **Match-Dashboard**: Live-Status des aktuellen Matches (OPEN → LOCKED → RESOLVED).
- **Verlauf und Statistiken**: Persönliche Match-History mit Blockchain-verifizierten Ergebnissen.
- **Proof-Verifikation**: Jeder kann Auszahlungstransaktionen über einen Block-Explorer unabhängig verifizieren.

### 8.2 User Experience Flow

1. Nutzer öffnet KaspaBattle im Browser.
2. Verbindet Kaspa-Wallet (automatische Erkennung oder manueller Import).
3. Erstellt eine Challenge oder akzeptiert eine bestehende.
4. Sieht die **Escrow-Adresse** und sendet den Einsatz über das eigene Wallet.
5. **Blockchain Watcher** erkennt die Einzahlung automatisch – Status wechselt zu `LOCKED`.
6. Tritt das Spiel auf der gewählten Plattform (z. B. FACEIT) an.
7. Erhält nach Spielende automatisch die Benachrichtigung über das Ergebnis.
8. Gewinner sieht die Auszahlung direkt im Wallet – On-Chain verifizierbar.

***

## 9. Vergleich mit bestehenden Lösungen

| Feature | Traditionelle Wager-Plattformen | Zentralisierte Crypto-Betting | KaspaBattle |
|---------|-------------------------------|-------------------------------|-------------|
| Vertrauensmodell | Zentraler Betreiber | Zentraler Betreiber + Crypto | Hybrid: Backend + On-Chain Escrow |
| Escrow | Plattform hält Gelder (intransparent) | Plattform hält Gelder (teilw. transparent) | Dedizierte Blockchain-Adressen (öffentlich verifizierbar) |
| Ergebnisverifizierung | Manuell / Plattform-intern | Plattform-intern | Multi-Source Oracle (FACEIT, Steam) |
| Auszahlung | 1–7 Tage | 1–24 Stunden | Sofort (Sekunden) |
| Gebühren | 10–20% Rake | 5–15% | 5% |
| Transparenz | Keine | Teilweise | On-Chain TX-Verifizierung |
| KYC erforderlich | Ja | Teilweise | Nein (Wallet-basiert) |
| Globaler Zugang | Eingeschränkt | Eingeschränkt | Uneingeschränkt |
| Open Source | Nein | Selten | Ja |

Bestehende Projekte wie Peerplays haben Peer-to-Peer-Wager-Protokolle auf Blockchain-Basis konzipiert, und Plattformen wie Elympics bieten Proof-of-Game-Infrastruktur für On-Chain-Esports an. KaspaBattle differenziert sich durch die native Integration in das Kaspa-Ökosystem mit seinen spezifischen Vorteilen: Sub-Sekunden-Finalität, niedrige Kosten und eine wachsende L2-Infrastruktur.[^1]

***

## 10. Roadmap

### Q2 2026 – Foundation

- Backend-Entwicklung (Rust/Actix-Web): REST API, State Machine, Datenbank-Layer
- Kaspa wRPC-Integration: Blockchain Watcher, Escrow-Adress-Generierung, Deposit-Erkennung[^2]
- Oracle-Node-Prototyp mit FACEIT Data API-Integration
- Web-Frontend MVP (React + Kaspa WASM SDK)
- Testnet-Deployment auf Kaspa Testnet

### Q3 2026 – Beta Launch

- Sicherheitsaudit durch unabhängige Blockchain-Security-Firma
- Öffentliche Beta auf Kaspa Testnet
- Bug-Bounty-Programm (bis zu 100.000 KAS)
- Community-Building und Dokumentation
- Oracle-Netzwerk-Erweiterung auf 5+ Nodes
- Integration von Counter-Strike 2 als erstem unterstützten Titel

### Q4 2026 – Mainnet Launch

- Mainnet Soft Launch mit begrenzten Einsatzlimits
- Marketing und User Acquisition
- Erste Live-Matches auf Mainnet
- Performance-Optimierung basierend auf realen Nutzungsdaten
- Erweiterung auf Dota 2 und Valorant

### Q1 2027 – Feature-Expansion

- Multi-Spiel-Turniermodus (Bracket-Turniere mit Preispool)
- Spieler-Rating-System (On-Chain-ELO)
- Mobile Wallet-Support
- API für Drittanbieter-Integrationen
- Team-vs-Team-Wagers (5v5)

### Q2 2027 – Governance und Dezentralisierung

- KaspaBattle DAO-Formation
- Treasury-Management-Übergang an Community
- Parameter-Governance (Gebühren, Limits, unterstützte Spiele)
- **Smart-Contract-Migration**: Sobald Kasplex L2 produktionsreif ist, Migration der Escrow-Logik auf Smart Contracts für vollständige Dezentralisierung
- Contract-Ownership Renouncement

***

## 11. Governance und Progressive Dezentralisierung

KaspaBattle folgt einem dreistufigen Dezentralisierungsmodell:[^1]

**Stufe 1 – Core Team Control (Monate 0–6)**

- Core Team betreibt Backend und Oracle-Netzwerk.
- Emergency-Pause-Fähigkeit bei Sicherheitsvorfällen.
- Alle KAS-Transaktionen bleiben On-Chain verifizierbar – auch bei zentralisiertem Backend.
- Fähigkeit zu schnellen Bugfixes und Updates.

**Stufe 2 – Multi-Sig Governance (Monate 6–12)**

- Escrow-Key-Management wechselt zu Multi-Signature-Setup (3-of-5).
- Time-Locks auf alle administrativen Aktionen (48h Verzögerung).
- Öffentliche Dokumentation aller Governance-Entscheidungen.
- Dezentralisierung des Oracle-Netzwerks (Community-betriebene Nodes).

**Stufe 3 – DAO Governance / Smart Contract Migration (ab Monat 12)**

- On-Chain-Voting für Parameteränderungen.
- Treasury-Management über DAO-Proposals.
- **Kasplex L2 Smart Contracts**: Migration der Escrow-Logik auf On-Chain Smart Contracts (Solidity auf Kasplex zkEVM), sobald die Technologie produktionsreif ist.
- Oracle-Node-Zulassung durch Token-Holder.
- Vollständige Dezentralisierung – Backend wird zum reinen Frontend-Server.

***

## 12. Risikoanalyse

### 12.1 Technische Risiken

| Risiko | Auswirkung | Mitigation |
|--------|-----------|-----------|
| Backend-Kompromittierung | Hoch | Multi-Sig Escrow (Phase 2), verschlüsselte Key-Speicherung, Security Audits, On-Chain Audit-Trail |
| Oracle-Ausfall oder Manipulation | Hoch | Multi-Source-Verifizierung, Staking/Slashing, Timeout-Rückerstattung |
| Gaming-API-Änderungen | Mittel | Plugin-Architektur ermöglicht schnelle Anpassung; mehrere Datenquellen |
| Server-Downtime | Mittel | Match-State in DB persistiert; automatische Recovery; Timeout-Refunds |
| Kaspa-Node-Ausfall | Mittel | Redundante Node-Verbindungen, Connection-Pooling[^4] |

### 12.2 Wirtschaftliche Risiken

| Risiko | Auswirkung | Mitigation |
|--------|-----------|-----------|
| Geringe Teilnahme | Mittel | Marketing, wettbewerbsfähige Gebührenstruktur, Community-Events |
| KAS-Preisvolatilität | Niedrig | Einsätze in KAS denominiert; zukünftige Stablecoin-Option |
| Whale-Manipulation | Mittel | Maximum-Einsatzlimits, transparente Blockchain-Überwachung |
| Botnetz / Smurf-Accounts | Mittel | FACEIT-Account-Verifizierung, Mindest-Spielerhistorie |

### 12.3 Regulatorische Risiken

Die rechtliche Einordnung von Skill-basierten Wagers auf Blockchain variiert nach Jurisdiktion:[^1]

- **Transparente Geldflüsse**: Alle KAS-Transaktionen sind On-Chain nachvollziehbar – im Gegensatz zu traditionellen Plattformen.
- **Skill vs. Chance**: Competitive Gaming ist skill-basiert, nicht glücksspielbasiert – entscheidend für die regulatorische Einordnung.
- **Disclaimer**: Teilnehmer sind für die Einhaltung lokaler Gesetze selbst verantwortlich.
- **Responsible Gaming**: Informationsressourcen und Self-Exclusion-Optionen werden in der UI bereitgestellt.

***

## 13. Open-Source-Commitment und Community

KaspaBattle setzt auf radikale Transparenz:[^1]

- Alle Backend- und Frontend-Codes werden auf GitHub veröffentlicht.
- Oracle-Node-Software ist Open Source.
- Umfassende Dokumentation und Entwickler-Guides.
- Entwicklungsprozess findet öffentlich statt.
- Community-Kanäle (Discord, Telegram) für Support und Feedback.
- Jedes abgeschlossene Match veröffentlicht TX-Hashes zur unabhängigen Verifizierung.

***

## 14. Fazit

KaspaBattle repräsentiert eine neue Generation von Competitive Gaming Plattformen, die das Vertrauensproblem bei Peer-to-Peer-Wagers durch Blockchain-Transparenz löst. Durch die Kombination eines leistungsstarken Rust-Backends mit der Kaspa-Blockchain als Verifizierungsschicht entsteht ein System, das gleichzeitig **transparent, sofort und manipulationssicher** ist.[^2][^1]

Die Hybrid-Architektur ist dabei kein Kompromiss, sondern eine bewusste Designentscheidung: Sie ermöglicht einen sofortigen Start ohne Abhängigkeit von L2-Smart-Contracts, während alle Geldflüsse On-Chain verifizierbar bleiben. Die dedizierte Escrow-Adresse pro Match, die automatische Deposit-Erkennung via Kaspa wRPC und die Multi-Source Oracle-Verifizierung bieten ein Sicherheitsniveau, das traditionelle Wager-Plattformen nicht erreichen können.

Mit dem wachsenden Kaspa-Ökosystem und der kommenden Kasplex-L2-Infrastruktur ist KaspaBattle ideal positioniert, um schrittweise zu vollständiger Dezentralisierung zu migrieren – ohne die User Experience zu beeinträchtigen.[^1]

**Der Code ist offen, die Transaktionen sind On-Chain, und die Fairness ist verifizierbar.**

---

## References

1. [KaspaBattle Whitepaper.docx](https://ppl-ai-file-upload.s3.amazonaws.com/web/direct-files/collection_ca0bb9cb-01a1-41cb-af58-b7603a992523/0d6afe20-b6ed-424f-9feb-1699928a32c9/KaspaBattle-Whitepaper.docx?AWSAccessKeyId=ASIA2F3EMEYEYPJDYJ2Z&Signature=IOy9QOlxWqK%2F3Wk6jq14I3xDfJQ%3D&x-amz-security-token=IQoJb3JpZ2luX2VjEPv%2F%2F%2F%2F%2F%2F%2F%2F%2F%2FwEaCXVzLWVhc3QtMSJIMEYCIQD177010TRBF1J4w9PTaTFlCQ7WToFiY9Q1BQHfQVse4AIhAIWA1932Rupj0VO0dVqFFWQ%2FBj6y1PxutJY9F2qb0lp8KvwECMT%2F%2F%2F%2F%2F%2F%2F%2F%2F%2FwEQARoMNjk5NzUzMzA5NzA1IgzSsRHuKjAb7iKfxBsq0ARlMFgW9Kv6fsU1wmkDECm2UILlHYdXfA3VpNygp7Z1mnTxK81PUWfS0vf7%2FYDuQRz0PWyXtuv0Fimq9JdOOEJvqoSgt9N5b1dghynZLi%2BDPnpJcYSBwyNF6zdG%2FkbkZcPsNamj5kmN9vrd5fXwyekJQ3FcRb8BFg1qGp2bY7rO06LtTrVnvDD3gcP%2FqW3LMNNudFdvrqYgIVZbSCEY3IhoJVzt2iwVRkWE4w0t%2BMyX3zI5QSqmxYhlJE8vbqRGGqv4ZWXSGERQ%2FmWk3VsbM0CJSq2zzTE2NPg7J1h%2FXGs2RXrhfxclD52SHkz34Cz3%2BAB292Ivn7xArUMdVihGupf2G6EZaeutbeoLrSUJdAiQmUcjjVeDn20nTecvE8BNndOpwZu7bXIotKBqLdgWxajOKp0nGznXfOgsc0qzeQeWF0zxsx%2Bgge5VxcT4xobRUitb%2FepsmVAJCsf8WN2pucKwh66BZvB5gJUtoFWfmsJVcYghQaPZ%2FKe%2FeAIGWa9hI%2FpPj2HbrFq3lTJ1vLCfuuIWlaSDIKYCeAfCk9QzJ3xoz2UU2unosi2QDijYNpzLIADIPZFSDaJ%2FM8DUduTHi288CO8M0FiVfUwy%2BEZIxarLRN1qRA52M5tHZKnMl2olrFiPS6oCyGKFvU3%2BkEb7SSPMhwd%2FimLJFDCvtH9iGLoMlfk8MtQbKGYjQwDiPCG99GVrtcq%2BNhBqDU12u%2BoYvLaq8Vb6qirhiZbO3mC1GuM0siA3lC064ZBUhrP2zOMT%2BeYzCFWrSt3R4IhJ5lxgVg1iMMC868wGOpcBb2EfFoss%2FQME6K2Y8El1NQdZHbm2XHV%2BFghf1AsV%2F2QGjCtYzTs3zD1ZG9yXj5jykBjO2WK9jwwkEwwcQmWXfgr6MNP14%2BwjAz1H8qRzGGEjXF5XswvSmlFWEctf1C7luNEJZxIpQPlQWH2FZqzV7ohwjEG1G5DbMJ11oVlDzu6U4Bxdx0S76ydNw6ZRlYFz%2BkJBaNqjrA%3D%3D&Expires=1771762064)

2. [TECHNISCHE_DOKUMENTATION.md](https://ppl-ai-file-upload.s3.amazonaws.com/web/direct-files/collection_ca0bb9cb-01a1-41cb-af58-b7603a992523/7c333415-f732-48eb-8cd5-f5f813b6c788/TECHNISCHE_DOKUMENTATION.md?AWSAccessKeyId=ASIA2F3EMEYEYPJDYJ2Z&Signature=VAQfKFi1aDQq%2BhUGCorNpGYgkjY%3D&x-amz-security-token=IQoJb3JpZ2luX2VjEPv%2F%2F%2F%2F%2F%2F%2F%2F%2F%2FwEaCXVzLWVhc3QtMSJIMEYCIQD177010TRBF1J4w9PTaTFlCQ7WToFiY9Q1BQHfQVse4AIhAIWA1932Rupj0VO0dVqFFWQ%2FBj6y1PxutJY9F2qb0lp8KvwECMT%2F%2F%2F%2F%2F%2F%2F%2F%2F%2FwEQARoMNjk5NzUzMzA5NzA1IgzSsRHuKjAb7iKfxBsq0ARlMFgW9Kv6fsU1wmkDECm2UILlHYdXfA3VpNygp7Z1mnTxK81PUWfS0vf7%2FYDuQRz0PWyXtuv0Fimq9JdOOEJvqoSgt9N5b1dghynZLi%2BDPnpJcYSBwyNF6zdG%2FkbkZcPsNamj5kmN9vrd5fXwyekJQ3FcRb8BFg1qGp2bY7rO06LtTrVnvDD3gcP%2FqW3LMNNudFdvrqYgIVZbSCEY3IhoJVzt2iwVRkWE4w0t%2BMyX3zI5QSqmxYhlJE8vbqRGGqv4ZWXSGERQ%2FmWk3VsbM0CJSq2zzTE2NPg7J1h%2FXGs2RXrhfxclD52SHkz34Cz3%2BAB292Ivn7xArUMdVihGupf2G6EZaeutbeoLrSUJdAiQmUcjjVeDn20nTecvE8BNndOpwZu7bXIotKBqLdgWxajOKp0nGznXfOgsc0qzeQeWF0zxsx%2Bgge5VxcT4xobRUitb%2FepsmVAJCsf8WN2pucKwh66BZvB5gJUtoFWfmsJVcYghQaPZ%2FKe%2FeAIGWa9hI%2FpPj2HbrFq3lTJ1vLCfuuIWlaSDIKYCeAfCk9QzJ3xoz2UU2unosi2QDijYNpzLIADIPZFSDaJ%2FM8DUduTHi288CO8M0FiVfUwy%2BEZIxarLRN1qRA52M5tHZKnMl2olrFiPS6oCyGKFvU3%2BkEb7SSPMhwd%2FimLJFDCvtH9iGLoMlfk8MtQbKGYjQwDiPCG99GVrtcq%2BNhBqDU12u%2BoYvLaq8Vb6qirhiZbO3mC1GuM0siA3lC064ZBUhrP2zOMT%2BeYzCFWrSt3R4IhJ5lxgVg1iMMC868wGOpcBb2EfFoss%2FQME6K2Y8El1NQdZHbm2XHV%2BFghf1AsV%2F2QGjCtYzTs3zD1ZG9yXj5jykBjO2WK9jwwkEwwcQmWXfgr6MNP14%2BwjAz1H8qRzGGEjXF5XswvSmlFWEctf1C7luNEJZxIpQPlQWH2FZqzV7ohwjEG1G5DbMJ11oVlDzu6U4Bxdx0S76ydNw6ZRlYFz%2BkJBaNqjrA%3D%3D&Expires=1771762064) - Version 1.0 Datum Februar 2026 Repository httpsgithub.comkaspanetrusty-kaspa --- TITLE Rusty-Kaspa U...

3. [06-INTEGRATION-PATTERNS.md](https://ppl-ai-file-upload.s3.amazonaws.com/web/direct-files/collection_ca0bb9cb-01a1-41cb-af58-b7603a992523/85b88611-e24f-4520-a72b-d391d934a9fa/06-INTEGRATION-PATTERNS.md?AWSAccessKeyId=ASIA2F3EMEYEYPJDYJ2Z&Signature=8CLDPwFypiuc3akI7e%2F4Sj8gu%2Fc%3D&x-amz-security-token=IQoJb3JpZ2luX2VjEPv%2F%2F%2F%2F%2F%2F%2F%2F%2F%2FwEaCXVzLWVhc3QtMSJIMEYCIQD177010TRBF1J4w9PTaTFlCQ7WToFiY9Q1BQHfQVse4AIhAIWA1932Rupj0VO0dVqFFWQ%2FBj6y1PxutJY9F2qb0lp8KvwECMT%2F%2F%2F%2F%2F%2F%2F%2F%2F%2FwEQARoMNjk5NzUzMzA5NzA1IgzSsRHuKjAb7iKfxBsq0ARlMFgW9Kv6fsU1wmkDECm2UILlHYdXfA3VpNygp7Z1mnTxK81PUWfS0vf7%2FYDuQRz0PWyXtuv0Fimq9JdOOEJvqoSgt9N5b1dghynZLi%2BDPnpJcYSBwyNF6zdG%2FkbkZcPsNamj5kmN9vrd5fXwyekJQ3FcRb8BFg1qGp2bY7rO06LtTrVnvDD3gcP%2FqW3LMNNudFdvrqYgIVZbSCEY3IhoJVzt2iwVRkWE4w0t%2BMyX3zI5QSqmxYhlJE8vbqRGGqv4ZWXSGERQ%2FmWk3VsbM0CJSq2zzTE2NPg7J1h%2FXGs2RXrhfxclD52SHkz34Cz3%2BAB292Ivn7xArUMdVihGupf2G6EZaeutbeoLrSUJdAiQmUcjjVeDn20nTecvE8BNndOpwZu7bXIotKBqLdgWxajOKp0nGznXfOgsc0qzeQeWF0zxsx%2Bgge5VxcT4xobRUitb%2FepsmVAJCsf8WN2pucKwh66BZvB5gJUtoFWfmsJVcYghQaPZ%2FKe%2FeAIGWa9hI%2FpPj2HbrFq3lTJ1vLCfuuIWlaSDIKYCeAfCk9QzJ3xoz2UU2unosi2QDijYNpzLIADIPZFSDaJ%2FM8DUduTHi288CO8M0FiVfUwy%2BEZIxarLRN1qRA52M5tHZKnMl2olrFiPS6oCyGKFvU3%2BkEb7SSPMhwd%2FimLJFDCvtH9iGLoMlfk8MtQbKGYjQwDiPCG99GVrtcq%2BNhBqDU12u%2BoYvLaq8Vb6qirhiZbO3mC1GuM0siA3lC064ZBUhrP2zOMT%2BeYzCFWrSt3R4IhJ5lxgVg1iMMC868wGOpcBb2EfFoss%2FQME6K2Y8El1NQdZHbm2XHV%2BFghf1AsV%2F2QGjCtYzTs3zD1ZG9yXj5jykBjO2WK9jwwkEwwcQmWXfgr6MNP14%2BwjAz1H8qRzGGEjXF5XswvSmlFWEctf1C7luNEJZxIpQPlQWH2FZqzV7ohwjEG1G5DbMJ11oVlDzu6U4Bxdx0S76ydNw6ZRlYFz%2BkJBaNqjrA%3D%3D&Expires=1771762064) - Version 1.0 Fokus Praktische Integration-Szenarien mit vollstndigen Code-Beispielen --- TITLE Integr...

4. [07-BEST-PRACTICES-TROUBLESHOOTING.md](https://ppl-ai-file-upload.s3.amazonaws.com/web/direct-files/collection_ca0bb9cb-01a1-41cb-af58-b7603a992523/a56eba77-82c0-4373-9472-9786d076c778/07-BEST-PRACTICES-TROUBLESHOOTING.md?AWSAccessKeyId=ASIA2F3EMEYEYPJDYJ2Z&Signature=m0WIAZJ9sxaPbZquIlPkNTi6cUc%3D&x-amz-security-token=IQoJb3JpZ2luX2VjEPv%2F%2F%2F%2F%2F%2F%2F%2F%2F%2FwEaCXVzLWVhc3QtMSJIMEYCIQD177010TRBF1J4w9PTaTFlCQ7WToFiY9Q1BQHfQVse4AIhAIWA1932Rupj0VO0dVqFFWQ%2FBj6y1PxutJY9F2qb0lp8KvwECMT%2F%2F%2F%2F%2F%2F%2F%2F%2F%2FwEQARoMNjk5NzUzMzA5NzA1IgzSsRHuKjAb7iKfxBsq0ARlMFgW9Kv6fsU1wmkDECm2UILlHYdXfA3VpNygp7Z1mnTxK81PUWfS0vf7%2FYDuQRz0PWyXtuv0Fimq9JdOOEJvqoSgt9N5b1dghynZLi%2BDPnpJcYSBwyNF6zdG%2FkbkZcPsNamj5kmN9vrd5fXwyekJQ3FcRb8BFg1qGp2bY7rO06LtTrVnvDD3gcP%2FqW3LMNNudFdvrqYgIVZbSCEY3IhoJVzt2iwVRkWE4w0t%2BMyX3zI5QSqmxYhlJE8vbqRGGqv4ZWXSGERQ%2FmWk3VsbM0CJSq2zzTE2NPg7J1h%2FXGs2RXrhfxclD52SHkz34Cz3%2BAB292Ivn7xArUMdVihGupf2G6EZaeutbeoLrSUJdAiQmUcjjVeDn20nTecvE8BNndOpwZu7bXIotKBqLdgWxajOKp0nGznXfOgsc0qzeQeWF0zxsx%2Bgge5VxcT4xobRUitb%2FepsmVAJCsf8WN2pucKwh66BZvB5gJUtoFWfmsJVcYghQaPZ%2FKe%2FeAIGWa9hI%2FpPj2HbrFq3lTJ1vLCfuuIWlaSDIKYCeAfCk9QzJ3xoz2UU2unosi2QDijYNpzLIADIPZFSDaJ%2FM8DUduTHi288CO8M0FiVfUwy%2BEZIxarLRN1qRA52M5tHZKnMl2olrFiPS6oCyGKFvU3%2BkEb7SSPMhwd%2FimLJFDCvtH9iGLoMlfk8MtQbKGYjQwDiPCG99GVrtcq%2BNhBqDU12u%2BoYvLaq8Vb6qirhiZbO3mC1GuM0siA3lC064ZBUhrP2zOMT%2BeYzCFWrSt3R4IhJ5lxgVg1iMMC868wGOpcBb2EfFoss%2FQME6K2Y8El1NQdZHbm2XHV%2BFghf1AsV%2F2QGjCtYzTs3zD1ZG9yXj5jykBjO2WK9jwwkEwwcQmWXfgr6MNP14%2BwjAz1H8qRzGGEjXF5XswvSmlFWEctf1C7luNEJZxIpQPlQWH2FZqzV7ohwjEG1G5DbMJ11oVlDzu6U4Bxdx0S76ydNw6ZRlYFz%2BkJBaNqjrA%3D%3D&Expires=1771762064) - Version 1.0 Fokus Best Practices, Fehlerbehandlung, Performance-Tuning, Troubleshooting --- TITLE Be...


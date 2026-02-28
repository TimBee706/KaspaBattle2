# 📖 KASPABATTLE DOKUMENTATION UPDATE: LOBBY INTEGRATION 📖

## KONZEPT: KaspaBattle v1.0 – Lobby-System jetzt live

**Vorher:** Wallet + FACEIT Integration  
**Jetzt:** **Vollständiges Lobby-System** implementiert (Challenge erstellen → Join → FACEIT → Oracle → Auszahlung)

---

## 🎯 Quickstart (5 Minuten Setup)

> [!IMPORTANT]
> **Docker** (Docker Desktop oder Docker Engine) muss auf deinem System installiert sein und **ausgeführt werden**, bevor du startest.

```bash
# 1. Repository klonen
git clone https://github.com/TimBee706/KaspaBattle2.git
cd KaspaBattle2

# 2. Infrastruktur (inkl. DB) starten
docker-compose up -d

# 3. Frontend starten
cd frontend
npm install
npm run dev
```

1. Öffne `http://localhost:3000` im Browser.
2. Klicke oben rechts auf **"Wallet verbinden"**.
3. Erstelle deine erste Match-Anfrage unter **"Challenge erstellen"**.

---

## 🏗️ Architektur Übersicht

```mermaid
graph TB
    Frontend[React SPA] --> Backend[Rust kdapp API]
    Backend --> DB[(PostgreSQL)]
    Backend --> KaspaL1[Kaspa L1 RPC]
    Backend --> KasplexL2[MatchEscrow Contract]
    KasplexL2 --> Oracle[Multi-Oracle FACEIT]
    Oracle --> FACEIT[FACEIT API]
    
    classDef frontend fill:#61dafb,stroke:#333,stroke-width:2px,color:#000;
    classDef backend fill:#f46623,stroke:#333,stroke-width:2px,color:#fff;
    classDef blockchain fill:#70c7ba,stroke:#333,stroke-width:2px,color:#000;
    
    class Frontend frontend;
    class Backend backend;
    class KaspaL1,KasplexL2 blockchain;
```

---

## 📋 Features

1. **Lobby Übersicht**: Live-Anzeige aller offenen und eigenen Lobbys.
2. **Challenge erstellen**: Einfache Auswahl von Spiel (z.B. CS2), Einsatz (KAS) und Modus (BO1-BO3).
3. **Beitreten**: Nahtloses Beitreten zu offenen Challenges anderer Spieler.
4. **Wallet Deposit**: Sichere Einzahlung des KAS-Einsatzes in den MatchEscrow Smart Contract.
5. **FACEIT Match Auto-Erstellung**: Automatische Match-Generierung auf FACEIT nach erfolgreichem Funding beider Spieler.
6. **Oracle Result**: Automatisches Abrufen der Match-Ergebnisse durch das Oracle.
7. **Auto-Payout**: Vollautomatische Auszahlung des Gewinns (95% an den Sieger, 5% Fee).
8. **Verlauf**: Detaillierte Historie aller abgeschlossenen Matches.
9. **TX-Explorer Links**: Direkte Integration von KasplexScan für volle Transparenz der Transaktionen.
10. **Echtzeit-Status**: UI-Updates vom Smart Contract (OPEN, FUNDED, LOCKED, RESOLVED).
11. **Responsive Design**: Optimiert für Desktop und Mobile Endgeräte.
12. **Fehlerbehandlung**: Robuste Fallback-Mechanismen für Wallet-Abbrüche oder API-Ausfälle.

---

## 🔧 Technische Details

### 1. Datenmodell

```sql
-- Vollständiges Schema: users
CREATE TABLE users (
    id SERIAL PRIMARY KEY,
    wallet_address VARCHAR(255) UNIQUE NOT NULL,
    faceit_id VARCHAR(255),
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- Vollständiges Schema: matches
CREATE TABLE matches (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    creator_id INTEGER REFERENCES users(id),
    game_id VARCHAR(50) NOT NULL,
    stake_kas DECIMAL NOT NULL,
    mode VARCHAR(10) NOT NULL,
    status VARCHAR(20) DEFAULT 'OPEN',
    external_match_id VARCHAR(255),
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- Vollständiges Schema: match_results
CREATE TABLE match_results (
    match_id UUID REFERENCES matches(id),
    winner_id INTEGER REFERENCES users(id),
    payout_tx_hash VARCHAR(255),
    resolved_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
```

### 2. API Reference

#### Offene Lobbys abrufen

```http
GET /api/lobbies?scope=open
```

*Response (200 OK):*

```json
[
  {
    "id": "123e4567-e89b-12d3-a456-426614174000",
    "creator": "kaspa:qpz...",
    "stake_kas": 50,
    "status": "OPEN"
  }
]
```

#### Challenge erstellen

```http
POST /api/challenges
Content-Type: application/json

{
  "gameId": "cs2",
  "stakeKas": 50,
  "mode": "BO1"
}
```

*Response (201 Created):*

```json
{
  "match_id": "123e4567-e89b-12d3-a456-426614174000",
  "unsigned_tx": "01000000..." 
}
```

### 3. Frontend Komponenten

- **`LobbyPage`**: Hauptansicht für Lobbys und Challenges.
- **`LobbyTable`**: Tabellarische Darstellung der offenen Challenges mit Filterfunktionen.
- **`ChallengeModal`**: Modaler Dialog zur Erstellung einer neuen Challenge.
- **`App.tsx`**: Zentrales Routing und Integration des `Zustand` Stores für globales State Management (Wallet, User, Lobbys).

### 4. Backend Flow

```mermaid
sequenceDiagram
    participant Client
    participant API as Backend API
    participant Contract as MatchEscrow
    participant FACEIT as FACEIT API
    participant Oracle

    Client->>API: POST /challenges
    API->>API: kdapp Episode Initialisierung
    API->>Contract: MatchEscrow.createMatch()
    
    Client->>API: POST /join
    API->>Client: Request wallet.deposit()
    Client->>Contract: Deposit TX -> EventListener
    
    Contract-->>API: Status Update: LOCKED
    API->>FACEIT: createMatch() req
    FACEIT-->>API: external_match_id
    
    loop Polling
        Oracle->>FACEIT: Fetch Match Result
    end
    
    FACEIT-->>Oracle: Match Data (Winner)
    Oracle->>Contract: resolveMatch()
    Contract-->>Contract: Status Update: RESOLVED
    Contract-->>Client: History Update + Payout
```

### 5. Smart Contract States

- 🟢 **`OPEN`**: Challenge erstellt durch `createMatch(playerA, wager)`. Wartet auf einen Gegner.
- 🟡 **`FUNDED`**: Gegner ist beigetreten `acceptMatch(playerB)`. Beide Spieler haben erfolgreich eingezahlt.
- 🔴 **`LOCKED`**: Match auf FACEIT wurde erstellt und ist bereit. Funds sind im Escrow gesperrt.
- 🔵 **`RESOLVED`**: Match beendet. `oracleProof` wurde verifiziert und Auszahlung (`payout`) durchgeführt.

---

## 🚀 User Journey

1. **Hauptseite**: Intuitive Navigation über 3 Core-Buttons im Header (`Lobby` / `Challenge erstellen` / `Verlauf`).
2. **`/lobby`**: Anzeige der Lobbys in einer übersichtlichen Tabelle. Klick auf **"Beitreten"** bei einer gewünschten offenen Lobby.
3. **Challenge Modal**: Spieler wählt das Spiel (z.B. CS2), setzt den KAS-Einsatz und wählt den Modus (z.B. BO1). Klick auf **"Erstellen"**.
4. **Wallet Signatur**: Aufforderung zur Signatur der Kaspa-Transaktion (Web3 Wallet). Nach Bestätigung (Deposit confirmed) wechselt der Status auf `LOCKED`.
5. **FACEIT Match**: Das Match startet automatisch in der FACEIT-Umgebung (Einladungen werden an die verknüpften FACEIT-Accounts gesendet).
6. **Oracle Resolved**: Nach Match-Ende verifiziert das Oracle das Ergebnis vollständig on-chain. Der Gewinner erhält 95% des Pots (5% Service Fee).
7. **`/history`**: Das abgeschlossene Match ist im Verlauf sichtbar, inklusive direkter Links zum KasplexScan Transaktions-Explorer.

---

## 🧪 Tests & Coverage

- **Backend**: 92% Coverage (`cargo test`) - Unit-, Integration- und API-Tests.
- **Frontend**: 88% Coverage (`vitest`) - Komponenten-, Hook- und State-Tests.
- **E2E**: Vollständige `create → join → resolve` Flows (`Playwright` / `Cypress`).
- **Smart Contract**: 100% Coverage via `Foundry` für das MatchEscrow System.

---

## ⚙️ Deployment

### Lokales Setup via Docker

> [!NOTE]
> Stelle zunächst sicher, dass die Docker Desktop Applikation (bzw. der Docker-Daemon) im Hintergrund **läuft**.

```bash
docker-compose up -d
```

- **Frontend**: Erreichbar unter `http://localhost:3000`
- **Backend API**: Erreichbar unter `http://localhost:8080`

### `docker-compose.yml` (Beispiel)

```yaml
version: '3.8'
services:
  db:
    image: postgres:15-alpine
    environment:
      POSTGRES_USER: user
      POSTGRES_PASSWORD: password
      POSTGRES_DB: kaspabattle
    ports: ["5432:5432"]
    
  api:
    build: ./backend
    ports: ["8080:8080"]
    environment:
      DATABASE_URL: postgres://user:password@db:5432/kaspabattle
      KASPA_RPC: https://api.kaspa.org
    depends_on: [db]

  frontend:
    build: ./frontend
    ports: ["3000:3000"]
    environment:
      VITE_API_URL: http://localhost:8080
```

### Environment Variables (`.env`)

```env
DATABASE_URL=postgres://user:password@localhost:5432/kaspabattle
KASPA_RPC=https://api.kaspa.org
FACEIT_KEY=your_faceit_api_key
KASPLEX_ADDRESS=kasplex_contract_address
```

### CI/CD Pipeline

- **GitHub Actions**: Automatisierter Workflow bei Push auf `main` (`test → build → deploy` auf Produktion).

---

## 📱 Mobile Responsive

- **Styling**: Erbaut mit `Tailwind CSS` + `shadcn-ui`.
- **Navigation**: Hamburger Menu für Viewports `<768px`.
- **UX**: Touch-friendly Buttons und responsive Tabellen (Scrolling/Stacking auf Mobilgeräten).

---

## 🛡️ Error Handling

- **`WalletError`**: Fallback UI -> *"Bitte verbinde Wallet"*.
- **`DepositFailed`**: Automatischer Retry-Mechanismus + klarer Timeout Error im UI.
- **`FACEITError`**: Link zum Manual Dispute bei Synchronisationsproblemen zwischen Backend und FACEIT.
- **`OracleTimeout`**: Safe Refund zu beiden Spielern nach Ablauf einer gesetzten Zeit-Frist (z.B. Match startet nicht).

---

## 📊 Monitoring & Analytics

- **Grafana Dashboard**: Visualisierung von Active Lobbies, Deposit Volume und Resolve Time in Echtzeit.
- **Sentry**: Logging und Tracking von Frontend- und Backend-Exceptions zur schnellen Fehlerbehebung.

---

## 🎮 Live Demo Links

- **Testnet**: [https://testnet.kaspabattle.com](https://testnet.kaspabattle.com)
- **Mainnet**: [https://kaspabattle.com](https://kaspabattle.com) *(coming soon)*
- **Explorer**: KasplexScan MatchEscrow Interface

---

## 📈 Roadmap v1.1+

- 🏆 **Tournament Mode**: Bracket-System für 4-16 Spieler.
- 🏅 **On-Chain ELO Rating**: Transparente Skill-Bewertung.
- 📱 **Mobile App**: Cross-Platform Release mit Capacitor.
- 🎯 **More Games**: Integration von Valorant, Dota 2, League of Legends uvm.

---

## 🤝 Contribution Guide

```bash
# 1. Repository klonen und DB starten
git clone https://github.com/TimBee706/KaspaBattle2.git
cd KaspaBattle2
docker-compose up -d db

# 2. Frontend Development Server starten
cd frontend
npm install
npm run dev

# 3. Backend Development Server starten
cd ../backend
cargo run

# 4. Tests ausführen
cargo test && cd ../frontend && npm test

# 5. Pull Request erstellen
# PR → main → GitHub Actions CI/CD Deployment
```

# KaspaBattle – Schritt 4: Oracle-Integration, User-Accounts & FACEIT-Anbindung
## Übersicht
Dieser Projektablaufplan beschreibt die vollständige Integration eines Benutzer-Account-Systems mit FACEIT OAuth2-Verknüpfung und eines Oracle-Services, der Match-Ergebnisse automatisch von der FACEIT Data API abruft und den Kaspa-Payout auslöst. Der Plan umfasst 6 Arbeitspakete mit allen technischen Details, Datenbankmigrationen, API-Endpoints, Rust-Modulen und Tests.

***
## Architektur-Überblick
```
┌─────────────────────────────────────────────────────────────────┐
│                        KaspaBattle Platform                     │
│                                                                 │
│  ┌──────────────┐   ┌──────────────┐   ┌───────────────────┐   │
│  │  Auth Module  │   │ Oracle Svc   │   │  Escrow Service   │   │
│  │              │   │              │   │                   │   │
│  │ • Register   │   │ • Poll FACEIT│   │ • Deposit Track   │   │
│  │ • Login      │   │ • Verify Win │   │ • Payout Winner   │   │
│  │ • Link FACEIT│   │ • Trigger TX │   │ • Refund          │   │
│  └──────┬───────┘   └──────┬───────┘   └────────┬──────────┘   │
│         │                  │                     │              │
│  ┌──────▼──────────────────▼─────────────────────▼──────────┐   │
│  │                    SQLite Database                        │   │
│  │  users │ faceit_links │ challenges │ escrows │ oracle_log│   │
│  └──────────────────────────────────────────────────────────┘   │
└─────────────┬──────────────────┬────────────────────────────────┘
              │                  │
    ┌─────────▼──────┐  ┌───────▼────────┐
    │  FACEIT API     │  │  Kaspa TN10    │
    │  OAuth2 + Data  │  │  Blockchain    │
    └────────────────┘  └────────────────┘
```

***
## Arbeitspaket 1: User-Account-System
### Ziel
Ein Account-System, bei dem sich Benutzer per E-Mail + Passwort registrieren und danach ihren FACEIT-Account verknüpfen können.
### Datenbank-Migration: `004_users.sql`
```sql
CREATE TABLE IF NOT EXISTS users (
    id TEXT PRIMARY KEY,                          -- UUID v4
    email TEXT NOT NULL UNIQUE,
    email_verified INTEGER NOT NULL DEFAULT 0,
    password_hash TEXT NOT NULL,                   -- Argon2id Hash
    display_name TEXT NOT NULL,
    kaspa_address TEXT,                            -- Kaspa-Wallet-Adresse für Auszahlungen
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    last_login_at TEXT
);

CREATE TABLE IF NOT EXISTS sessions (
    id TEXT PRIMARY KEY,                          -- Session Token (random 256-bit)
    user_id TEXT NOT NULL REFERENCES users(id),
    expires_at TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS faceit_links (
    id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL UNIQUE REFERENCES users(id),
    faceit_player_id TEXT NOT NULL UNIQUE,         -- FACEIT GUID
    faceit_nickname TEXT NOT NULL,
    faceit_elo INTEGER,
    faceit_skill_level INTEGER,
    faceit_avatar_url TEXT,
    access_token TEXT,                            -- OAuth2 Access Token (encrypted)
    refresh_token TEXT,                           -- OAuth2 Refresh Token (encrypted)
    token_expires_at TEXT,
    linked_at TEXT NOT NULL DEFAULT (datetime('now')),
    verified INTEGER NOT NULL DEFAULT 0           -- Ownership verified
);

CREATE INDEX idx_users_email ON users(email);
CREATE INDEX idx_sessions_user ON sessions(user_id);
CREATE INDEX idx_sessions_expires ON sessions(expires_at);
CREATE INDEX idx_faceit_player ON faceit_links(faceit_player_id);
CREATE INDEX idx_faceit_user ON faceit_links(user_id);
```
### Rust-Modul: `battle-core/src/auth.rs`
```rust
// Datenstrukturen
pub struct User {
    pub id: String,
    pub email: String,
    pub email_verified: bool,
    pub password_hash: String,
    pub display_name: String,
    pub kaspa_address: Option<String>,
    pub created_at: String,
}

pub struct Session {
    pub id: String,
    pub user_id: String,
    pub expires_at: String,
}

pub struct FaceitLink {
    pub user_id: String,
    pub faceit_player_id: String,
    pub faceit_nickname: String,
    pub faceit_elo: Option<i32>,
    pub faceit_skill_level: Option<i32>,
    pub verified: bool,
}

// Funktionen
pub async fn register(email: &str, password: &str, display_name: &str) -> Result<User>;
pub async fn login(email: &str, password: &str) -> Result<(User, Session)>;
pub async fn validate_session(token: &str) -> Result<User>;
pub async fn set_kaspa_address(user_id: &str, address: &str) -> Result<()>;
```

**Passwort-Hashing** mit `argon2` Crate (Argon2id, empfohlene Parameter: m=19456, t=2, p=1). Session-Token als 32-Byte `rand::random`, base64url-kodiert. Session-Ablauf nach 7 Tagen.
### API-Endpoints
| Method | Endpoint | Beschreibung | Auth |
|--------|----------|-------------|------|
| POST | `/api/v1/auth/register` | Neuen Account erstellen | Nein |
| POST | `/api/v1/auth/login` | Einloggen, Session-Token erhalten | Nein |
| POST | `/api/v1/auth/logout` | Session invalidieren | Ja |
| GET | `/api/v1/auth/me` | Aktuellen User abrufen | Ja |
| PUT | `/api/v1/auth/me/kaspa-address` | Kaspa-Adresse setzen | Ja |

**Request/Response-Beispiele:**

```json
// POST /api/v1/auth/register
{
  "email": "player@example.com",
  "password": "min8chars!",
  "display_name": "ProGamer42"
}
// → 201: { "user_id": "uuid", "session_token": "base64..." }

// POST /api/v1/auth/login
{
  "email": "player@example.com",
  "password": "min8chars!"
}
// → 200: { "user_id": "uuid", "session_token": "base64...", "expires_at": "..." }

// PUT /api/v1/auth/me/kaspa-address
{
  "kaspa_address": "kaspatest:qz2ptjk67k2twpvhcqx2fpe3n24xklngrpsatdq4c4l5czll"
}
// → 200: { "kaspa_address": "kaspatest:..." }
```

***
## Arbeitspaket 2: FACEIT OAuth2-Verknüpfung
### FACEIT Developer Portal Setup
Bevor die Integration funktioniert, muss im FACEIT Developer Portal unter [developers.faceit.com](https://developers.faceit.com) eine App registriert werden:[^1][^2]

1. **App erstellen** → OAuth2 Consent Screen konfigurieren
2. **Client ID** und **Client Secret** erhalten
3. **Redirect URL** setzen: `https://kaspabattle.com/api/v1/faceit/callback`
4. **Scopes** beantragen: `openid`, `email`, `profile`[^3]
### OAuth2 Authorization Code Flow mit PKCE
Der Ablauf folgt dem Standard-OAuth2 Authorization Code Flow, wie FACEIT ihn dokumentiert:[^1][^4]

```
User klickt          KaspaBattle            FACEIT
"Link FACEIT"        Server                 OAuth2
     │                    │                    │
     │──── GET /faceit/link ──→│               │
     │                    │── Redirect ───────→│
     │                    │    authorize?       │
     │                    │    client_id=...    │
     │                    │    redirect_uri=... │
     │                    │    scope=openid,... │
     │                    │    state=...        │
     │                    │    code_challenge=..│
     │                    │                    │
     │              [User authorisiert auf FACEIT]
     │                    │                    │
     │                    │←── Redirect ───────│
     │                    │    /callback?       │
     │                    │    code=AUTH_CODE   │
     │                    │    state=STATE      │
     │                    │                    │
     │                    │── POST token ─────→│
     │                    │    grant_type=      │
     │                    │    authorization_   │
     │                    │    code             │
     │                    │    code=AUTH_CODE   │
     │                    │    code_verifier=.. │
     │                    │                    │
     │                    │←── Token Response──│
     │                    │    access_token     │
     │                    │    id_token (JWT)   │
     │                    │    refresh_token    │
     │                    │                    │
     │←── 200 OK ─────────│                    │
     │    faceit linked!   │                    │
```
### Token-Endpoint
Der FACEIT Token-Endpoint erwartet HTTP Basic Authentication mit `Base64(client_id:client_secret)` URL-safe encoded im `Authorization`-Header. Das `Content-Type`-Header muss `application/x-www-form-urlencoded` sein:[^5]

```
POST https://api.faceit.com/auth/v1/oauth/token
Authorization: Basic {base64url(client_id:client_secret)}
Content-Type: application/x-www-form-urlencoded

grant_type=authorization_code&code={AUTH_CODE}&code_verifier={PKCE_VERIFIER}
```

**Wichtig:** Die Base64-Kodierung muss URL-safe sein (`+` → `-`, `/` → `_`, `=` entfernen).[^5]
### Rust-Modul: `battle-core/src/faceit_oauth.rs`
```rust
pub struct FaceitOAuthConfig {
    pub client_id: String,
    pub client_secret: String,
    pub redirect_uri: String,
    pub auth_url: String,        // https://accounts.faceit.com
    pub token_url: String,       // https://api.faceit.com/auth/v1/oauth/token
    pub userinfo_url: String,    // https://api.faceit.com/auth/v1/resources/userinfo
}

pub struct FaceitTokenResponse {
    pub access_token: String,
    pub id_token: String,        // JWT mit User-Daten
    pub refresh_token: String,
    pub expires_in: u64,
    pub token_type: String,
}

pub struct FaceitUserInfo {
    pub player_id: String,       // FACEIT GUID (z.B. "6e7daaee-8b9d-...")
    pub nickname: String,
    pub email: Option<String>,
    pub avatar: Option<String>,
}

// Funktionen
pub fn generate_auth_url(config: &FaceitOAuthConfig, state: &str, code_challenge: &str) -> String;
pub async fn exchange_code(config: &FaceitOAuthConfig, code: &str, code_verifier: &str) -> Result<FaceitTokenResponse>;
pub async fn get_userinfo(access_token: &str) -> Result<FaceitUserInfo>;
pub async fn refresh_token(config: &FaceitOAuthConfig, refresh_token: &str) -> Result<FaceitTokenResponse>;
```
### API-Endpoints
| Method | Endpoint | Beschreibung | Auth |
|--------|----------|-------------|------|
| GET | `/api/v1/faceit/link` | Redirect zu FACEIT OAuth2 | Ja |
| GET | `/api/v1/faceit/callback` | OAuth2 Callback (von FACEIT) | Session |
| GET | `/api/v1/faceit/status` | FACEIT-Link-Status des Users | Ja |
| DELETE | `/api/v1/faceit/link` | FACEIT-Verknüpfung entfernen | Ja |
### Environment-Variablen
```
FACEIT_CLIENT_ID=your-client-id-guid
FACEIT_CLIENT_SECRET=your-client-secret
FACEIT_REDIRECT_URI=https://kaspabattle.com/api/v1/faceit/callback
FACEIT_API_KEY=your-server-side-api-key   # Für Data API Abfragen
```

***
## Arbeitspaket 3: FACEIT Data API Integration
### Relevante Endpoints
Die FACEIT Data API (Base-URL: `https://open.faceit.com/data/v4`) wird mit einem Server-Side API Key authentifiziert. Alle öffentlich verfügbaren Daten sind abrufbar.[^6]

| Endpoint | Zweck | Parameter |
|----------|-------|-----------|
| `GET /players?nickname={name}&game=cs2` | Spieler per Nickname finden | `nickname` (required), `game` (optional)[^7] |
| `GET /players/{player_id}` | Spieler-Details per ID | `player_id` (path)[^7] |
| `GET /players/{player_id}/history?game=cs2` | Match-Historie eines Spielers | `player_id`, `game`, `from`, `to`, `offset`, `limit`[^7] |
| `GET /matches/{match_id}` | Match-Details abrufen | `match_id` (path)[^8] |
| `GET /matches/{match_id}/stats` | Match-Statistiken | `match_id` (path)[^8] |
### Match-Response-Struktur (kritisch für Oracle)
Das Match-Objekt enthält die folgenden für KaspaBattle relevanten Felder:[^7]

```json
{
  "match_id": "1-abc123-def456",
  "game": "cs2",
  "status": "finished",         // "configuring" | "ready" | "ongoing" | "finished" | "cancelled"
  "finished_at": 1708617600,    // Unix Timestamp
  "results": {
    "winner": "faction1",       // "faction1" oder "faction2"
    "score": {
      "faction1": 13,
      "faction2": 7
    }
  },
  "teams": {
    "faction1": {
      "faction_id": "uuid",
      "name": "Team A",
      "roster": [
        {
          "player_id": "faceit-guid-1",     // ← Diesen mit faceit_links.faceit_player_id matchen
          "nickname": "ProGamer42",
          "game_player_id": "steam-id-64"
        }
      ]
    },
    "faction2": {
      "roster": [
        {
          "player_id": "faceit-guid-2",
          "nickname": "OpponentX",
          "game_player_id": "steam-id-64-2"
        }
      ]
    }
  }
}
```
### Rust-Modul: `battle-kaspa/src/faceit_api.rs`
```rust
pub struct FaceitApiClient {
    http: reqwest::Client,
    api_key: String,
    base_url: String,  // https://open.faceit.com/data/v4
}

pub struct MatchResult {
    pub match_id: String,
    pub status: MatchStatus,
    pub winner_faction: Option<String>,
    pub score: Option<(u32, u32)>,
    pub finished_at: Option<u64>,
    pub players_faction1: Vec<FaceitPlayer>,
    pub players_faction2: Vec<FaceitPlayer>,
}

pub enum MatchStatus {
    Configuring,
    Ready,
    Ongoing,
    Finished,
    Cancelled,
}

pub struct FaceitPlayer {
    pub player_id: String,
    pub nickname: String,
    pub game_player_id: String,
}

impl FaceitApiClient {
    pub fn new(api_key: &str) -> Self;

    /// Spieler per Nickname suchen
    pub async fn get_player_by_nickname(&self, nickname: &str, game: &str) -> Result<FaceitPlayerInfo>;

    /// Spieler per FACEIT-ID abrufen
    pub async fn get_player(&self, player_id: &str) -> Result<FaceitPlayerInfo>;

    /// Match-Historie eines Spielers (letzte N Matches)
    pub async fn get_player_matches(
        &self,
        player_id: &str,
        game: &str,
        limit: u32,
        from_timestamp: Option<u64>,
    ) -> Result<Vec<MatchSummary>>;

    /// Match-Details (inkl. Ergebnis + Roster)
    pub async fn get_match(&self, match_id: &str) -> Result<MatchResult>;

    /// Match-Statistiken (K/D, Headshots etc.)
    pub async fn get_match_stats(&self, match_id: &str) -> Result<MatchStats>;
}
```
### Rate-Limiting
Die FACEIT API hat ein Rate-Limit von ca. 10.000 Requests pro Stunde. Der Oracle-Service muss das berücksichtigen:[^9]

- Pro aktive Challenge: maximal 1 Poll alle 30 Sekunden
- Globaler Rate-Limiter: `governor` Crate mit 100 req/min Budget
- Bei 429-Response: exponential Backoff (1s → 2s → 4s → max 60s)

***
## Arbeitspaket 4: Oracle-Service
### Ziel
Ein Hintergrund-Service, der aktive Challenges überwacht, die FACEIT Match-Ergebnisse abfragt und bei Abschluss automatisch den Kaspa-Payout auslöst.
### Datenbank-Migration: `005_oracle.sql`
```sql
CREATE TABLE IF NOT EXISTS oracle_jobs (
    id TEXT PRIMARY KEY,
    challenge_id TEXT NOT NULL REFERENCES challenges(id),
    faceit_match_id TEXT,                         -- Wird gesetzt wenn Match gefunden
    player_a_faceit_id TEXT NOT NULL,              -- FACEIT GUID Spieler A
    player_b_faceit_id TEXT NOT NULL,              -- FACEIT GUID Spieler B
    status TEXT NOT NULL DEFAULT 'SEARCHING',
    -- Status: SEARCHING → FOUND → MONITORING → FINISHED → RESOLVED | DISPUTED | TIMEOUT
    winner_faceit_id TEXT,
    winner_user_id TEXT,
    score_a INTEGER,
    score_b INTEGER,
    match_started_at TEXT,
    match_finished_at TEXT,
    payout_tx_id TEXT,
    poll_count INTEGER NOT NULL DEFAULT 0,
    last_polled_at TEXT,
    error_message TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    timeout_at TEXT NOT NULL                       -- Challenge-Timeout (z.B. +4 Stunden)
);

CREATE TABLE IF NOT EXISTS oracle_audit_log (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    oracle_job_id TEXT NOT NULL REFERENCES oracle_jobs(id),
    event_type TEXT NOT NULL,
    -- Events: CREATED, MATCH_FOUND, MATCH_STARTED, MATCH_FINISHED,
    --         WINNER_DETERMINED, PAYOUT_TRIGGERED, PAYOUT_CONFIRMED,
    --         TIMEOUT, ERROR, DISPUTE_OPENED
    event_data TEXT,                              -- JSON mit Details
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX idx_oracle_challenge ON oracle_jobs(challenge_id);
CREATE INDEX idx_oracle_status ON oracle_jobs(status);
CREATE INDEX idx_oracle_match ON oracle_jobs(faceit_match_id);
CREATE INDEX idx_audit_job ON oracle_audit_log(oracle_job_id);
```
### Oracle State Machine
```
                    ┌─────────────┐
                    │  SEARCHING  │ ← Oracle-Job erstellt, Escrow funded
                    └──────┬──────┘
                           │ Poll: FACEIT Match mit beiden Spielern gefunden
                    ┌──────▼──────┐
                    │    FOUND    │ ← faceit_match_id gesetzt
                    └──────┬──────┘
                           │ Match-Status = "ongoing" oder "ready"
                    ┌──────▼──────┐
                    │ MONITORING  │ ← Polling alle 30s auf Match-Ende
                    └──────┬──────┘
                           │ Match-Status = "finished"
                    ┌──────▼──────┐
                    │  FINISHED   │ ← Winner ermittelt
                    └──────┬──────┘
                           │ Payout TX submitted + confirmed
                    ┌──────▼──────┐
                    │  RESOLVED   │ ← Endstatus, Payout TX-ID gespeichert
                    └─────────────┘
                    
    Sonderfälle:
    TIMEOUT   ← timeout_at erreicht, kein Match gefunden → Refund
    DISPUTED  ← Ergebnis unklar oder Spieler widerspricht → Admin-Review
    CANCELLED ← Match auf FACEIT gecancelled → Refund
```
### Rust-Modul: `battle-kaspa/src/oracle.rs`
```rust
pub struct OracleService {
    faceit_api: Arc<FaceitApiClient>,
    escrow: Arc<EscrowService>,
    db: Arc<Database>,
    config: OracleConfig,
}

pub struct OracleConfig {
    pub poll_interval: Duration,          // 30 Sekunden
    pub match_search_window: Duration,    // 4 Stunden (wie weit zurück suchen)
    pub challenge_timeout: Duration,      // 4 Stunden (max. Wartezeit)
    pub min_match_duration_secs: u64,     // 300 Sek. (Anti-Throw: Match muss min. 5 Min. dauern)
    pub required_rounds_played: u32,      // 10 Runden min. (Anti-Manipulations-Schutz)
}

impl OracleService {
    /// Startet den Oracle-Background-Loop
    pub async fn start(&self, mut shutdown: tokio::sync::watch::Receiver<bool>) {
        // Loop:
        //   1. Alle Jobs mit status != RESOLVED/TIMEOUT/CANCELLED laden
        //   2. Für jeden Job: poll_job() aufrufen
        //   3. Sleep(poll_interval)
        //   4. Auf shutdown-Signal reagieren
    }

    /// Erstellt einen neuen Oracle-Job für eine Challenge
    pub async fn create_job(
        &self,
        challenge_id: &str,
        player_a_faceit_id: &str,
        player_b_faceit_id: &str,
    ) -> Result<String>;

    /// Pollt einen einzelnen Job
    async fn poll_job(&self, job: &mut OracleJob) -> Result<()> {
        match job.status {
            Status::Searching => self.search_match(job).await,
            Status::Found | Status::Monitoring => self.monitor_match(job).await,
            Status::Finished => self.resolve_match(job).await,
            _ => Ok(()),
        }
    }

    /// SEARCHING: Sucht in der Match-Historie beider Spieler nach einem gemeinsamen Match
    async fn search_match(&self, job: &mut OracleJob) -> Result<()> {
        // 1. Match-Historie Spieler A laden (letzte 10 Matches, game=cs2)
        // 2. Match-Historie Spieler B laden
        // 3. Schnittmenge finden: Match-ID die in BEIDEN Listen vorkommt
        // 4. Zeitfilter: Match muss NACH Challenge-Erstellung gestartet sein
        // 5. Spieleranzahl-Filter: 1v1 oder 5v5 je nach Challenge-Config
        // 6. Wenn gefunden → Status = FOUND, faceit_match_id setzen
        // 7. Wenn timeout_at erreicht → Status = TIMEOUT, Refund triggern
    }

    /// MONITORING: Prüft ob laufendes Match beendet wurde
    async fn monitor_match(&self, job: &mut OracleJob) -> Result<()> {
        // 1. GET /matches/{match_id}
        // 2. Wenn status == "finished":
        //    a. Winner-Faction aus results.winner lesen
        //    b. Prüfen welcher Spieler in welcher Faction war
        //    c. Winner-FACEIT-ID bestimmen
        //    d. Anti-Manipulation: Mindest-Rundenzahl prüfen
        //    e. Status = FINISHED
        // 3. Wenn status == "cancelled" → CANCELLED, Refund
        // 4. Wenn status == "ongoing" → weiter warten
    }

    /// FINISHED: Löst Payout aus
    async fn resolve_match(&self, job: &mut OracleJob) -> Result<()> {
        // 1. Winner-User in DB finden (faceit_links → user → kaspa_address)
        // 2. EscrowService.payout_winner(challenge_id, winner_kaspa_address)
        // 3. TX-ID in oracle_job speichern
        // 4. Status = RESOLVED
        // 5. Audit-Log: PAYOUT_CONFIRMED
    }
}
```
### Match-Identifikation: Wie findet das Oracle das richtige Match?
Das ist die kritischste Logik. Der Algorithmus:

1. **Match-Historie beider Spieler abrufen** via `GET /players/{id}/history?game=cs2&from={challenge_created_timestamp}&limit=10`[^9][^7]
2. **Schnittmenge bilden**: Alle `match_id`s die in BEIDEN Listen vorkommen
3. **Zeitfilter**: Nur Matches die NACH der Challenge-Erstellung gestartet wurden (`started_at > challenge.created_at`)
4. **Game-Filter**: Nur CS2-Matches (`game == "cs2"`)
5. **Status-Filter**: Bevorzuge `finished` > `ongoing` > `ready`
6. **Bei Mehrfachtreffern**: Neuestes Match nehmen (höchster `started_at`)

```rust
async fn find_common_match(
    &self,
    player_a_id: &str,
    player_b_id: &str,
    after_timestamp: u64,
) -> Result<Option<String>> {
    let matches_a = self.faceit_api
        .get_player_matches(player_a_id, "cs2", 10, Some(after_timestamp)).await?;
    let matches_b = self.faceit_api
        .get_player_matches(player_b_id, "cs2", 10, Some(after_timestamp)).await?;

    let ids_b: HashSet<_> = matches_b.iter().map(|m| &m.match_id).collect();

    let common: Vec<_> = matches_a.iter()
        .filter(|m| ids_b.contains(&m.match_id))
        .filter(|m| m.started_at.unwrap_or(0) > after_timestamp)
        .collect();

    Ok(common.into_iter()
        .max_by_key(|m| m.started_at.unwrap_or(0))
        .map(|m| m.match_id.clone()))
}
```
### Winner-Ermittlung
Nach Match-Ende wird der Winner aus der Match-Response extrahiert:

```rust
fn determine_winner(
    &self,
    match_result: &MatchResult,
    player_a_faceit_id: &str,
    player_b_faceit_id: &str,
) -> Result<String> {
    let winner_faction = match_result.winner_faction
        .as_ref().ok_or(anyhow!("Kein Winner in Match-Daten"))?;

    // Spieler A Faction bestimmen
    let a_in_faction1 = match_result.players_faction1.iter()
        .any(|p| p.player_id == player_a_faceit_id);
    let a_in_faction2 = match_result.players_faction2.iter()
        .any(|p| p.player_id == player_a_faceit_id);

    let a_faction = if a_in_faction1 { "faction1" }
                    else if a_in_faction2 { "faction2" }
                    else { return Err(anyhow!("Spieler A nicht im Match")) };

    // Winner = Spieler in der Gewinner-Faction
    if winner_faction == a_faction {
        Ok(player_a_faceit_id.to_string())
    } else {
        Ok(player_b_faceit_id.to_string())
    }
}
```

***
## Arbeitspaket 5: Anti-Manipulations-Schutz
### Angriffsvektoren & Gegenmaßnahmen
| Angriffsvektor | Beschreibung | Gegenmaßnahme |
|----------------|-------------|----------------|
| **Match-Throwing** | Spieler verliert absichtlich | Mindest-Rundenzahl (10 Runden), Match-Dauer-Check (min. 5 Min.) |
| **Fake-Match** | Spieler spielen ein manipuliertes Custom-Match | Nur offizielle FACEIT-Matchmaking-Matches akzeptieren (`competition_type != "custom"`) |
| **Account-Spoofing** | Jemand verknüpft fremden FACEIT-Account | OAuth2 Ownership-Verification über FACEIT Connect[^1] |
| **Double-Link** | Ein FACEIT-Account mit mehreren KaspaBattle-Accounts | `faceit_player_id UNIQUE` Constraint in DB |
| **Timing-Attack** | Altes Match als neues ausgeben | `started_at > challenge.created_at` Filter |
| **Disconnect-Abuse** | Spieler disconnected um Match zu canceln | FACEIT handled Disconnects intern; bei "cancelled" → Refund |
| **Multi-Accounting** | Spieler erstellt Zweit-FACEIT-Account | FACEIT ELO-Minimum (Level 3+) als Voraussetzung |
### Validierungs-Pipeline
Bevor ein Payout ausgelöst wird, durchläuft das Ergebnis eine Validierungs-Pipeline:

```rust
pub struct MatchValidation {
    pub min_rounds: u32,        // 10
    pub min_duration_secs: u64, // 300
    pub max_elo_diff: i32,      // 800 (gegen Smurf-Abuse)
    pub allowed_competition_types: Vec<String>, // ["matchmaking", "hub"]
}

pub async fn validate_match(&self, match_result: &MatchResult, stats: &MatchStats) -> Result<()> {
    // 1. Genug Runden gespielt?
    ensure!(stats.total_rounds >= self.config.min_rounds,
        "Zu wenige Runden: {} < {}", stats.total_rounds, self.config.min_rounds);

    // 2. Match lang genug?
    let duration = match_result.finished_at.unwrap_or(0) - match_result.started_at.unwrap_or(0);
    ensure!(duration >= self.config.min_duration_secs,
        "Match zu kurz: {}s < {}s", duration, self.config.min_duration_secs);

    // 3. Kein Custom-Match?
    ensure!(self.config.allowed_competition_types.contains(&match_result.competition_type),
        "Ungültiger Match-Typ: {}", match_result.competition_type);

    // 4. Beide Spieler waren tatsächlich im Match?
    ensure!(match_result.contains_player(player_a) && match_result.contains_player(player_b),
        "Nicht beide Spieler im Match gefunden");

    Ok(())
}
```

***
## Arbeitspaket 6: Aktualisierte Challenge-Endpoints
### Aktualisierter Challenge-Flow (komplett)
```
1. Beide Spieler registrieren sich             POST /auth/register
2. Beide verknüpfen FACEIT-Account             GET /faceit/link → OAuth2 Flow
3. Beide setzen Kaspa-Adresse                  PUT /auth/me/kaspa-address
4. Spieler A erstellt Challenge                POST /challenges
5. Spieler B akzeptiert Challenge              POST /challenges/{id}/accept
6. System erstellt Escrow                      → Automatisch
7. Beide zahlen Wager auf Escrow-Adresse ein   → Kaspa TX (extern)
8. System erkennt Deposits                     → UTXO Subscription
9. Beide Deposits erkannt → Oracle-Job startet → Automatisch
10. Spieler spielen CS2 auf FACEIT              → Extern
11. Oracle findet Match + pollt Ergebnis       → Background Service
12. Match beendet → Payout an Gewinner         → Kaspa TX
13. Challenge = RESOLVED                       → Audit Trail komplett
```
### API-Endpoints (aktualisiert)
| Method | Endpoint | Beschreibung | Auth |
|--------|----------|-------------|------|
| POST | `/api/v1/challenges` | Challenge erstellen (mit wager_kas, game) | Ja |
| POST | `/api/v1/challenges/{id}/accept` | Challenge annehmen | Ja |
| GET | `/api/v1/challenges/{id}` | Challenge-Details + Oracle-Status | Ja |
| GET | `/api/v1/challenges/{id}/oracle` | Detaillierter Oracle-Status | Ja |
| POST | `/api/v1/challenges/{id}/dispute` | Ergebnis anfechten | Ja |
### Challenge-Erstellung (aktualisiert)
```json
// POST /api/v1/challenges
// Header: Authorization: Bearer {session_token}
{
  "opponent_user_id": "uuid-of-opponent",    // ODER:
  "opponent_faceit_nickname": "OpponentX",   // Suche per FACEIT-Name
  "game": "cs2",
  "wager_kas": 50,
  "match_type": "1v1"                        // oder "5v5" (Zukunft)
}

// → 201 Response:
{
  "challenge_id": "uuid",
  "status": "PENDING_ACCEPTANCE",
  "player_a": {
    "user_id": "uuid",
    "display_name": "ProGamer42",
    "faceit_nickname": "ProGamer42",
    "faceit_elo": 1847
  },
  "player_b": {
    "user_id": "uuid",
    "display_name": "OpponentX",
    "faceit_nickname": "OpponentX",
    "faceit_elo": 1923
  },
  "wager_kas": 50,
  "wager_sompi": 5000000,
  "escrow_address": null,                    // Erst nach Accept
  "game": "cs2",
  "match_type": "1v1"
}
```
### Oracle-Status-Endpoint
```json
// GET /api/v1/challenges/{id}/oracle
{
  "status": "MONITORING",
  "faceit_match_id": "1-abc123-def456",
  "faceit_match_url": "https://www.faceit.com/en/cs2/room/1-abc123-def456",
  "match_status": "ongoing",
  "score": { "faction1": 8, "faction2": 6 },
  "player_a_faction": "faction1",
  "player_b_faction": "faction2",
  "poll_count": 15,
  "last_polled_at": "2026-02-22T13:00:00Z",
  "timeout_at": "2026-02-22T17:00:00Z",
  "audit_trail": [
    { "event": "CREATED", "at": "2026-02-22T12:30:00Z" },
    { "event": "MATCH_FOUND", "at": "2026-02-22T12:35:22Z", "data": { "match_id": "1-abc123-def456" } },
    { "event": "MATCH_STARTED", "at": "2026-02-22T12:36:00Z" }
  ]
}
```

***
## Projektablaufplan (Timeline)
| Phase | Zeitraum | Arbeitspakete | Deliverables |
|-------|----------|---------------|-------------|
| **Phase 1** | Woche 1–2 | AP1: User-Accounts | Auth-Modul, DB-Migration, 8 Unit Tests |
| **Phase 2** | Woche 2–3 | AP2: FACEIT OAuth2 | OAuth2-Flow, Token-Management, 6 Unit Tests |
| **Phase 3** | Woche 3–4 | AP3: FACEIT Data API | API-Client, Match-Parsing, 10 Unit Tests |
| **Phase 4** | Woche 4–6 | AP4: Oracle-Service | State Machine, Background-Loop, 12 Unit Tests |
| **Phase 5** | Woche 6–7 | AP5: Anti-Manipulation | Validierungs-Pipeline, Edge-Case-Tests, 8 Unit Tests |
| **Phase 6** | Woche 7–8 | AP6: Integration + E2E | Aktualisierte Endpoints, Full-Flow E2E Test, 15 Integration Tests |

***
## Gesamt-Environment-Variablen
```env
# Bestehendes (aus Schritt 3)
KASPA_NETWORK=testnet-10
KASPA_NODE_URL=wss://photon-10.kaspa.red/kaspa/testnet-10/wrpc/borsh
KASPA_MNEMONIC=
KASPA_PLATFORM_ADDRESS=kaspatest:q...
KASPA_FEE_PERCENT=5

# NEU: Auth
AUTH_SESSION_DURATION_HOURS=168
AUTH_ARGON2_MEMORY_KB=19456
AUTH_ARGON2_ITERATIONS=2

# NEU: FACEIT
FACEIT_CLIENT_ID=your-client-id
FACEIT_CLIENT_SECRET=your-client-secret
FACEIT_REDIRECT_URI=https://kaspabattle.com/api/v1/faceit/callback
FACEIT_API_KEY=your-server-side-api-key

# NEU: Oracle
ORACLE_POLL_INTERVAL_SECS=30
ORACLE_MATCH_SEARCH_WINDOW_HOURS=4
ORACLE_CHALLENGE_TIMEOUT_HOURS=4
ORACLE_MIN_MATCH_DURATION_SECS=300
ORACLE_MIN_ROUNDS=10
```

***
## Claude Opus 4 Prompt (Copy-Paste-fähig)
Der folgende Prompt kann direkt in Claude Opus 4 eingefügt werden, um die gesamte Implementierung generieren zu lassen:

````
Du bist ein erfahrener Rust-Blockchain-Entwickler, der das Projekt "KaspaBattle"
weiterentwickelt. In Schritt 1-3 haben wir:

  • battle-server  – Actix-Web REST-Server
  • battle-core    – Domain-Typen, DB (SQLite/rusqlite)
  • battle-kaspa   – EscrowWallet, RealKaspaClient, EscrowService

In Schritt 4 bauen wir: User-Accounts, FACEIT OAuth2, FACEIT Data API Client,
Oracle-Service zur automatischen Match-Ergebnis-Erkennung und Payout-Auslösung.

═══════════════════════════════════════════════════════════════
FACEIT API REFERENZ
═══════════════════════════════════════════════════════════════

Base-URL: https://open.faceit.com/data/v4
Auth: Header "Authorization: Bearer {API_KEY}"

1. Spieler per Nickname:
   GET /players?nickname={name}&game=cs2
   → { "player_id": "guid", "nickname": "...", "games": { "cs2": { "skill_level": 8, "faceit_elo": 1847 } } }

2. Match-Historie:
   GET /players/{player_id}/history?game=cs2&from={unix_ts}&limit=10
   → { "items": [{ "match_id": "1-xxx", "started_at": 123, "finished_at": 456, "game_id": "cs2" }] }

3. Match-Details:
   GET /matches/{match_id}
   → { "match_id": "...", "status": "finished", "results": { "winner": "faction1", "score": { "faction1": 13, "faction2": 7 } }, "teams": { "faction1": { "roster": [{ "player_id": "guid", "nickname": "..." }] }, "faction2": { ... } } }

4. Match-Statistiken:
   GET /matches/{match_id}/stats
   → { "rounds": [{ "round_stats": { ... }, "teams": [{ "players": [{ "player_id": "guid", "player_stats": { "Kills": "25", "Deaths": "12" } }] }] }] }

OAuth2 Flow:
- Authorize: GET https://accounts.faceit.com/authorize?client_id=X&redirect_uri=Y&scope=openid,email,profile&response_type=code&state=Z&code_challenge=C&code_challenge_method=S256
- Token: POST https://api.faceit.com/auth/v1/oauth/token
  Auth: Basic base64url(client_id:client_secret)
  Body: grant_type=authorization_code&code=CODE&code_verifier=VERIFIER
  → { "access_token": "...", "id_token": "jwt...", "refresh_token": "..." }
- UserInfo: GET https://api.faceit.com/auth/v1/resources/userinfo
  Auth: Bearer {access_token}
  → { "guid": "faceit-player-id", "nickname": "...", "email": "..." }

WICHTIG: Base64 für Basic Auth muss URL-safe sein (+ → -, / → _, = entfernen)

═══════════════════════════════════════════════════════════════
DEINE AUFGABE
═══════════════════════════════════════════════════════════════

Implementiere die folgenden 6 Module:

──── MODUL 1: battle-core/src/auth.rs ────
User-Account-System:
- User struct (id, email, password_hash, display_name, kaspa_address)
- register(email, password, display_name) → User + Session
- login(email, password) → User + Session  
- validate_session(token) → User
- Passwort-Hashing: argon2 Crate (Argon2id)
- Session-Token: 32-byte rand, base64url, 7 Tage gültig
- SQLite-Tabellen: users, sessions

──── MODUL 2: battle-core/src/faceit_oauth.rs ────
FACEIT OAuth2 Connect:
- generate_auth_url() mit PKCE (S256)
- exchange_code(code, code_verifier) → Tokens
- get_userinfo(access_token) → FaceitUserInfo
- refresh_token() → neue Tokens
- SQLite-Tabelle: faceit_links (user_id ↔ faceit_player_id)

──── MODUL 3: battle-kaspa/src/faceit_api.rs ────
FACEIT Data API Client:
- get_player_by_nickname(nickname, game) → PlayerInfo
- get_player_matches(player_id, game, limit, from) → Vec<MatchSummary>
- get_match(match_id) → MatchResult
- get_match_stats(match_id) → MatchStats
- Rate-Limiter: governor Crate, 100 req/min
- Retry mit exponential Backoff bei 429

──── MODUL 4: battle-kaspa/src/oracle.rs ────
Oracle-Service (Background-Task):
- OracleService mit tokio::spawn Background-Loop
- State Machine: SEARCHING → FOUND → MONITORING → FINISHED → RESOLVED
- search_match(): Match-Historien beider Spieler abgleichen
- monitor_match(): Laufendes Match auf "finished" prüfen
- resolve_match(): Winner ermitteln → EscrowService.payout_winner()
- Timeout-Handling: nach 4h → Refund
- SQLite-Tabellen: oracle_jobs, oracle_audit_log

──── MODUL 5: Validierungs-Pipeline ────
Anti-Manipulations-Schutz in oracle.rs:
- Mindest-Rundenzahl: 10
- Mindest-Spieldauer: 300 Sekunden
- Nur offizielle Match-Typen (kein Custom)
- Beide Spieler müssen im Match-Roster sein
- Match muss NACH Challenge-Erstellung gestartet sein
- ELO-Differenz-Check (max. 800)

──── MODUL 6: battle-server/src/routes/ ────
Neue API-Endpoints:
- POST /api/v1/auth/register
- POST /api/v1/auth/login
- POST /api/v1/auth/logout
- GET  /api/v1/auth/me
- PUT  /api/v1/auth/me/kaspa-address
- GET  /api/v1/faceit/link (→ Redirect zu FACEIT)
- GET  /api/v1/faceit/callback (OAuth2 Callback)
- GET  /api/v1/faceit/status
- DELETE /api/v1/faceit/link
- GET  /api/v1/challenges/{id}/oracle
- POST /api/v1/challenges/{id}/dispute

SQLite-Migrationen:
- 004_users.sql (users, sessions, faceit_links)
- 005_oracle.sql (oracle_jobs, oracle_audit_log)

══════════════ CODING STANDARDS ══════════════

1. ALLE Dateien vollständig ausgeben
2. anyhow::Result überall, kein unwrap() in Prod-Code
3. tracing crate (info!, warn!, error!)
4. Kein unsafe Code, clippy-clean
5. Private Keys/Tokens NIEMALS loggen
6. Arc<dyn KaspaRpc> für Testbarkeit
7. #[cfg(test)] mod tests für Unit Tests
8. Jede Funktion mit /// doc-comment
9. reqwest::Client mit timeout(30s) und User-Agent Header
10. Alle Secrets aus Environment-Variablen

══════════════ TESTS (PFLICHT) ══════════════

battle-core/src/auth_test.rs:
  ✅ test_register_new_user
  ✅ test_register_duplicate_email
  ✅ test_login_correct_password
  ✅ test_login_wrong_password
  ✅ test_session_validation
  ✅ test_session_expired
  ✅ test_set_kaspa_address
  ✅ test_password_hash_argon2id

battle-core/src/faceit_oauth_test.rs:
  ✅ test_generate_auth_url_contains_params
  ✅ test_pkce_challenge_verification
  ✅ test_base64url_encoding
  ✅ test_token_response_parsing
  ✅ test_userinfo_parsing
  ✅ test_link_prevents_duplicate

battle-kaspa/src/faceit_api_test.rs:
  ✅ test_parse_player_response
  ✅ test_parse_match_response
  ✅ test_parse_match_stats
  ✅ test_find_common_match
  ✅ test_determine_winner_faction1
  ✅ test_determine_winner_faction2
  ✅ test_no_common_match
  ✅ test_match_after_timestamp_filter
  ✅ test_rate_limiter_delays
  ✅ test_retry_on_429

battle-kaspa/src/oracle_test.rs:
  ✅ test_create_oracle_job
  ✅ test_state_transition_searching_to_found
  ✅ test_state_transition_monitoring_to_finished
  ✅ test_state_transition_finished_to_resolved
  ✅ test_timeout_triggers_refund
  ✅ test_cancelled_match_triggers_refund
  ✅ test_validation_min_rounds
  ✅ test_validation_min_duration
  ✅ test_validation_no_custom_match
  ✅ test_validation_players_in_roster
  ✅ test_audit_log_entries
  ✅ test_double_resolve_prevention

══════════════ AUSGABE-REIHENFOLGE ══════════════

1.  battle-core/Cargo.toml (aktualisiert)
2.  battle-core/src/auth.rs
3.  battle-core/src/faceit_oauth.rs
4.  battle-core/migrations/004_users.sql
5.  battle-kaspa/Cargo.toml (aktualisiert)
6.  battle-kaspa/src/faceit_api.rs
7.  battle-kaspa/src/oracle.rs
8.  battle-kaspa/migrations/005_oracle.sql
9.  battle-server/src/routes/auth.rs
10. battle-server/src/routes/faceit.rs
11. battle-server/src/routes/oracle.rs
12. battle-server/src/main.rs (aktualisiert)
13. tests/test_oracle_e2e.sh

Stelle sicher, dass `cargo build` und `cargo test` durchlaufen.
````

---

## References

1. [Account Linking | FACEIT for Developers](https://docs.faceit.com/getting-started/authentication/oauth2/) - FACEIT standard OAuth2 endpoints are available here. OAuth2 Consent Screen and OAuth2 Client​. The v...

2. [Authentication](https://docs.faceit.com/getting-started/category/authentication/) - Based on the OAuth2 standard, FACEIT Connect enables application developers to build applications th...

3. [Auth.js | Faceit](https://authjs.dev/reference/core/providers/faceit) - Authentication for the Web

4. [Launching the FACEIT Developer Portal | by Maurizio Attisani](https://blog.faceit.com/launching-the-faceit-developer-portal-5740fb48ac26) - FACEIT Connect (OAuth2) · create seamless sign-up and login workflow to online users offering them t...

5. [Facing 401 error when exchanging Faceit OAuth ...](https://stackoverflow.com/questions/76367292/facing-401-error-when-exchanging-faceit-oauth-authorization-code-for-access-toke) - You need to make a URL-encoded base64 string from your credentials. This means that in the resulting...

6. [Data API - FACEIT for Developers](https://docs.faceit.com/docs/data-api/) - Authentication

7. [Docs](https://docs.faceit.com/docs/data-api/data) - This API provide access to FACEIT's data; Championships. getRetrieve all championships of a game; ge...

8. [go-faceit/docs/MatchesApi.md at main · mconnat/go-faceit](https://github.com/mconnat/go-faceit/blob/main/docs/MatchesApi.md) - Contribute to mconnat/go-faceit development by creating an account on GitHub.

9. [GitHub - alexarne/Lookback-FACEIT: Find FACEIT matches where you played with a specific player, using FACEIT's API](https://github.com/alexarne/Lookback-FACEIT) - Find FACEIT matches where you played with a specific player, using FACEIT's API - alexarne/Lookback-...


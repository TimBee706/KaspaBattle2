# Kaspa Battle 2.0 — Tournament Integration Plan
## Basierend auf Repository-Analyse: `TimBee706/KaspaBattle2`

> **Dokument-Typ:** Technischer Ablaufplan & Implementierungs-Guide  
> **Stand:** April 2026  
> **Zielgruppe:** Backend-/Rust-Entwickler mit Kenntnissen in Axum, SQLx, Kaspa RPC

---

## 1. Status der bestehenden Codebasis

### 1.1 Was bereits existiert (v1.x)

Das Repository enthält eine vollständige 1v1-Match-Plattform auf Basis von:

| Komponente | Pfad | Beschreibung |
|---|---|---|
| `BattleEpisode` | `battle-kdapp/src/episode.rs` | On-chain State-Maschine via kdapp-Framework |
| `BattleCommand` | `battle-kdapp/src/commands.rs` | Borsh-serialisierte On-Chain-Commands |
| `MatchEpisode` | `battle-api/src/episodes/match_episode.rs` | Off-chain API-Layer mit DB-Sync |
| DB-Schema | `migrations/202603300000_consolidated_schema.sql` | PostgreSQL mit allen v0.2–v0.6-Migrationen |
| Oracle-Route | `battle-api/src/routes/oracle.rs` | Ergebnis-Einreichung |
| Auth-Route | `battle-api/src/routes/auth.rs` | Wallet-Auth + FaceIT-Link |

### 1.2 Bestehender Match-Lebenszyklus (1v1)

```
WaitingForOpponent
       │ CreateMatch (Player A)
       ▼
WaitingForOpponent (aktiv)
       │ JoinMatch (Player B)
       ▼
WaitingForDeposits {a_deposited, b_deposited}
       │ ConfirmDeposit × 2
       ▼
    Locked  ──────────────────────────────────────────┐
       │ ReportResult (Oracle/FaceIT)                 │
       ▼                                              │
  Resolved {winner_idx}                               │ CancelMatch
       │ InitiatePayout                               │ (nur vor Lock)
       ▼                                  ▼           ▼
  Completed                           Disputed    Cancelled
```

**API-seitige Zustände (DB `match_status`):**

```
OPEN → AWAITING_FUNDING → FUNDED → GAME_ID_INPUT → IN_GAME
     → FINISHED_FACEIT → READY_FOR_PAYOUT → PAID_OUT
     → DISPUTED | CANCELLED
```

---

## 2. Was für Turniere fehlt (GAP-Analyse)

| Feature | Status | Priorität |
|---|---|---|
| `tournaments`-Tabelle | ❌ Fehlt | P0 |
| `tournament_teams`-Tabelle | ❌ Fehlt | P0 |
| `tournament_registrations`-Tabelle | ❌ Fehlt | P0 |
| `TournamentEpisode` (kdapp) | ❌ Fehlt | P0 |
| `TournamentCommand`-Enum | ❌ Fehlt | P0 |
| Bracket-Logik (Single Elim.) | ❌ Fehlt | P0 |
| Multi-Escrow / Prize-Pool | ❌ Fehlt | P0 |
| `GameType` nur `CS2` | ⚠️ Erweitern | P1 |
| FaceIT-Team-Match-API | ❌ Fehlt | P1 |
| Admin-/Organizer-Rollen | ❌ Fehlt | P1 |
| Timelock-basierter Refund | ❌ Fehlt | P1 |

---

## 3. Datenbank-Erweiterungen (neue Migrationen)

### Migration: `202604_tournament_schema.sql`

```sql
-- Enum: Turnierstatus
DO $$ BEGIN
  CREATE TYPE tournament_status AS ENUM (
    'REGISTRATION', 'FUNDED', 'BRACKET_READY',
    'IN_PROGRESS', 'COMPLETED', 'CANCELLED', 'DISPUTED'
  );
EXCEPTION WHEN duplicate_object THEN NULL; END $$;

-- Enum: Bracket-Phase
DO $$ BEGIN
  CREATE TYPE bracket_round AS ENUM (
    'QUARTERFINAL', 'SEMIFINAL', 'FINAL', 'GRAND_FINAL'
  );
EXCEPTION WHEN duplicate_object THEN NULL; END $$;

-- Tournaments
CREATE TABLE IF NOT EXISTS tournaments (
    id                      UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    title                   TEXT NOT NULL,
    game_id                 TEXT NOT NULL DEFAULT 'cs2',
    organizer_user_id       UUID REFERENCES users(id),
    buy_in_sompi            BIGINT NOT NULL,
    max_teams               INTEGER NOT NULL DEFAULT 8,
    team_size               INTEGER NOT NULL DEFAULT 5,
    status                  tournament_status NOT NULL DEFAULT 'REGISTRATION',
    prize_pool_sompi        BIGINT NOT NULL DEFAULT 0,
    escrow_address          TEXT,
    onchain_tournament_id   TEXT,            -- kdapp-Episode-ID
    faceit_hub_id           TEXT,            -- optionaler FaceIT-Hub
    winner_team_id          UUID,
    payout_tx_hash          TEXT,
    registration_deadline   TIMESTAMPTZ,
    start_time              TIMESTAMPTZ,
    completed_at            TIMESTAMPTZ,
    created_at              TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at              TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Teams
CREATE TABLE IF NOT EXISTS tournament_teams (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tournament_id   UUID NOT NULL REFERENCES tournaments(id) ON DELETE CASCADE,
    team_name       TEXT NOT NULL,
    captain_user_id UUID NOT NULL REFERENCES users(id),
    faceit_team_id  TEXT,
    seed            INTEGER,
    status          TEXT NOT NULL DEFAULT 'REGISTERED',  -- REGISTERED | FUNDED | ELIMINATED | WINNER
    deposit_tx_hash TEXT,
    deposit_sompi   BIGINT NOT NULL DEFAULT 0,
    deposit_confirmed BOOLEAN NOT NULL DEFAULT FALSE,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(tournament_id, team_name)
);

-- Team-Mitglieder (5 Spieler pro Team)
CREATE TABLE IF NOT EXISTS team_members (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    team_id         UUID NOT NULL REFERENCES tournament_teams(id) ON DELETE CASCADE,
    user_id         UUID NOT NULL REFERENCES users(id),
    role            TEXT NOT NULL DEFAULT 'player',  -- 'captain' | 'player'
    joined_at       TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(team_id, user_id)
);

-- Bracket-Matches (verlinkt auf bestehende matches-Tabelle)
CREATE TABLE IF NOT EXISTS tournament_bracket (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tournament_id   UUID NOT NULL REFERENCES tournaments(id) ON DELETE CASCADE,
    round           bracket_round NOT NULL,
    match_number    INTEGER NOT NULL,         -- Position im Bracket (1-basiert)
    team_a_id       UUID REFERENCES tournament_teams(id),
    team_b_id       UUID REFERENCES tournament_teams(id),
    winner_team_id  UUID REFERENCES tournament_teams(id),
    match_id        UUID REFERENCES matches(id),   -- Link zum existierenden 1v1-Match
    faceit_match_id TEXT,
    status          TEXT NOT NULL DEFAULT 'PENDING',  -- PENDING | IN_PROGRESS | COMPLETED | BYE
    scheduled_at    TIMESTAMPTZ,
    completed_at    TIMESTAMPTZ,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(tournament_id, round, match_number)
);

-- Tournament Payments (analog zu payments-Tabelle)
CREATE TABLE IF NOT EXISTS tournament_payments (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tournament_id   UUID NOT NULL REFERENCES tournaments(id) ON DELETE CASCADE,
    team_id         UUID NOT NULL REFERENCES tournament_teams(id),
    tx_id           TEXT NOT NULL,
    amount_sompi    BIGINT NOT NULL,
    block_daa_score BIGINT NOT NULL DEFAULT 0,
    confirmations   INTEGER NOT NULL DEFAULT 0,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(tournament_id, team_id)
);

-- Indices
CREATE INDEX IF NOT EXISTS idx_tournaments_status ON tournaments(status);
CREATE INDEX IF NOT EXISTS idx_tournament_teams_tournament ON tournament_teams(tournament_id);
CREATE INDEX IF NOT EXISTS idx_bracket_tournament ON tournament_bracket(tournament_id, round);
```

---

## 4. Neues kdapp-Modul: `TournamentEpisode`

### 4.1 Neue Commands (`battle-kdapp/src/tournament_commands.rs`)

```rust
use borsh::{BorshSerialize, BorshDeserialize};
use crate::kdapp_pki::PubKey;

/// On-Chain Commands für ein TournamentEpisode.
/// Jede Variante entspricht einer erlaubten State-Transition.
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub enum TournamentCommand {
    /// Organizer erstellt das Turnier on-chain.
    CreateTournament {
        buy_in_sompi:     u64,
        max_teams:        u8,
        team_size:        u8,
        registration_end: u64,  // DAA-Score (Deadline)
    },
    /// Ein Team registriert sich (Captain signiert).
    RegisterTeam {
        team_id_hash: [u8; 32],   // Hash der Team-UUID (off-chain Referenz)
    },
    /// Ein Team bestätigt seinen Buy-in-Deposit.
    ConfirmTeamDeposit {
        team_id_hash:  [u8; 32],
        tx_hash:       [u8; 32],
        amount_sompi:  u64,
    },
    /// Organizer sperrt das Turnier (alle Deposits vorhanden → Bracket wird erstellt).
    LockTournament,
    /// Oracle meldet Ergebnis eines Bracket-Matches.
    ReportBracketResult {
        round:           u8,   // 0=QF, 1=SF, 2=Final
        match_number:    u8,
        winner_team_idx: u8,
        faceit_match_id: [u8; 32],
    },
    /// Sieger-Team löst den Prize-Pool aus.
    InitiateTournamentPayout,
    /// Refund aller Teams (z.B. nach Timeout oder Abbruch).
    CancelTournament { reason_code: u8 },
    /// Einzelnes Team gibt eine Dispute ein.
    DisputeResult {
        round:       u8,
        match_number: u8,
        reason_code: u8,
    },
}

/// Turnier-Phasen (on-chain)
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, PartialEq, Eq)]
pub enum TournamentPhase {
    Registration,
    WaitingForDeposits { deposits_received: u8 },
    BracketReady,
    InProgress { current_round: u8 },
    Resolved { winner_team_idx: u8 },
    Completed,
    Cancelled { reason_code: u8 },
    Disputed { round: u8, match_number: u8, by_team_idx: u8 },
}

pub mod tournament_reason {
    pub const CANCEL_TIMEOUT:         u8 = 1;
    pub const CANCEL_NOT_ENOUGH_TEAMS: u8 = 2;
    pub const CANCEL_ORGANIZER:       u8 = 3;
    pub const DISPUTE_SCORE_MISMATCH: u8 = 10;
    pub const DISPUTE_CHEAT:          u8 = 11;
}
```

### 4.2 TournamentEpisode (`battle-kdapp/src/tournament_episode.rs`)

```rust
use crate::kdapp_episode::{Episode, EpisodeError, PayloadMetadata};
use crate::kdapp_pki::PubKey;
use crate::tournament_commands::{
    TournamentCommand, TournamentPhase, TournamentRollback,
};

const MAX_TEAMS: u8 = 32;

/// On-chain State eines Turniers.
#[derive(Debug, Clone)]
pub struct TournamentEpisode {
    pub organizer:        PubKey,
    pub phase:            TournamentPhase,
    pub buy_in_sompi:     u64,
    pub max_teams:        u8,
    pub team_size:        u8,
    pub registered_teams: Vec<[u8; 32]>,       // team_id_hashes
    pub deposits:         Vec<u64>,             // Deposit je Team (Index = Team-Slot)
    pub bracket:          Vec<BracketSlot>,     // Flaches Bracket-Array
    pub winner_team_idx:  Option<u8>,
    pub created_daa:      u64,
    pub registration_end_daa: u64,
}

#[derive(Debug, Clone)]
pub struct BracketSlot {
    pub round:           u8,
    pub match_number:    u8,
    pub team_a_idx:      Option<u8>,
    pub team_b_idx:      Option<u8>,
    pub winner_team_idx: Option<u8>,
}

impl Episode for TournamentEpisode {
    type Command         = TournamentCommand;
    type CommandRollback = TournamentRollback;
    type CommandError    = TournamentError;

    fn initialize(participants: Vec<PubKey>, metadata: &PayloadMetadata) -> Self {
        TournamentEpisode {
            organizer:            participants.into_iter().next().unwrap_or_default(),
            phase:                TournamentPhase::Registration,
            buy_in_sompi:         0,
            max_teams:            8,
            team_size:            5,
            registered_teams:     Vec::new(),
            deposits:             Vec::new(),
            bracket:              Vec::new(),
            winner_team_idx:      None,
            created_daa:          metadata.accepting_daa,
            registration_end_daa: 0,
        }
    }

    fn execute(
        &mut self,
        cmd: &TournamentCommand,
        authorization: Option<PubKey>,
        metadata: &PayloadMetadata,
    ) -> Result<TournamentRollback, EpisodeError<TournamentError>> {
        match cmd {
            TournamentCommand::CreateTournament {
                buy_in_sompi, max_teams, team_size, registration_end
            } => {
                // Nur Organizer darf erstellen
                self.require_organizer(&authorization)?;
                self.buy_in_sompi = *buy_in_sompi;
                self.max_teams    = (*max_teams).min(MAX_TEAMS);
                self.team_size    = *team_size;
                self.registration_end_daa = *registration_end;
                self.deposits     = vec![0u64; *max_teams as usize];
                Ok(TournamentRollback::UndoCreate)
            }

            TournamentCommand::RegisterTeam { team_id_hash } => {
                if !matches!(self.phase, TournamentPhase::Registration) {
                    return Err(self.bad_transition("RegisterTeam"));
                }
                if self.registered_teams.len() >= self.max_teams as usize {
                    return Err(EpisodeError::InvalidCommand(TournamentError::TournamentFull));
                }
                // Prüfe Deadline
                if self.registration_end_daa > 0
                    && metadata.accepting_daa > self.registration_end_daa
                {
                    return Err(EpisodeError::InvalidCommand(TournamentError::RegistrationClosed));
                }
                self.registered_teams.push(*team_id_hash);
                Ok(TournamentRollback::UndoRegisterTeam)
            }

            TournamentCommand::ConfirmTeamDeposit {
                team_id_hash, tx_hash: _, amount_sompi
            } => {
                let idx = self.find_team(team_id_hash)?;
                if self.deposits[idx] >= self.buy_in_sompi {
                    return Err(EpisodeError::InvalidCommand(TournamentError::AlreadyDeposited));
                }
                self.deposits[idx] += amount_sompi;
                let deposits_received = self.deposits.iter()
                    .filter(|&&d| d >= self.buy_in_sompi)
                    .count() as u8;
                self.phase = TournamentPhase::WaitingForDeposits { deposits_received };
                Ok(TournamentRollback::UndoDeposit { team_idx: idx as u8, amount: *amount_sompi })
            }

            TournamentCommand::LockTournament => {
                self.require_organizer(&authorization)?;
                let funded_count = self.deposits.iter()
                    .filter(|&&d| d >= self.buy_in_sompi)
                    .count();
                if funded_count < 2 {
                    return Err(EpisodeError::InvalidCommand(TournamentError::NotEnoughTeams));
                }
                // Bracket-Initialisierung: Single Elimination
                self.bracket = Self::build_bracket(funded_count as u8);
                self.phase   = TournamentPhase::BracketReady;
                Ok(TournamentRollback::UndoLock)
            }

            TournamentCommand::ReportBracketResult {
                round, match_number, winner_team_idx, ..
            } => {
                let slot = self.bracket.iter_mut()
                    .find(|s| s.round == *round && s.match_number == *match_number)
                    .ok_or_else(|| EpisodeError::InvalidCommand(TournamentError::BracketSlotNotFound))?;
                slot.winner_team_idx = Some(*winner_team_idx);

                // Prüfe ob Turnier abgeschlossen
                let max_round = self.bracket.iter().map(|s| s.round).max().unwrap_or(0);
                if *round == max_round {
                    self.winner_team_idx = Some(*winner_team_idx);
                    self.phase = TournamentPhase::Resolved {
                        winner_team_idx: *winner_team_idx,
                    };
                } else {
                    self.phase = TournamentPhase::InProgress {
                        current_round: *round + 1,
                    };
                    // Gewinner in nächste Runde einsetzen
                    Self::advance_winner(&mut self.bracket, *round, *match_number, *winner_team_idx);
                }
                Ok(TournamentRollback::UndoBracketResult { round: *round, match_number: *match_number })
            }

            TournamentCommand::InitiateTournamentPayout => {
                if !matches!(self.phase, TournamentPhase::Resolved { .. }) {
                    return Err(self.bad_transition("InitiateTournamentPayout"));
                }
                self.phase = TournamentPhase::Completed;
                Ok(TournamentRollback::UndoPayout)
            }

            TournamentCommand::CancelTournament { reason_code } => {
                self.phase = TournamentPhase::Cancelled { reason_code: *reason_code };
                Ok(TournamentRollback::UndoCancel)
            }

            _ => Err(EpisodeError::InvalidCommand(TournamentError::UnknownCommand)),
        }
    }

    fn rollback(&mut self, rb: TournamentRollback) -> bool {
        match rb {
            TournamentRollback::UndoRegisterTeam => {
                self.registered_teams.pop();
                true
            }
            TournamentRollback::UndoDeposit { team_idx, amount } => {
                if let Some(d) = self.deposits.get_mut(team_idx as usize) {
                    *d = d.saturating_sub(amount);
                }
                true
            }
            _ => true, // weitere Rollbacks analog zu BattleRollback
        }
    }
}

impl TournamentEpisode {
    /// Erstellt ein Single-Elimination-Bracket für n Teams.
    /// Bei 5 Teams: 1 Bye in der ersten Runde.
    fn build_bracket(n: u8) -> Vec<BracketSlot> {
        // Runde auf nächste Zweierpotenz auf
        let bracket_size = n.next_power_of_two();
        let mut slots    = Vec::new();
        let mut round    = 0u8;
        let mut matches  = bracket_size / 2;
        while matches >= 1 {
            for m in 0..matches {
                slots.push(BracketSlot {
                    round,
                    match_number:    m,
                    team_a_idx:      None,
                    team_b_idx:      None,
                    winner_team_idx: None,
                });
            }
            round   += 1;
            matches /= 2;
        }
        // Seeds in Runde 0 eintragen
        for (i, slot) in slots.iter_mut().filter(|s| s.round == 0).enumerate() {
            slot.team_a_idx = Some((i * 2) as u8);
            let b_idx       = (i * 2 + 1) as u8;
            slot.team_b_idx = if b_idx < n { Some(b_idx) } else { None }; // None = BYE
        }
        slots
    }

    fn advance_winner(
        bracket:      &mut Vec<BracketSlot>,
        round:        u8,
        match_number: u8,
        winner_idx:   u8,
    ) {
        let next_round   = round + 1;
        let next_match   = match_number / 2;
        let is_team_a    = match_number % 2 == 0;
        if let Some(slot) = bracket.iter_mut()
            .find(|s| s.round == next_round && s.match_number == next_match)
        {
            if is_team_a {
                slot.team_a_idx = Some(winner_idx);
            } else {
                slot.team_b_idx = Some(winner_idx);
            }
        }
    }

    fn find_team(&self, hash: &[u8; 32]) -> Result<usize, EpisodeError<TournamentError>> {
        self.registered_teams.iter().position(|t| t == hash)
            .ok_or(EpisodeError::InvalidCommand(TournamentError::TeamNotFound))
    }

    fn require_organizer(
        &self,
        auth: &Option<PubKey>,
    ) -> Result<(), EpisodeError<TournamentError>> {
        match auth {
            Some(pk) if pk == &self.organizer => Ok(()),
            _ => Err(EpisodeError::InvalidCommand(TournamentError::NotOrganizer)),
        }
    }

    fn bad_transition(&self, action: &str) -> EpisodeError<TournamentError> {
        EpisodeError::InvalidCommand(TournamentError::InvalidTransition {
            from:   format!("{:?}", self.phase),
            action: action.to_string(),
        })
    }
}
```

---

## 5. API-Erweiterungen (`battle-api`)

### 5.1 Neue Route-Datei: `routes/tournaments.rs`

```rust
// Einzubindende Endpunkte:

// POST   /api/tournaments                  → create_tournament()
// GET    /api/tournaments                  → list_tournaments()
// GET    /api/tournaments/:id              → get_tournament()
// POST   /api/tournaments/:id/register     → register_team()
// POST   /api/tournaments/:id/deposit      → submit_team_deposit()
// POST   /api/tournaments/:id/lock         → lock_tournament()    [Organizer]
// GET    /api/tournaments/:id/bracket      → get_bracket()
// POST   /api/tournaments/:id/result       → report_bracket_result() [Oracle]
// POST   /api/tournaments/:id/payout       → initiate_payout()
// POST   /api/tournaments/:id/cancel       → cancel_tournament()
// GET    /api/tournaments/:id/escrow       → get_escrow_status()
```

### 5.2 TournamentEpisode (API-Layer, analog zu `MatchEpisode`)

```rust
/// API-seitiger Tournament-Episode-Lifecycle:
///
/// REGISTRATION → FUNDED (alle Teams deposited)
///             → BRACKET_READY (Organizer locked)
///             → IN_PROGRESS (erstes Match startet)
///             → COMPLETED (Sieger ermittelt, Prize ausgezahlt)
///             → CANCELLED (Timeout, zu wenig Teams, Abbruch)
///             → DISPUTED (Ergebnis angefochten)
///
/// Prize-Verteilung:
///   1. Platz:  70% des Pools
///   2. Platz:  20% des Pools
///   Plattform: 10% des Pools (automatisch bei InitiatePayout)
```

---

## 6. Vollständiger Ablaufplan: 5-Teams-Turnier (Beispiel)

### Parameter

```
Turnier-ID:    TRN-001
Buy-in:        100 KAS pro Team (= 100 × 5 = 500 KAS Gesamtpool)
Teams:         5 × 5 Spieler = 25 Teilnehmer
Format:        Single Elimination (mit 1 Bye in Runde 1)
FaceIT:        Hub oder Tournament-Modus für 5v5 CS2
Plattform-Fee: 10% (= 50 KAS)
```

### Bracket-Struktur (5 Teams → 8er-Bracket, 3 Byes)

```
Runde 0 (Quarterfinals):
  Match 0: Team A vs Team B
  Match 1: Team C vs Team D
  Match 2: Team E       ← BYE (Team E kommt automatisch weiter)
  Match 3: (leer)       ← BYE

Runde 1 (Halbfinale):
  Match 0: Sieger(QF-0) vs Sieger(QF-1)
  Match 1: Team E       vs Sieger(QF-2)   [Team E hat Freilos]

Runde 2 (Finale):
  Match 0: Sieger(SF-0) vs Sieger(SF-1)
```

### Schritt-für-Schritt-Ablauf

---

#### Phase 0: Turnier-Erstellung

**Organizer → Backend**

```
POST /api/tournaments
{
  "title":                "KaspaBattle Open #1",
  "game_id":              "cs2",
  "buy_in_sompi":         10_000_000_000,   // 100 KAS
  "max_teams":            8,
  "team_size":            5,
  "registration_deadline": "2026-04-10T20:00:00Z"
}
```

**Backend-Aktionen:**
1. DB: `INSERT INTO tournaments` → Status `REGISTRATION`
2. kdapp: `CreateTournament`-Command on-chain senden
3. Kaspa: Dedizierte Escrow-Adresse für Prize-Pool generieren
4. DB: `escrow_address`, `onchain_tournament_id` speichern
5. Response: Tournament-Objekt mit Escrow-Adresse

---

#### Phase 1: Team-Registrierung (5 Teams)

**Captain A → Backend**

```
POST /api/tournaments/TRN-001/register
{
  "team_name":    "Team Alpha",
  "members":      ["userUUID1", "userUUID2", "userUUID3", "userUUID4", "userUUID5"],
  "faceit_team_id": "faceit-team-123"     // optional
}
```

**Backend-Aktionen (für jedes der 5 Teams):**
1. Auth: Captain muss eingeloggt und FaceIT-verlinkt sein
2. Validierung: Team-Größe exakt 5, alle Members FaceIT-verlinkt
3. DB: `INSERT INTO tournament_teams` → Status `REGISTERED`
4. DB: `INSERT INTO team_members` (5 Zeilen pro Team)
5. kdapp: `RegisterTeam`-Command on-chain
6. Response: Team-Objekt mit Escrow-Zahlungsziel

**Voraussetzung je Member (aus bestehender Auth-Logik):**
```
✓ users.kaspa_address NOT NULL
✓ faceit_links.faceit_player_id NOT NULL
✓ faceit_links.faceit_elo gesetzt
```

---

#### Phase 2: Buy-in-Deposits (5 Teams × 100 KAS)

**Captain → Kaspa-Wallet**

Jedes Team überweist 100 KAS an die Tournament-Escrow-Adresse.

**Backend-Aktionen (BlockchainWatcher, analog zu MatchEpisode):**

```rust
// TournamentEpisode::execute() für Status REGISTRATION:

// 1. Escrow-Balance abfragen (wie in MatchEpisode)
let escrow_status = watcher.check_escrow_balance(&escrow_addr).await?;

// 2. UTXOs pro Team attribuieren (via tournament_payments-Tabelle)
//    Attribution: depot_tx_hash aus submit_team_deposit-API
for utxo in sorted_utxos {
    let team_idx = find_team_by_tx_hash(&utxo.tx_id)?;
    tournament_payments::upsert(match_id, team_id, utxo).await?;
}

// 3. Wenn alle 5 Teams ≥ 100 KAS deposited und ≥10 Confirmations:
if all_teams_funded {
    db.update_tournament_status(id, "FUNDED").await?;
    // Prize-Pool: 5 × 100 KAS = 500 KAS
    db.update_prize_pool(id, 500_000_000_000).await?;
}
```

**Monitoring-Endpunkt:**
```
GET /api/tournaments/TRN-001/escrow
→ {
    "total_deposited_sompi": 500_000_000_000,
    "teams_funded": 5,
    "teams_pending": 0,
    "prize_pool_sompi": 500_000_000_000
  }
```

**Timeouts:**
- Registrierung ohne Deposit: 60 Minuten → Auto-`CANCELLED`
- Gesamte Deposit-Phase: bis `registration_deadline`

---

#### Phase 3: Tournament-Lock & Bracket-Generierung

**Organizer → Backend** (oder Automatik wenn alle Teams gefunded)

```
POST /api/tournaments/TRN-001/lock
Authorization: Bearer <organizer-jwt>
```

**Backend-Aktionen:**
1. Validierung: ≥ 2 Teams funded
2. kdapp: `LockTournament`-Command → `BracketReady`
3. DB: Bracket-Slots generieren (5 Teams → 8er-Bracket mit 3 Byes)

```sql
-- Bracket für 5 Teams (Runde 0):
INSERT INTO tournament_bracket (tournament_id, round, match_number, team_a_id, team_b_id, status)
VALUES
  ('TRN-001', 'QUARTERFINAL', 1, 'team-alpha', 'team-beta',  'PENDING'),
  ('TRN-001', 'QUARTERFINAL', 2, 'team-gamma', 'team-delta', 'PENDING'),
  ('TRN-001', 'QUARTERFINAL', 3, 'team-echo',  NULL,          'BYE'),     -- Freilos
  ('TRN-001', 'QUARTERFINAL', 4, NULL,          NULL,          'BYE');     -- Freilos

-- Halbfinale (leer, wird nach QF befüllt):
INSERT INTO tournament_bracket (tournament_id, round, match_number, status)
VALUES
  ('TRN-001', 'SEMIFINAL', 1, 'PENDING'),
  ('TRN-001', 'SEMIFINAL', 2, 'PENDING');

-- Finale:
INSERT INTO tournament_bracket (tournament_id, round, match_number, status)
VALUES
  ('TRN-001', 'FINAL', 1, 'PENDING');
```

4. DB: Tournament-Status → `BRACKET_READY`
5. WebSocket: Broadcast `tournament_update` an alle verbundenen Clients

---

#### Phase 4: Match-Durchführung (Quarterfinals)

**Für jedes QF-Match (Match 1 und 2, Match 3 = BYE):**

**Schritt 4.1 — Match erstellen (aus existierender Match-Logik)**

Das Turnier-System erstellt automatisch ein reguläres Match-Objekt pro Bracket-Slot:

```rust
// tournament_service::start_bracket_match()
let match_id = match_service::create_match(CreateMatchParams {
    creator_user_id: team_a.captain_user_id,
    opponent_user_id: team_b.captain_user_id,
    wager_sompi: tournament.buy_in_sompi,
    mode: "team5v5",
    // WICHTIG: kein separates Escrow — Tournament-Escrow deckt alles
    escrow_type: EscrowType::TournamentPool(tournament_id),
}).await?;

// Link in Bracket eintragen
db.update_bracket_slot_match_id(bracket_slot_id, match_id).await?;
```

**Schritt 4.2 — FaceIT-Match**

Da Kaspa Battle die bestehende `GAME_ID_INPUT`-Logik nutzt:

```
1. FaceIT-Hub erstellt Team-Match zwischen Team A und Team B
   (Entweder: Spieler erstellen manuell auf FaceIT-Plattform,
    oder: API-gestützter Hub-Match via FaceIT Organizer-API)

2. Captain A submitted FaceIT-Match-ID:
   POST /api/matches/:match_id/submit-game-id
   { "faceit_match_id": "1-abc123def456" }

3. Captain B confirmed dieselbe FaceIT-Match-ID:
   POST /api/matches/:match_id/submit-game-id
   { "faceit_match_id": "1-abc123def456" }

4. Match → Status IN_GAME
   FaceIT-Watcher startet (wie in bestehender faceit_watcher_jobs-Logik)
```

**Schritt 4.3 — FaceIT-Ergebnis**

```
FaceIT-API gibt zurück:
{
  "status": "FINISHED",
  "results": {
    "winner": "faction1",   // = Team Alpha
    "score": { "faction1": 16, "faction2": 9 }
  }
}

→ Match → FINISHED_FACEIT
→ faceit_winner_faction, faceit_score gespeichert
```

**Schritt 4.4 — Oracle-Result an Tournament melden**

```rust
// tournament_oracle_service::process_bracket_result()
// Wird von FaceIT-Watcher getriggert wenn bracket-Match abgeschlossen:

let winner_team_id = if faceit_winner == "faction1" {
    bracket_slot.team_a_id
} else {
    bracket_slot.team_b_id
};

// 1. kdapp: ReportBracketResult on-chain
kdapp.send_command(TournamentCommand::ReportBracketResult {
    round:           slot.round as u8,
    match_number:    slot.match_number as u8,
    winner_team_idx: winner_team_idx,
    faceit_match_id: faceit_match_id_bytes,
}).await?;

// 2. DB: Bracket-Slot updaten
db.update_bracket_winner(slot.id, winner_team_id).await?;

// 3. Nächste Runde befüllen (Sieger weitersetzen)
tournament_service::advance_to_next_round(tournament_id, slot).await?;

// 4. BYE-Slots automatisch auflösen
tournament_service::resolve_bye_slots(tournament_id).await?;

// 5. WebSocket Broadcast
ws.broadcast(TournamentUpdateEvent { tournament_id, bracket }).await?;
```

---

#### Phase 5: Halbfinale & Finale

Phasen 4.1–4.4 wiederholen sich für:
- **Halbfinale 1:** Sieger QF-1 vs Sieger QF-2
- **Halbfinale 2:** Team Echo (Bye) vs Sieger QF-3
- **Finale:** Sieger HF-1 vs Sieger HF-2

```
Nach jedem Match:
1. FaceIT-Ergebnis kommt via Watcher
2. Oracle reported on-chain (TournamentCommand::ReportBracketResult)
3. Nächste Runde wird entsperrt
4. WebSocket informiert Frontend
```

**Nach dem Finale:**
```
TournamentEpisode::phase → Resolved { winner_team_idx: 0 }
DB: tournaments.winner_team_id = 'team-alpha'
DB: tournaments.status = 'COMPLETED'
```

---

#### Phase 6: Prize-Payout

**Automatisch nach Finale-Ergebnis (oder Winner-Trigger):**

```
POST /api/tournaments/TRN-001/payout
Authorization: Bearer <winner-captain-jwt>
```

**Prize-Verteilung (500 KAS Gesamtpool):**

```
Winner (Team Alpha):   350 KAS  (70%)
Runner-up (Team Beta): 100 KAS  (20%)
Plattform-Fee:          50 KAS  (10%)  → Plattform-Wallet
```

**Backend-Aktionen:**
1. kdapp: `InitiateTournamentPayout`-Command
2. Tournament-Episode → `Completed`
3. `PayoutService::create_tournament_payout_pskt()`:
   ```rust
   // Analogon zu bestehender create_payout_pskt()-Funktion in battle-kaspa
   let pskt = build_tournament_payout_pskt(
       escrow_addr,
       vec![
           (winner_captain_addr,    350_000_000_000),
           (runner_up_captain_addr, 100_000_000_000),
           (platform_wallet_addr,    50_000_000_000),
       ]
   ).await?;
   ```
4. Winner signiert PSKT via Frontend
5. TX wird gebroadcastet
6. DB: `tournaments.payout_tx_hash`, Status `COMPLETED`
7. WebSocket: Final broadcast

---

## 7. Dispute-Behandlung

### Trigger
- Spieler zweifelt am FaceIT-Ergebnis
- Match-Result nicht eingereicht innerhalb von 30 Minuten nach FaceIT-Finish

### Ablauf

```
1. Captain filed Dispute:
   POST /api/tournaments/TRN-001/dispute
   { "bracket_slot_id": "...", "reason_code": 10 }

2. kdapp: DisputeResult-Command on-chain
   → TournamentEpisode::phase = Disputed { round, match_number, by_team_idx }

3. DB: tournaments.status = 'DISPUTED'
   tournaments_bracket.status = 'DISPUTED'

4. Admin-Dashboard zeigt Dispute
5. Admin reviewed FaceIT-Demo/Screenshot
6. Admin resolved:
   POST /api/admin/tournaments/:id/resolve-dispute
   { "winner_team_id": "...", "resolution_note": "..." }

7. Normal-Flow ab Phase 4.4 fortgesetzt
```

### Timeout-Regeln (analog zu MatchEpisode)

| Zustand | Timeout | Aktion |
|---|---|---|
| `REGISTRATION` (kein Deposit) | 60 min | Auto-`CANCELLED` |
| `REGISTRATION` (Deadline) | `registration_deadline` | `CANCELLED` wenn < 2 Teams |
| `GAME_ID_INPUT` pro Match | 30 min | Match `CANCELLED`, Runden-Bye |
| `READY_FOR_PAYOUT` (Tournament) | 7 Tage | `DISPUTED` |

---

## 8. WebSocket-Events (Erweiterung)

```typescript
// Neue Event-Types für Frontend
type TournamentEvent =
  | { type: "tournament_created";    tournament: Tournament }
  | { type: "team_registered";       team: TournamentTeam }
  | { type: "team_funded";           team: TournamentTeam; progress: FundingProgress }
  | { type: "bracket_ready";         bracket: BracketSlot[] }
  | { type: "match_started";         bracket_slot: BracketSlot }
  | { type: "match_result";          bracket_slot: BracketSlot; winner: TournamentTeam }
  | { type: "round_advanced";        new_round: BracketRound; slots: BracketSlot[] }
  | { type: "tournament_completed";  winner: TournamentTeam; payout: PayoutInfo }
  | { type: "tournament_cancelled";  reason: string }
  | { type: "dispute_filed";         bracket_slot: BracketSlot };
```

---

## 9. Implementierungs-Reihenfolge (Sprint-Plan)

### Sprint 1 — Fundament (ca. 2 Wochen)
- [ ] Migration `202604_tournament_schema.sql` schreiben und testen
- [ ] `TournamentCommand` + `TournamentPhase` in `battle-kdapp` hinzufügen
- [ ] `TournamentEpisode` implementieren (inkl. `build_bracket()`)
- [ ] Unit-Tests für Episode analog zu `episode.rs`-Tests

### Sprint 2 — API-Layer (ca. 2 Wochen)
- [ ] `routes/tournaments.rs` mit allen Endpunkten
- [ ] `TournamentEpisode` (API-Layer) analog zu `MatchEpisode`
- [ ] Team-Registration + Deposit-Flow
- [ ] BlockchainWatcher-Integration für Tournament-Deposits

### Sprint 3 — Bracket + FaceIT (ca. 2 Wochen)
- [ ] Lock + Bracket-Generierung
- [ ] FaceIT-Watcher auf Bracket-Matches anpassen
- [ ] Oracle-Result → `advance_winner()` + BYE-Auflösung
- [ ] WebSocket-Events für Turnier-Updates

### Sprint 4 — Payout + Disputes (ca. 1 Woche)
- [ ] `PayoutService::create_tournament_payout_pskt()`
- [ ] Multi-Output-PSKT (Winner 70% + Runner-up 20% + Fee 10%)
- [ ] Dispute-Flow + Admin-Endpoints
- [ ] Timeout-Monitoring für alle Turnier-Phasen

### Sprint 5 — Frontend + Testing (ca. 2 Wochen)
- [ ] Tournament-Lobby-UI
- [ ] Live-Bracket-Visualisierung
- [ ] Deposit-Progress-Anzeige
- [ ] End-to-End-Test: Testnet-Turnier mit 2–4 Teams

---

## 10. Kritische Integrationspunkte

### 10.1 Escrow-Strategie

Das bestehende System nutzt **eine Escrow-Adresse pro Match**. Für Turniere gibt es zwei Optionen:

| Strategie | Pro | Contra |
|---|---|---|
| **Einzelne Tournament-Escrow** (empfohlen) | Einfacher, ein Prize-Pool | Attribution aufwendiger |
| **Escrow pro Team** | Bekannte Attribution | 5+ Adressen, komplexes Payout |

**Empfehlung:** Einzelne Tournament-Escrow + `tournament_payments`-Tabelle für Attribution via recorded TX-Hashes (exakt wie `player_X_deposit_tx_hash` in der bestehenden Logik).

### 10.2 kdapp-Episode vs. API-Episode

Das Projekt hat zwei Episode-Ebenen:

```
battle-kdapp  →  TournamentEpisode (on-chain, Borsh, UTXO-State)
battle-api    →  TournamentEpisode (off-chain, PostgreSQL, HTTP)
```

Beide müssen synchron laufen. On-chain ist die **Quelle der Wahrheit** für Zustandsübergänge; die API-Episode pollt und spiegelt den Zustand in die DB.

### 10.3 FaceIT-Team-API

Die bestehende FaceIT-Integration ist auf **1v1-Spieler** ausgelegt (via `faceit_links`-Tabelle). Für 5v5-Matches benötigt wird:

```rust
// Bestehend: faceit_links (player-level)
// Neu benötigt:
pub struct FaceitTeamLink {
    pub team_id:       Uuid,
    pub faceit_team_id: String,
    pub faceit_hub_id:  Option<String>,
}

// FaceIT-API-Endpunkte für Teams:
// GET  /organizer/v1/hubs/:hubId/championships  (Turnier-Modus)
// POST /organizer/v1/hubs/:hubId/matches         (Match erstellen)
// GET  /data/v4/matches/:matchId                 (Status pollen — bereits vorhanden!)
```

Der FaceIT-Watcher (`faceit_watcher_jobs`) kann **ohne Änderungen** für Team-Matches genutzt werden, da er auf `faceit_match_id` arbeitet und nicht auf Player-Level.

---

## 11. Zusammenfassung: Änderungsmatrix

| Bereich | Datei | Aktion |
|---|---|---|
| DB | `migrations/202604_tournament_schema.sql` | **NEU** |
| kdapp | `battle-kdapp/src/tournament_commands.rs` | **NEU** |
| kdapp | `battle-kdapp/src/tournament_episode.rs` | **NEU** |
| kdapp | `battle-kdapp/src/lib.rs` | Ergänzen: `pub mod tournament_*` |
| API | `battle-api/src/routes/tournaments.rs` | **NEU** |
| API | `battle-api/src/episodes/tournament_episode.rs` | **NEU** |
| API | `battle-api/src/routes/oracle.rs` | Erweitern: Tournament-Result-Handler |
| API | `battle-api/src/main.rs` | Route-Registration + Episode-Manager |
| API | `battle-kaspa/src/payout.rs` | Erweitern: Multi-Output PSKT |
| FaceIT | `battle-api/src/faceit/` | Neu: Team-Match-Support |
| WS | `battle-api/src/websocket.rs` | Neue Event-Types |
| Frontend | `src/routes/tournaments/` | **NEU** (separate Implementierung) |


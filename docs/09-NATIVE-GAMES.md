# 09 – Native Browser Games (Vier Gewinnt / Connect Four)

Status: MVP, serverautoritativ, **Testnet only**. Feature Flag: `NATIVE_GAMES_ENABLED`.

---

## 1. Technische Bestandsaufnahme (vor der Implementierung)

### 1.1 Authentifizierung & FACEIT-Kopplung
* **Wallet-Challenge-Response-Login existiert bereits**: `POST /api/v1/auth/wallet-challenge` →
  `POST /api/v1/auth/wallet-verify` (Kaspa-Personal-Message-Signatur, Schnorr, einmalige Challenge, Session-Cookie
  `kaspabattle-auth`). Es wird beim ersten Login automatisch ein Wallet-User (`…@wallet.local`) angelegt.
  → *Abweichung von der Annahme „Login nur über FACEIT“*: Es muss **keine** neue Anmeldung gebaut werden.
* FACEIT ist eine optionale Verknüpfung (`faceit_links`, 1:1 zu `users`), `GET /auth/me` liefert `faceit_connected`.
* **Die FACEIT-Pflicht wird aktuell ausschließlich im Frontend erzwungen** (`isFullyConnected`, Buttons disabled).
  Das Backend (`create_challenge`, `join_challenge`) prüft nur Session + Wallet (`SessionUser`).
  → Diese Lücke wird für FACEIT-Matches serverseitig geschlossen (siehe 3.5); NATIVE-Matches brauchen kein FACEIT.
* Extractors: `SessionUser` (Session + Wallet Pflicht), `SessionUserNoWallet` (nur Session).

### 1.2 Games- / Match-Datenmodell
* Es gibt **keine `games`-Tabelle**. Spiele sind im Frontend hart kodiert (`SUPPORTED_GAMES`), `matches.game_id` ist Freitext.
* `matches`: `creator_user_id` (= Player A), `opponent_user_id` (= Player B), `status match_status` (PG-Enum),
  Deposit-/FACEIT-/Payout-/Refund-Spalten, `winner_user_id`, `loser_user_id`. `multisig_escrows` (1:1), `payments`.
* Die Domain-`MatchState`-Maschine (`battle-core/src/match_state.rs`) ist rein und wird über `Match::to_state_machine()`
  für Join/Cancel-Validierung genutzt; die DB-Statusübergänge im Episode Runner / Workern laufen direkt per SQL.

### 1.3 Match-State-Machine (Ist)
`OPEN → AWAITING_FUNDING (Join) → FUNDED (Episode Runner, on-chain Deposit ≥ 10 Confs) → GAME_ID_INPUT →
IN_GAME (beide melden dieselbe FACEIT-ID) → FINISHED_FACEIT (FACEIT-Watcher) → READY_FOR_PAYOUT (Payout Worker, PSKT)
→ RESOLVED (Gewinner triggert `payout/submit-signature`, Backend broadcastet)`.
Nebenpfade: `CANCELLED`, `DISPUTED`, `REFUNDED`. `PAID_OUT` existiert im Enum, wird vom Ist-Code **nie gesetzt**
(Endzustand ist `RESOLVED`). *Abweichung*: Die Spec nennt `PAID_OUT`; wir lassen den bestehenden gemeinsamen Endzustand
`RESOLVED` unverändert (FACEIT bleibt kompatibel) und dokumentieren das.

### 1.4 Deposit / Escrow / Refund / Payout
* Escrow: 2-von-3-P2SH-Multisig (`battle-kaspa/src/multisig`, `kaspa-txscript`), Adresse pro Match bei `create_challenge`.
* Deposits: Spieler sendet TX (Wallet im Browser), meldet Hash (`POST /matches/:id/deposit`); der Episode Runner
  attribuiert UTXOs und setzt `FUNDED`.
* Payout: `payout_worker` (`FINISHED_FACEIT` → PSKT → `READY_FOR_PAYOUT`), danach signiert der Gewinner eine
  Wallet-Message („Approve payout for match …“), das Backend signiert (Oracle + Player-Key) und broadcastet
  (`execute_payout`, 5 % Plattformgebühr, Mass-basierte Netzwerkgebühr).
* Refund: `refund_worker` (`CANCELLED`/`DISPUTED`) → `execute_refund`: **Split-Refund 50/50** abzüglich Netzwerkgebühr
  (`create_unsigned_refund_tx`). Ein sicherer Draw-Refund beider Einsätze ist damit technisch vorhanden.

### 1.5 WebSocket
Ein globaler `tokio::broadcast`-Kanal (`AppState.tx`), `GET /ws` (Limit 200 Verbindungen, Idle-Timeout 60 s).
Nachrichten sind Strings: rohe `Match`-JSON (legacy) oder typisierte Envelopes (`match_update`, `payout_broadcast`, …).
Es gibt keine Per-Connection-Authentifizierung; Events enthalten nur öffentliche Daten.

### 1.6 Betroffene Frontend-Bereiche
`CreateMatchPage` / `CreateChallengeForm`, `LobbyPage` / `LobbyTable`, `MatchPage` / `MatchDetailView` /
`MatchPlayersPanel`, `LandingPage`, `useAuthStore`, `api/types.ts`, `api/matches.ts`, `useMatchPolling` (WS),
`config/constants.ts`, `locales/{de,en}/common.json`.

### 1.7 Risiken / Blocker
| # | Risiko | Bewertung |
|---|--------|-----------|
| B1 | **Player-Keys im Escrow werden serverseitig deterministisch aus `match_id` abgeleitet** (`derive_player_key`, SHA-256 ohne Secret). Wer die (öffentliche) Match-ID kennt, kann Key A + B berechnen und (2-von-3) den Escrow ohne Oracle ausgeben. Das verletzt „Private Keys nie im Backend“ und ist **bestehendes Verhalten** (FACEIT-Flow identisch). | **Mainnet-Blocker** |
| B2 | Draw-Refund: technisch über `execute_refund` (50/50) möglich, aber teilt B1. | Testnet ok, Mainnet blockiert |
| B3 | Der Frontend-Payout-Claim (Message-Signatur des Gewinners) existiert im UI nicht (nur API). | wird für NATIVE im UI ergänzt |
| B4 | kdapp ist Alpha; Moves sind nicht on-chain signiert, der Server ist Schiedsrichter (vertrauensbasiert). | MVP-Einschränkung |

**Konsequenz:** Native Games werden auf Mainnet **nicht aktiviert**: Beim Erstellen wird abgelehnt, wenn `KASPA_NETWORK`
„mainnet“ enthält, außer `NATIVE_GAMES_ALLOW_MAINNET=true` (bewusst nicht dokumentiert als empfohlen).


### 1.8 Abweichungen von den Annahmen (ausdrücklich)
1. **Wallet-Login existiert bereits** → keine neue Anmeldung; nur die serverseitige FACEIT-Prüfung für FACEIT-Games ergänzt.
2. **Kein `games`-Katalog vorhanden** → Tabelle `games` neu (FACEIT-Slugs = bisherige `matches.game_id`). Unbekannte/deaktivierte `game_id` werden beim Erstellen jetzt abgelehnt (vorher Freitext).
3. **Endzustand ist `RESOLVED`, nicht `PAID_OUT`** (Ist-Verhalten von `payout/submit-signature`); unverändert für beide Provider.
4. **FACEIT-Pflicht war nur im Frontend** → jetzt auch im Backend (`create_challenge`, `join_challenge`). Das UI-`TEST_MODE` umgeht das nicht mehr für FACEIT-Matches.
5. **Idempotenz vor Version**: `clientNonce`-Prüfung läuft *vor* `expectedVersion`, sonst würde ein Retry eines erfolgreichen Zugs als „stale version“ abgelehnt.
6. **Draw → `REFUND_PENDING` direkt** (nicht über `FINISHED_GAME`), damit der Payout Worker nie ein Draw-Match sieht.
7. Frontend-Gebührenkonstanten (99 %/1 %) weichen vom Backend (`PLATFORM_FEE_PERCENT = 5`) ab – bestehend, nicht geändert.
8. `npm run build` führt `tsc` ohne `-b` aus (Root-tsconfig hat `files: []`), typcheckt also nichts; `tsc -p tsconfig.app.json` zeigt 19 **bereits vorhandene** Fehler in nicht angefassten Dateien.

## 2. Neuer Match-Lifecycle

```
OPEN → AWAITING_FUNDING → FUNDED
   FACEIT: → GAME_ID_INPUT → IN_GAME → FINISHED_FACEIT ┐
   NATIVE: → READY_TO_PLAY → IN_GAME → FINISHED_GAME   ├→ READY_FOR_PAYOUT → RESOLVED (PAID_OUT im Enum, ungenutzt)
   NATIVE draw:  IN_GAME → REFUND_PENDING → REFUNDED    ┘ (kein Winner-Payout)
```
* `FUNDED → READY_TO_PLAY`: Episode Runner legt die Session an (idempotent). `READY_TO_PLAY → IN_GAME`: `POST /game/start` (Clients rufen es automatisch auf; der Server startet nach `NATIVE_AUTOSTART_SECS`=20 s selbst).
* **Resignation** → Gegner gewinnt (`FINISHED_GAME`, `end_reason=RESIGNATION`). **Timeout/Abandoned**: läuft `turn_deadline` (`NATIVE_TURN_TIMEOUT_SECS`=120) ab, verliert der Spieler am Zug (`TIMEOUT`).
* **DISPUTED**: Payout Worker eskaliert `FINISHED_*` nach 24 h ohne PSKT (bestehend, gilt jetzt auch für `game_finished_at`); Refund Worker bedient `DISPUTED`.
* Ein Match kommt genau einmal in den Payout-Flow: Übergänge sind `WHERE status = …`-geschützt, Worker-Query verlangt `winner_user_id`, für NATIVE zusätzlich `result_source='NATIVE_ENGINE'` und `result_hash`.
* Ein Native-Match kann ab `READY_TO_PLAY` nicht mehr cancelled werden (State Machine).

## 3. Migrationen (additiv, `kaspabattle/migrations/`)
| Datei | Inhalt |
|---|---|
| `202609290001_games_catalog_and_provider.sql` | Tabelle `games` (+Seeds: 5 FACEIT-Games, `connect-four`), `matches.provider/requires_faceit/native_game_type` (Snapshot, Default `FACEIT` → Altbestand korrekt), `result_source`, `game_finished_at`, `result_hash`, CHECKs |
| `202609290002_match_status_native.sql` | Enum-Werte `READY_TO_PLAY`, `FINISHED_GAME`, `REFUND_PENDING` (eigene Datei, da neue Enum-Werte nicht in derselben Transaktion nutzbar sind) |
| `202609290003_native_game_sessions.sql` | `native_game_sessions` (UNIQUE match_id, Version, board_state JSONB, …), `native_game_moves` (UNIQUE(match_id, sequence_number), UNIQUE(match_id, player_id, client_nonce), column 0–6) |

## 4. API (Basis `/api/v1`; die Spec-Pfade `/api/…` entsprechen `/api/v1/…`)
| Methode/Pfad | Auth | Beschreibung |
|---|---|---|
| `GET /matches/:id/game` | optional | Vollständiger Snapshot (+`you`). 404 `game_not_started` vor der Session. **Erste Quelle nach jedem Reconnect.** |
| `POST /matches/:id/game/start` | Session+Wallet | Idempotent; nur Teilnehmer. |
| `POST /matches/:id/connect-four/moves` | Session+Wallet | `{"column":3,"expectedVersion":8,"clientNonce":"<uuid>"}` → Snapshot (`replay:true` bei wiederholter Nonce). Fehler: 400 `invalid_column`/`bad_request`, 403 `not_participant`, 409 `not_your_turn`/`version_conflict`(+`currentVersion`)/`game_over`/`wrong_status`, 422 `column_full`. |
| `POST /matches/:id/game/resign` | Session+Wallet | Gegner gewinnt. |
| `GET /features` | – | `{nativeGamesEnabled}` (Flag && !Mainnet-Sperre). |

Snapshot: `board[row][col]` mit Row 0 = unten; 0 leer, 1 blau (Ersteller, beginnt), 2 rot. Client sendet **nur** die Spalte; Zeile, Zug, Gewinner und Hash bestimmt der Server.

**WebSocket** (nach Commit): `native_game_started`, `native_game_state`, `native_game_move`, `native_game_finished` (jeweils mit vollem `state` + `version`) und `match_update`. Clients übernehmen Events nur bei höherer Version.

## 5. Betrieb / Feature Flag
* Backend: `NATIVE_GAMES_ENABLED=true` (Default aus). Steuert **Erstellen und Beitreten**; laufende Spiele bleiben spielbar, damit keine Einsätze hängen.
* Testnet only: bei `KASPA_NETWORK` mit „mainnet“ wird abgelehnt, außer `NATIVE_GAMES_ALLOW_MAINNET=true` (nicht empfohlen, siehe B1).
* Optional `NATIVE_TURN_TIMEOUT_SECS`, `NATIVE_AUTOSTART_SECS`.
* Frontend fragt `GET /features`; `VITE_NATIVE_GAMES_ENABLED=false` blendet den Bereich hart aus.
* Aktivieren: Migrationen laufen beim API-Start automatisch → Flag setzen → API neu starten.

## 6. Tests
* Domain (`battle-core`): 18 Engine-Tests (Gewinn horizontal/vertikal/beide Diagonalen, volle/ungültige Spalte, falscher/fremder Spieler, Zug nach Ende, Draw, Hash-Determinismus, Rollback normal + Gewinnzug, State-Validierung) + 4 State-Machine-Tests.
* Integration (`battle-api/src/native_tests.rs`, echtes Postgres + Mock-Kaspa-RPC + echter Axum-Router; brauchen `DATABASE_URL`, sonst Skip; CI hat Postgres): Native ohne FACEIT erstellen/beitreten, Funding→Session→Start, Spiel bis Gewinner, Payout Worker genau einmal, Draw ohne Payout + Refund (50/50), doppelte Nonce, parallele Züge, Reconnect-Snapshot vs. Audit-Log, Resign/Timeout/Autostart, DB-Constraints, Legacy-FACEIT unverändert, FACEIT ohne Link nicht erstell-/betretbar, Feature-Flag, Mainnet-Sperre, Event-Reihenfolge nach Commit.
* Frontend (Vitest): Access-Helper, Domain, Board (Maus/Tastatur/ARIA/DE), Container mit Fake-WebSocket (Snapshot zuerst, Reconnect, Versionskonflikt, stale Events), Create/Lobby/Landing/Match-Provider-Weiche.

## 7. Bekannte Einschränkungen
* **B1 (Mainnet-Blocker)**: Player-Keys des Escrows sind serverseitig aus der Match-ID ableitbar (bestehend, auch FACEIT). Payout/Refund signiert das Backend (Backend-held-keys-MVP). Vor Mainnet: Client-seitige Schlüssel + echtes PSKT-Signieren (Backend hält nur den Oracle-Key).
* Der Server ist Schiedsrichter (vertrauensbasiert); Züge sind nicht on-chain signiert.
* Draw-Refund teilt Netzwerkgebühr 50/50 ab (kein Plattform-Fee).
* Der Payout-Claim im Browser braucht die im Speicher gehaltene Recovery-Phrase (nach Reload: Wallet neu verbinden).
* Der Refund Worker teilt Beträge gleichmäßig; bei ungleichen Einzahlungen (Überzahlung) nicht deposit-genau (bestehend).

## 8. Manuelle Testanleitung (2 Browser-Sessions, Testnet)
1. Backend mit `NATIVE_GAMES_ENABLED=true`, Frontend starten. Zwei Browser-Profile (A, B), in beiden Wallet per Recovery-Phrase verbinden (**kein FACEIT**).
2. A: *Create Lobby → Play on KaspaBattle → Connect Four*, Einsatz ≥ 10 KAS, veröffentlichen, einzahlen.
3. B: Lobby → Filter *Browser Games* → Karte „BROWSER GAME · …“ → annehmen, einzahlen.
4. Nach ~10 Bestätigungen erscheint das Brett automatisch (A = blau, beginnt). Abwechselnd ziehen (Maus, Pfeiltasten+Enter, Ziffern 1–7).
5. Reload/Netz trennen in einem Tab → Brett kommt per `GET /game` zurück.
6. Vier in einer Reihe → Gewinner klickt *Claim payout*. Alternativ Aufgeben, Zeitablauf (120 s) oder Draw (Refund).

## 9. Phase 2 – echte kdapp-Episode
1. `ConnectFour` als `Episode` implementieren (`initialize/execute/rollback` sind 1:1 vorbereitet; `Command = column`, `CommandRollback = MoveRollback`, Autorisierung über `PubKey` statt User-ID).
2. Züge als signierte kdapp-Payloads (Schnorr über `{match_id, seq, column, prev_state_hash}`) on-chain; Server/Proxy wird zum bloßen Indexer, `state_hash` verknüpft die Kette.
3. Escrow-Auflösung: Oracle attestiert nur `result_hash` (existiert bereits) – oder Winner-Script mit CLTV-Timeout ohne Oracle.
4. Erst wenn kdapp stabil ist und B1 behoben: Mainnet-Sperre entfernen.

## 10. Geänderte / neue Dateien
**Neu**: `docs/09-NATIVE-GAMES.md`; Migrationen `202609290001–3`; `battle-core/src/native_games/{mod,connect_four}.rs`; `battle-api/src/{native_game,native_tests}.rs`, `battle-api/src/api/native.rs`; Frontend `api/nativeGames.ts`, `domain/{access,nativeGame}.ts`, `hooks/{useAccess,useConnectFourGame,useGameSocket,useGameLabel,useNativeGamesEnabled,usePayoutClaim}.ts`, `components/native/*`, `components/match/ProviderBadge.tsx`, 9 Testdateien.
**Geändert**: `battle-core/src/{lib,match_state}.rs`; `battle-api/{Cargo.toml,src/main.rs,models.rs,api/mod.rs,episodes/match_episode.rs,payout_worker.rs,refund_worker.rs}`; `.github/workflows/ci.yml`; Frontend `api/types.ts`, `stores/useAuthStore.ts`, `config/constants.ts`, `styles/globals.css`, `LobbyPage/LobbyTable/CreateMatchPage/CreateChallengeForm/MatchPage/LandingPage/EscrowPage/AuthGuard/MatchStatusBadge`, `locales/{en,de}/common.json`, `vite-env.d.ts`, `.env.example`.

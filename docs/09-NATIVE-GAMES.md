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

### 1.8 Datei-Liste
Siehe Abschlussbericht am Ende dieses Dokuments (wird mit der Implementierung fortgeschrieben).

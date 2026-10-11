# Code-, Architektur- und Sicherheitsaudit — 2026-10-07

**Basis:** `origin/main` @ `fe8eee6` · **Branch:** `refactor/security-architecture-audit` · **Toolchain:** rustc/cargo 1.94.1, Node 22.22.2
**Vorgänger:** [FULL_SYSTEM_AUDIT_2026-09-29.md](../FULL_SYSTEM_AUDIT_2026-09-29.md) (IDs `F-xx`). Dieser Bericht ergänzt ihn, wiederholt ihn nicht; neue Befunde tragen `A-xx`.

> **Ehrlicher Umfang.** Dieser Audit war gezielt, nicht erschöpfend, und wurde ohne Zugriff auf Produktion, Docker-Daemon, `cargo-audit`, `cargo-deny`, Trivy, hadolint oder gitleaks durchgeführt (nicht installiert; es wurde nichts global installiert). Gründlich geprüft wurde der seit dem Vorgänger-Audit neue Code (E-Mail-Konten, Free Play, Bot, WebSocket-Umbau), die Session-Speicherung, CI/Container-Konfiguration, Panic-Pfade und Dependencies. **Nicht** neu aufgerollt: Multisig/Escrow-/Payout-Logik (im Vorgänger-Audit geprüft; hier nur per Stichprobe), `battle-kdapp`, Smart-Contract-Quellen unter `contracts/`, Frontend-Sicherheit jenseits `npm audit`. „Tests grün" ist kein Sicherheitsbeweis.

## 1. Systemübersicht

Kaspa-Battle ist eine P2P-Wettplattform (Testnet) mit zwei Spielwelten:

| Modus | Domäne | Geld |
|---|---|---|
| `kaspa_testnet` | FACEIT-Matches und native Spiele (Vier gewinnt) mit 2-aus-3-Multisig-Escrow, Deposit, Oracle-signiertem Payout/Refund, Turniere | Ja (Testnet-KAS) |
| `free_play` | Vier gewinnt gegen Menschen/Bot, E-Mail-Konto, keine Wallet | **Nein** — eigene Tabellen ohne FK zu Matches/Escrow, DB-CHECKs (`stake = 0`) |

Stack: Rust-Workspace (Axum 0.7, sqlx 0.8, PostgreSQL 16, tokio), React 18/Vite/TypeScript, Docker Compose + Caddy.

## 2. Modulübersicht und Verantwortlichkeiten

| Crate | Zeilen (≈) | Verantwortung | Bewertung |
|---|---|---|---|
| `battle-core` | 9 k | Domäne: Match-State-Machine, native Spiele (`connect_four`, `connect_four_bot`), Auth-Service, FACEIT-Client/OAuth, Worker-Traits | gut getrennt, reine Domänenlogik testbar |
| `battle-kaspa` | 6 k | Kaspa-RPC, Escrow-/Multisig-Skripte, Payout | klar gekapselt hinter `PayoutProvider`-Traits |
| `battle-kdapp` | 5 k | kdapp-Engine/Episoden (WIP laut CLAUDE.md) | nicht neu bewertet |
| `battle-silverscript` | klein | SilverScript-Contract-Anbindung | nicht neu bewertet |
| `battle-api` | 14 k | HTTP/WS-Schicht, Account, Free Play, Mail, Worker-Verdrahtung | siehe A-10 (God Module `api/mod.rs`) |

## 3. Datenflüsse, Integrationen, Auth

- **Auth:** drei Wege auf *einem* `users`-Konto: Wallet-Challenge (Signatur), FACEIT-OAuth, E-Mail + Passwort (Argon2id). Session = zufälliges 256-Bit-Token in HttpOnly-Cookie (`kaspabattle-auth`), **seit A-01 nur als SHA-256 in `sessions.id`**. CSRF: Origin-Check-Layer; CORS: exakter Origin-Vergleich mit Credentials.
- **Autorisierung:** Extractors `SessionUser` (verlangt Wallet) und `SessionUserNoWallet`; Free Play verlangt nur Login. Teilnehmerprüfung pro Objekt (Match/Free-Play-Spiel) im Handler/Service.
- **Geld:** Escrow-Adresse → Deposit-Meldung (`tx_hash`) → Watcher bestätigt → Spiel/FACEIT-Ergebnis → Oracle-signierter Payout/Refund (Worker). Free Play berührt diesen Pfad strukturell nicht.
- **Extern:** FACEIT-API/OAuth/Webhook (HMAC), Kaspa-Node (wRPC; optional), SMTP (lettre).
- **Persistenz:** sqlx-Pool; Zustandsübergänge in Transaktionen mit `FOR UPDATE`, Idempotenz über `client_nonce`, optimistische Versionen (`expectedVersion`); Events erst nach Commit.

## 4. Testabdeckung

≈329 Rust-Tests (`#[test]`/`#[tokio::test]`; DB-Tests laufen nur mit `DATABASE_URL`, in CI mit Postgres 16), 35 Frontend-Testdateien (vitest) + Playwright-E2E-Skript für Free Play. Eine Coverage-Messung (`cargo llvm-cov`) wurde **nicht** ausgeführt (Tool nicht vorhanden).

## 5. Build und Deployment

CI (`ci.yml`): fmt-Check (neu, A-06), `clippy -D warnings`, `cargo test --workspace --all-features`, Frontend-Lint/Test, Docker-Build des Prod-Frontends, `docker compose config`, Security-Job (cargo-audit/npm-audit/TruffleHog, alle `continue-on-error`). `release.yml` baut/pusht das Image, Trivy-Scan, SBOM. Deployment manuell per Compose + Caddy (`DEPLOYMENT_IONOS_DOCKER.md`); Migrationen laufen beim Start (`sqlx::migrate!`).

## 6. Befunde

Status: **behoben** (Commit in diesem Branch) · **offen** · **akzeptiert**.

### A-01 — Session-Tokens im Klartext in der Datenbank (= F-07)
- **Kategorie:** Authentifizierung · **Priorität:** HIGH (im Vorgänger: Medium; angehoben, weil nun auch E-Mail-Konten mit langen „Angemeldet bleiben"-Sitzungen existieren)
- **Datei:** `battle-core/src/auth.rs`, `battle-api/src/account.rs`, `battle-api/src/api/mod.rs` (Insert/Lookup/Delete)
- **Beobachtung:** `sessions.id` = Bearer-Token. **Auswirkung:** DB-Lesezugriff (Backup, SQLi, Log) = sofortige Kontoübernahme für alle aktiven Nutzer.
- **Maßnahme:** SHA-256-Hash (Hex) wird gespeichert, Cookie-Wert vor jedem Zugriff gehasht; Migration `202610070001` konvertiert Bestand in-place (Länge 43 vs. 64 macht sie idempotent) — niemand wird ausgeloggt.
- **Verifikation:** Tests `session_tokens_are_stored_hashed_…` (Hash in DB ≠ Token; gespeicherter Hash als Cookie authentifiziert **nicht**), `hash_migration_keeps_existing_sessions_valid_and_is_idempotent` (SQL-sha256 == Rust-Hash, Re-Run ändert nichts), RFC-Testvektor. **Status:** behoben.
- **Rollback-Hinweis:** Ein Zurückrollen auf den alten Code würde gehashte Sessions ungültig machen (Re-Login); Daten gehen nicht verloren.

### A-02 — CI-Clippy rot auf neuem `stable` (14 Fehler, `double_must_use`)
- **Kategorie:** CI/Supply Chain · **Priorität:** MEDIUM · **Datei:** `battle-core/src/kaspa_backend.rs`, `workers/oracle_worker.rs` (generierter `#[async_trait]`-Code)
- **Beobachtung:** CI nutzt unpinned `stable` (1.99); PR #28 zeigte den Fehler, `main` ist damit ebenfalls rot. **Maßnahme:** begründetes crate-weites `#![allow(clippy::double_must_use)]` in `battle-core/src/lib.rs`. **Status:** behoben *(lokal mit 1.94 nicht reproduzierbar; CI-Lauf dieses PR ist der Nachweis)*. Empfehlung: Toolchain per `rust-toolchain.toml` pinnen (A-06).

### A-03 — Vergiftete Mutexe lösen Folge-Panics aus
- **Kategorie:** Rust-Robustheit · **Priorität:** MEDIUM · **Datei:** `account.rs` (`Throttle`), `free_play.rs` (`Presence`), `api/mod.rs` (WS-Subscriptions)
- **Beobachtung:** `lock().unwrap()` — ein Panic unter dem Lock hätte jeden späteren Login-Versuch bzw. jede WS-Nachricht panicen lassen (Dauer-Ausfall des Login-Pfads). **Maßnahme:** `unwrap_or_else(|e| e.into_inner())`; geschützte Daten sind reine Zähler/Mengen. **Status:** behoben.

### A-04 — `unwrap()` auf DB-Spalten in Turnier-Handlern (55 Stellen)
- **Priorität:** MEDIUM · **Datei:** `battle-api/src/api/tournament.rs`
- **Beobachtung:** dreifach duplizierter 15-Feld-Konstruktor; Schema-Drift → Handler-Panic. **Maßnahme:** ein fallibler Mapper `tournament_from_row` (Fehler → geloggter 500). **Status:** teilweise behoben (Antwort-Mapper); **offen:** ≈40 weitere `unwrap()` im selben File (Teams, Matches, Bracket) → eigener Refactor.

### A-05 — Destruktives Dev-Tool `clear_testdata`
- **Priorität:** MEDIUM · **Datei:** `battle-api/src/bin/clear_testdata.rs`
- **Beobachtung:** löscht *alle* `payments`/`matches`, Default-DB-URL fest eingebaut, keine Bestätigung; wird von `cargo build -p battle-api` mitgebaut (nicht ins Runtime-Image kopiert, aber ein versehentlicher Aufruf mit gesetztem `DATABASE_URL` wäre fatal). **Maßnahme:** `DATABASE_URL` Pflicht (kein Default) + `CONFIRM_CLEAR_TESTDATA=yes`. Keine Referenz in Docs/Skripten/CI (grep). **Status:** behoben.

### A-06 — CI: kein `cargo fmt --check`, ungepinnte Toolchain und Actions
- **Priorität:** LOW/MEDIUM · **Datei:** `.github/workflows/*.yml`
- **Beobachtung:** `dtolnay/rust-toolchain@stable`, `actions/*@v4`, `docker/*@v3/v5`, `Swatinem/rust-cache@v2`, `anchore/sbom-action@v0` per beweglichem Tag (nur Trivy/TruffleHog sind per SHA gepinnt); Security-Jobs `continue-on-error`. **Maßnahme:** `cargo fmt --check` als CI-Schritt ergänzt (Baum ist formatiert). SHA-Pinning/Toolchain-Pin *nicht umgesetzt* — SHAs lassen sich in dieser Umgebung nicht verifizieren (GitHub-Zugriff nur für dieses Repo). **Status:** teilweise behoben (fmt-Check); offen: SHA-Pinning + Dependabot für Actions, `rust-toolchain.toml`.

### A-07 — Hartkodierte Fallback-Treasury-Adresse
- **Priorität:** LOW (Adresse ist öffentlich/Testnet) · **Datei:** `battle-api/src/main.rs:~315`
- **Beobachtung:** ohne `TREASURY_MNEMONIC`/`TREASURY_ADDRESS` laufen Gebühren stillschweigend an eine im Quelltext stehende Adresse. **Maßnahme:** lauter `warn!`; Verhalten unverändert (Entscheidung offen: Fail-Closed?). **Status:** teilweise behoben → **Frage an Betreiber (Q-1)**.

### A-08 — Ungenutzte Dependencies in `battle-api`
- **Priorität:** LOW · `env_logger`, `urlencoding`, `ed25519-dalek`, `subtle` ohne Referenz (grep über `src`, Tests, `build.rs`); Workspace baut mit `--all-targets --all-features --locked` ohne sie, Lockfile-Diff entfernt nur Einträge. **Status:** behoben. **Manuell prüfen (nicht entfernt):** `battle-kaspa`: `kaspa-wallet-keys`, `wasm-bindgen`, `js-sys`, `web-sys`, `ed25519-dalek` (js-sys-Pin `=0.3.72` ist laut Vorgänger-Audit bewusst); `battle-kdapp`: `battle-core`, `kaspa-addresses`, `kaspa-txscript`, `serde`, `uuid`, `chrono`, `env_logger`, `wasm-bindgen`, `js-sys`, `web-sys` (Crate ist WIP; Makros/Feature-Gates können Nutzung verdecken).

### A-09 — Kein globaler Request-Timeout; keine CSP
- **Priorität:** MEDIUM · **Datei:** `battle-api/src/main.rs` (Router-Layer), `Caddyfile.example`
- **Beobachtung:** Body-Limit greift über Axum-Defaults/Caddy (1 MB `/api`), aber keine serverseitige Request-Zeitgrenze (langsame Clients/hängende Upstream-Aufrufe halten Tasks). CSP bewusst offen (= F-15). **Maßnahme:** nicht umgesetzt — ein pauschales Timeout könnte lange On-Chain-Handler abbrechen; erfordert Messung/Entscheidung. **Status:** offen.

### A-10 — God Module `api/mod.rs` (3 190 Zeilen) und `api/tournament.rs` (1 644)
- **Priorität:** LOW (Wartbarkeit) · Router, Auth-Cookies, Wallet-Login, Lobby, FACEIT-Submit, WebSocket in einer Datei. **Maßnahme:** nicht umgesetzt (großflächiger Umbau ohne ausreichende Handler-Tests wäre riskanter als der Nutzen); Free-Play/Account liegen bereits in eigenen Modulen. Empfehlung: WS (`ws_*`), Wallet-Login und Lobby nach `api/ws.rs`, `api/wallet_auth.rs`, `api/lobby.rs` verschieben — rein mechanisch, eigener PR. **Status:** offen.

### A-11 — Login-Throttle im Speicher
- **Priorität:** LOW · Zähler leben pro Prozess (Neustart setzt sie zurück, mehrere Instanzen teilen nichts); Map wird erst ab 20 000 Schlüsseln bereinigt (im Fenster von einer Stunde begrenzt durch Request-Rate/Caddy). **Status:** akzeptiert für Single-Instance-Betrieb; bei Skalierung auf DB/Redis umstellen.

### A-12 — Dokumentation hinkt hinterher
- **Priorität:** INFORMATIONAL · `README.md` erwähnt Free Play/E-Mail-Konten nicht; `docs/03-SECURITY.md` beschreibt Sessions nicht. **Maßnahme:** README-Abschnitt ergänzt, Sicherheitsentscheidungen in diesem Bericht. **Status:** teilweise behoben.

### Weiterhin offen aus dem Vorgänger-Audit
F-11 (**Critical**: Testdata.txt in Git-Historie eines öffentlichen Repos — Eigentümeraktion: Wallets rotieren/Historie bereinigen), F-18 (kein CHECK auf `wager_sompi`), F-19 (WS-DoS/Per-IP-Limit), F-21 (`MIN_WAGER_KAS` nicht durchgesetzt — Entscheidung), F-22, F-23, F-24, 2× moderate `react-router` 6.x (Major-Upgrade auf 7.x nötig; `npm audit` bestätigt).

## 7. Liste potenziell ungenutzter Elemente (nicht gelöscht)

~20 `#[allow(dead_code)]` (u. a. `oracle_worker.rs`, `services/faceit.rs`, `services/wallet.rs`, `episodes/mod.rs`, `multisig.rs`×3, `tournament_episode.rs`×5), `tournament_payout_worker.rs` („reserved for Phase 5"), `battle-kdapp` als Ganzes (WIP), Dependencies siehe A-08. Keine davon wurde gelöscht: dynamische Nutzung/Feature-Flags/künftige Phasen sind nicht ausschließbar.

## 8. Offene Fragen an den Betreiber

- **Q-1:** Soll der Start ohne `TREASURY_ADDRESS`/`TREASURY_MNEMONIC` auf Netzwerken ≠ Testnet fehlschlagen (Fail-Closed)? Aktuell nur Warnung.
- **Q-2:** Soll `MIN_WAGER_KAS` durchgesetzt werden (F-21)?
- **Q-3:** Ist ein pauschales Request-Timeout (z. B. 30 s, WS ausgenommen) akzeptabel, oder gibt es Handler mit längerer Laufzeit?
- **Q-4:** Ist die Kontaktadresse `info@kaspabattle.com` produktiv eingerichtet (aus `.env.docker.example` übernommen)?

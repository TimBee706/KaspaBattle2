# Full-System-Audit — 2026-09-29

**Basis:** `origin/main` @ `219e9f4` (enthält PR #25 FACEIT/TN10) · **Branch:** `audit/full-system-hardening-20260929`
**Toolchain:** rustc/cargo 1.95.0 · Node 24.15 / npm 11.12 (lokal), CI Node 22 · Docker CLI 29.4 (Daemon lokal nicht gestartet)
**Repo-Sichtbarkeit:** öffentlich (`gh repo view`) — das verschärft jeden Secret-Befund.

> **Ehrlicher Umfang.** Dieser Audit war gezielt, nicht erschöpfend. Gründlich geprüft: Auth/Sessions,
> Wallet-Challenge, Multisig-Escrow-API und -Service, Deposit-/Payout-/Webhook-Pfade, öffentliche
> Datenmodelle, Rate-Limiting, Docker/Compose/Caddy/CI, Dependencies (npm + Cargo), Secrets in der
> Git-Historie, Migrationen (Musterprüfung). **Nicht** oder nur oberflächlich geprüft: siehe
> [Nicht geprüft](#nicht-geprüft--bekannte-lücken). „Tests grün" ist kein Sicherheitsbeweis.

## Management Summary

| | |
|---|---|
| **Zustand** | Solide Grundlage (Rust-State-Machine, Signaturprüfung, Constant-Time-Vergleiche, CSRF-Origin-Check, Rate-Limits, fail-closed Secrets). Die gefundenen Lücken lagen an den *Rändern*: fehlende Bindung von Aufrufern an Objekte, nicht-atomare Read-then-Write-Muster, zu breite Datenmodelle, Infrastruktur-Defaults. |
| **Höchstes Risiko** | **F-11 (Critical, offen):** `Testdata.txt` mit sehr wahrscheinlich Seed-Phrases liegt in der Historie eines **öffentlichen** Repos. Erfordert Aktion des Eigentümers (siehe unten). |
| **Wichtigste Verbesserungen** | Escrow-Terms nicht mehr von Fremden überschreibbar (F-01) · Overflow-Check umgangen durch Riesen-Wager (F-02) · Login-Challenge atomar (F-08) · biometrisch abgeleitete FaceID-Hashes und PSKT nicht mehr öffentlich (F-09) · Backend/Vite nicht mehr auf `0.0.0.0` (F-13) · Rate-Limit nicht mehr per XFF umgehbar (F-16) · Dependencies: npm-Prod-Vulnerabilities 5 → 2, Cargo 6 → 3 (Rest unerreichbar bzw. Major-Upgrade) |
| **Verbleibend** | F-11 (Critical), F-07 (Session-Token im Klartext in der DB), F-19 (WS-DoS), 2× react-router 6.x (moderate, Major-Upgrade nötig), CSP fehlt |
| **Production-Readiness** | **Testnet: eingeschränkt ja. Mainnet: nein**, solange F-11 nicht geklärt und F-07/F-19 nicht entschieden sind. |

## Befunde

Status: **behoben** (Commit) · **offen** · **akzeptiert** (begründet). Nachweise sind reproduzierbar per Code-Lesung bzw. Test.

| ID | Schwere | Komponente | Beschreibung | Nachweis | Status |
|---|---|---|---|---|---|
| F-01 | **High** | `api/multisig.rs`, `multisig/service.rs` | `POST /multisig/create` verlangte nur *irgendeine* Session; `match_id`, Wager und Timelock kamen vom Client, und `escrows.insert` überschrieb ein bestehendes Escrow ohne Prüfung. Ein Fremder konnte Wager/Timelock eines laufenden Escrows umschreiben. | Code: `escrows.insert(match_id, escrow)` unbedingt; Test `test_create_escrow_is_idempotent_and_does_not_overwrite`, `multisig_create_is_bound_to_match_participants_and_cannot_overwrite` | behoben `2f3f2fd` |
| F-02 | **High** | `create_challenge`, `service.rs` | Wager nur auf `> 0` geprüft, obere Grenze `MAX_WAGER_KAS` war definiert, aber nie durchgesetzt. `wager * 2` überläuft in Release-Builds (Wrap → 0) und hebelt die „Escrow voll finanziert"-Prüfung aus. | Tests `wager_bounds_are_enforced`, `test_create_escrow_rejects_zero_and_overflowing_wager`, `create_challenge_rejects_out_of_range_wagers` | behoben `2f3f2fd`, `40b185e` |
| F-03 | Medium | `faceit_webhook` | HMAC beweist nur „kommt von FACEIT", nicht dass der Gewinner Teilnehmer *dieses* Matches ist oder das Match noch auszahlbar. Kein State-Machine-Check (Admin-Pfad hat ihn). Praktisch entschärft: `external_match_id` wird nirgends geschrieben, der Webhook findet kein Match. | Code; Test `webhook_winner_must_be_a_participant` | behoben `40b185e` |
| F-04 | Medium | `multisig.rs`, Webhook, `admin_resolve_match` | Nach *bereits gesendeter* On-Chain-TX wurde das DB-Status-Update mit `let _ =`/`.ok()` verworfen → veralteter Status lädt zu Doppelauszahlung ein. | Code | behoben `2f3f2fd`, `40b185e` (jetzt `error!`-Log; **kein** Retry/Outbox, siehe Restrisiko) |
| F-05 | Low | `submit_faceit_match_id` | Die FACEIT-Match-ID wird nicht gegen die FACEIT-API validiert („Watcher erkennt Fehler"). | Code-Kommentar | akzeptiert (Watcher fängt es ab; Restrisiko dokumentiert) |
| F-06 | Medium | `submit_deposit` | „Double-deposit guard" war Read-then-Write (TOCTOU); `tx_hash` unvalidiert (Junk belegt den Ein-Schuss-Slot). | Code; Test `tx_hash_must_be_64_hex_chars` | behoben `40b185e` |
| F-07 | Medium | `sessions.id`, `AuthService` | Session-Token liegt im Klartext in der DB. Ein DB-Lesezugriff (Backup, SQLi, Log) = sofortige Kontoübernahme. Tokens haben 256 Bit Entropie (gut). | Code `generate_session_token` / `INSERT INTO sessions` | **offen** — Empfehlung: SHA-256 des Tokens speichern; Migration invalidiert bestehende Sessions (Re-Login). Berührt 6+ Stellen, daher nicht im selben PR. |
| F-08 | Medium | `verify_wallet_login` | Challenge wurde per unbedingtem `UPDATE` als benutzt markiert, nachdem gelesen wurde → parallele Requests lösen dieselbe signierte Challenge mehrfach ein. | Test `wallet_challenge_is_claimed_exactly_once_under_concurrency` (16 parallele Claims) | behoben `40b185e` |
| F-09 | **High** | `models.rs::Match` | `Match` wird von *unauthentifizierten* Endpunkten (`/lobbies`, `/history`, `/matches/:id`) und per WebSocket ausgeliefert und enthielt `player_{a,b}_faceid_hash` (biometrisch abgeleitet) und `payout_pskt_hex` (Oracle-signiert). Frontend liest beides nicht. | Test `match_json_never_contains_server_side_only_fields` | behoben `c83698f` |
| F-10 | **High** | `battle-frontend` (prod deps) | `npm audit --omit=dev`: axios (10 Advisories), form-data (CRLF), react-router/@remix-run/router (Open Redirect). | `npm audit` vor/nachher | behoben `360fb12` (5 → 2). **Akzeptiert:** 2× moderate in react-router 6.x (Backslash-Open-Redirect, SSR-only `deserializeErrors`) — Fix erfordert React Router 7 (Major). |
| **F-11** | **Critical** | Git-Historie | `Testdata.txt` wurde am 2026-02-27 (`822b830`, 233 B) und 2026-03-02 (`a1df5cd`, 414 B, „Treasury and Escrow-Wallet") eingecheckt und in `ece6a46` entfernt — bleibt aber in der Historie von `origin/main` und **allen** Branches erreichbar. Repo ist öffentlich. Form-Analyse *ohne* Inhaltsausgabe: 3–4 Zeilen mit je 14–15 kurzen Wörtern (passt zu „Label + 12-Wort-Mnemonic"). CLAUDE.md nennt die Datei „enthält reale Werte". | `git log --all -- Testdata.txt`, `git cat-file -s`, awk-Formanalyse (Inhalt nie gelesen/ausgegeben) | **offen — Eigentümer-Aktion erforderlich** |
| F-12 | Medium | `.github/workflows` | Kein `permissions:`-Block; `trufflehog@main` und `trivy-action@master` (Letzteres in Job mit `packages: write`) auf beweglichen Branches; Secret-Scan mit Shallow-Checkout; Node 20 EOL. | Workflow-Dateien | behoben `cc67770`, `ecefe1d` |
| F-13 | **High** | `docker-compose.yml` | Backend `8080` und Vite-Dev-Server `5173` auf `0.0.0.0` veröffentlicht, obwohl die Doku „nur intern" sagt. Docker-Ports umgehen `ufw`. Klartext-HTTP-API + Dev-Server aus dem Internet erreichbar; zusammen mit F-16 Rate-Limit-Umgehung. | `docker compose config` vorher/nachher (`host_ip: 127.0.0.1`) | behoben `881d386` |
| F-14 | Medium | `battle-frontend/Dockerfile`, `.dockerignore` | `npm install` statt `npm ci` (Lockfile ignoriert), `serve` ungepinnt, Node 20 (EOL), kein `.env*`-Ausschluss im Build-Kontext (Repo-Root). | Dockerfile | behoben `881d386` (Image-Build lokal **nicht** ausgeführt — Daemon aus; CI-Job `frontend-image` deckt es ab) |
| F-15 | Low | `Caddyfile.example` | Keine Security-Header, kein Body-Limit. | Datei | behoben `7a3925c` (HSTS, nosniff, X-Frame-Options, Referrer-Policy, Permissions-Policy, 1 MB `/api`). **Offen:** CSP. `caddy validate` nicht ausgeführt. |
| F-16 | Medium | `rate_limit.rs` | Mit `TRUST_X_FORWARDED_FOR=true` zählte der *erste* XFF-Eintrag (client-kontrolliert) → Limit-Umgehung + unbegrenztes Key-Wachstum. Variable war in keiner `.env`-Vorlage/Doku; ohne sie teilen sich hinter Caddy **alle Nutzer einen Bucket** (z. B. 3 Match-Erstellungen/min für die ganze Seite). | Tests `xff_uses_the_proxy_appended_last_entry`, `distinct_ips_get_distinct_buckets` | behoben `db9a3b9` |
| F-17 | High | `Cargo.lock` | `cargo audit`: 6 Vulnerabilities. | `cargo audit` vor/nachher | 3 behoben `ce17d57` (h2, rustls/webpki, crossbeam-epoch). **Akzeptiert (unerreichbar):** quinn-proto ×2, rsa — nur im Lockfile, `cargo tree --target all --all-features -i <crate>` leer. quinn-Fix blockiert durch `js-sys = "=0.3.72"`-Pin. |
| F-18 | Low | Schema | Kein DB-CHECK auf `matches.wager_sompi`. Ein nachträglicher Constraint könnte auf unbekannten Bestandsdaten Updates brechen. | Migrationen | **offen** (Empfehlung: `NOT VALID`-Constraint nach Prüfung der Prod-Daten) |
| F-19 | Low | `ws_handler` | WebSocket unauthentifiziert; globales Limit 200 ohne Per-IP-Limit → ein Client kann alle Slots belegen; Check-then-increment nicht atomar. Daten sind seit F-09 öffentlich unkritisch. | Code | **offen** |
| F-20 | Info | Repo | Verwaiste/Artefakt-Dateien. | Referenzanalyse unten | behoben `cf3bdcf`, `99cd0cc` |
| F-21 | Info | `battle-core/types.rs` | `MIN_WAGER_KAS = 10` ist definiert, wird nirgends durchgesetzt. Durchsetzen würde Verhalten ändern → **Entscheidung nötig**. | `git grep` | offen (Frage an Timo) |
| F-22 | Info | CI | `cargo audit`/`npm audit`/TruffleHog laufen mit `continue-on-error`; kein `cargo fmt --check` (Branch `style/cargo-fmt` wartet auf Merge). | ci.yml | offen |
| F-23 | Low | `verify_wallet_login` | Info-Level-Logs enthalten volle Wallet-Adresse und Challenge-ID. (Adressen sind on-chain öffentlich, aber Korrelation mit Nutzern/IP.) | Code | offen |
| F-24 | Info | Compose | `FRONTEND_BUILD_TARGET` defaultet auf `dev` (Vite-Dev-Server). Vergessenes `prod` = Dev-Server in Produktion. Seit F-13 nur noch loopback. | Compose | offen (Doku weist darauf hin) |

## Entfernte Dateien (Dead-Code-Nachweis)

| Datei | Zweck | Referenzanalyse | Ergebnis |
|---|---|---|---|
| `scratch.rs` (Root) | loses serde-Snippet | `git grep` über gesamten Baum inkl. Docs/Docker/CI: 0 Treffer; kein `Cargo.toml` im Root | entfernt `99cd0cc` |
| `kaspabattle/reset_migrations.rs` | Ad-hoc-Skript (destruktiv, hartkodierte lokale DB-URL) | 0 Treffer; kein `[[bin]]`, nicht unter `src/bin/` | entfernt `99cd0cc` |
| `battle-api/src/routes/auth.rs` | actix-web-Handler | kein `mod routes` (Modulbaum), `actix-web` keine Dependency → wurde nie kompiliert; live-Auth sind die Axum-Handler in `src/api` | entfernt `99cd0cc` |
| `kaspabattle/target/.rustc_info.json`, `kaspabattle/compile_err.txt` | Build-Artefakte | beide bereits in `.gitignore` (`**/target`, `compile_err.txt`), vor der Regel eingecheckt | aus dem Index entfernt `cf3bdcf` (Dateien bleiben lokal) |

Nach jeder Entfernung: `cargo check --workspace --all-targets --all-features` sauber. Dynamische Nutzung/Feature-Flags/Migrationen/CI: nicht betroffen (kein Treffer in `.github`, Dockerfiles, Skripten, Docs).

**Beibehalten (verdächtig, aber nicht bewiesen tot):** ~20 `#[allow(dead_code)]` (u. a. `oracle_worker.rs` mit `#![allow(dead_code, unused_imports, unused_variables)]`, `services/faceit.rs`, `services/wallet.rs`, `episodes/mod.rs`), `tournament_payout_worker.rs` (`EscrowService` „reserved for Phase 5"), `battle-kdapp` (WIP laut CLAUDE.md), `start.sh`/`start.ps1` (dokumentierte Deployment-Helfer).

## Tests

| | vorher | nachher |
|---|---|---|
| Rust (`cargo test --workspace --all-features`) | ≈260 passed (Summe der Läufe auf `main`) | **276 passed**, 0 failed (+16) |
| Frontend (vitest) | 103 (26 Dateien) | 103 (26 Dateien) — keine neuen Frontend-Tests |
| Entfernte Tests | – | **0** |
| Ersetzte Tests | – | 0 |

Neue Tests (13 reine + 3 DB-gestützte): `authorize_escrow_request` (3), Service-Idempotenz/Overflow (2), `wager_within_max`, `is_valid_tx_hash`, `is_match_participant`, `mask_id`, `Match`-JSON-Ausschluss, `client_ip_from_xff` (2), Bucket-Trennung; **DB-gestützt** (`native_tests.rs`): parallele Challenge-Claims, `multisig/create`-Autorisierung, Wager-Grenzen.

> ⚠️ **Die 3 DB-Tests wurden lokal nicht ausgeführt.** Sie folgen dem bestehenden Harness (`DATABASE_URL`, sonst früher Return) und zählen lokal als „passed", ohne etwas zu prüfen. Kein Postgres/Docker-Daemon verfügbar. Schema-Spalten wurden statisch gegen die Migrationen geprüft. **Die CI (Postgres 16) führt sie aus — das Ergebnis dieses PR-Laufs ist der Nachweis.**

`cargo clippy --workspace --all-targets --all-features -- -D warnings` war auf `main` **nicht** sauber (3 Fehler in `native_tests.rs`/`tournament_payout_worker.rs`, nur mit `--all-targets` sichtbar) → behoben, CI prüft jetzt `--all-targets`.

## Verifikation

| Befehl | Ergebnis |
|---|---|
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | ✅ sauber |
| `cargo test --workspace --all-features` | ✅ 276 passed, 0 failed (3 DB-Tests lokal ohne Wirkung, s. o.) |
| `cargo check --workspace --all-targets --all-features` | ✅ |
| `cargo audit` | 6 → 3 (alle 3 unerreichbar, s. F-17); 14 Warnungen (unmaintained/unsound, transitiv) |
| `npx tsc --noEmit`, `npm run lint`, `npm run test`, `npm run build` (inkl. `i18n:check`) | ✅ |
| `npm audit --omit=dev` | 5 → 2 moderate |
| `docker compose config` (mit Stand-in `.env.production`) | ✅ |
| Workflow-YAML (`js-yaml`) | ✅ parst |
| `cargo fmt --check` | ❌ auf `main` (repo-weit; Fix liegt in Branch `style/cargo-fmt`, bewusst **nicht** vermischt) |
| Docker-Image-Builds, `caddy validate`, Migrationen gegen echte DB, Playwright-E2E, Coverage | ⛔ **nicht ausgeführt** (Daemon/Postgres/Caddy lokal nicht verfügbar; keine E2E-Suite im Repo) |

## Nicht geprüft / bekannte Lücken

- Manuelle Tiefenprüfung von `api/tournament.rs` (~1.6k Zeilen), `admin_tournament.rs`, `battle-kdapp`, Worker-Concurrency (Locks über `.await`, Retry-Stürme), `refund_worker`/`payout_worker`-Idempotenz.
- Frontend: `kaspa/wallet.ts`, `kaspa/rpc.ts`, WebSocket-Reconnect, Route-Guards (nur Greps: kein `dangerouslySetInnerHTML`/`eval`, Storage enthält keine Tokens/Mnemonics, `persist` speichert nur Anzeigedaten).
- Census der `unwrap()/expect()` in **Produktionscode** (Grep zählt Test- und Prod-Code gemeinsam; Spitzenreiter Prod: `multisig/service.rs`, `oracle.rs`, `wallet.rs`, `main.rs`).
- Tools nicht eingesetzt: `cargo deny/udeps/machete`, `knip/depcheck`, Mutation-Testing, Fuzzing, Property-Tests.
- Secret-Scan der Historie: Regex-basiert (kein gitleaks/trufflehog lokal). Keine Private-Key-Blöcke/xprv gefunden; Treffer bei Env-Zuweisungen waren Platzhalter (`CHANGE_ME_*`, `your_*`). **F-11 ist davon unabhängig gefunden worden.**
- Live-Server: nichts geprüft oder geändert (Node-Server-Preflight siehe separater Arbeitsstrang; OpenClaw-Installationen unberührt).

## F-11 — was jetzt zu tun ist (Eigentümer-Aktion)

1. **Annehmen, dass die Wallets kompromittiert sind.** Prüfen, ob eine der betroffenen Mnemonics jemals **Mainnet-Guthaben** hielt oder ob deren Adressen (Treasury/Escrow) noch in Verwendung sind (`TREASURY_MNEMONIC`, `KASPA_MNEMONIC` auf dem Server ≠ die alten Werte?). Wenn ja: Funds sofort verschieben, Wallets ersetzen.
2. Reine Testnet-Wallets ohne weiteren Nutzen: Risiko gering, aber **nie wiederverwenden** (auch nicht in Tests).
3. **Historien-Bereinigung** (`git filter-repo`/BFG + Force-Push aller Branches) ist eine destruktive, öffentliche Aktion und wurde **nicht** durchgeführt. Wichtig: Selbst nach einer Bereinigung bleiben Forks/Klone/GitHub-Caches; Rotation ist die eigentliche Maßnahme, Bereinigung nur Hygiene.
4. Bei GitHub prüfen: Secret-Scanning/Push-Protection für das Repo aktivieren.

## Deployment (bei Merge)

1. **Vor** dem Deploy `TRUST_X_FORWARDED_FOR=true` in `.env.production` setzen (sonst bleibt der gemeinsame Rate-Limit-Bucket bestehen).
2. Compose bindet 8080/5173 jetzt an Loopback: Falls irgendetwas außerhalb von Caddy direkt `http://<server>:8080` nutzt (Monitoring, Skripte), bricht das — vorher prüfen.
3. Frontend-Image ändert sich (Node 22, `npm ci`); erster Build zeigt, ob Lockfile konsistent ist.
4. Caddy: neues `Caddyfile` aus `Caddyfile.example` ableiten und mit `docker run --rm -v $PWD/Caddyfile:/etc/caddy/Caddyfile caddy:2.7-alpine caddy validate --config /etc/caddy/Caddyfile` prüfen, **bevor** neu geladen wird. Caddy ausschließlich für die KaspaBattle-Domain ändern — OpenClaw-Routen nicht anfassen.
5. Keine Migrationen in diesem PR. Rollback = vorheriger Commit; kein Datenmodell betroffen (`Match`-JSON verliert 3 Felder — alte Clients lesen sie nicht).
6. Nach Deploy beobachten: 401/403 auf `POST /multisig/create` (erwartet für Nicht-Teilnehmer), 429-Rate (jetzt pro echter Client-IP), `TX broadcast but marking match … FAILED`-Logs (sollten nie auftreten).

## Restrisiken

- **F-04:** Ein fehlgeschlagenes Status-Update nach Broadcast wird jetzt laut geloggt, aber nicht automatisch nachgeholt (kein Outbox/Retry). Die Auszahlungs-Worker (Idempotenz) wurden in diesem Audit nicht vertieft geprüft.
- **F-07:** Session-Token im Klartext in der DB.
- **Unbekannter Prod-Stand:** Konfiguration/Daten auf dem Server wurden nicht geprüft.

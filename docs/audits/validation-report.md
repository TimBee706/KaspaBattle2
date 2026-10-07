# Validation report — branch `refactor/security-architecture-audit`

**Datum:** 2026-10-07 · **Basis:** `origin/main` @ `fe8eee6` · **Toolchain:** rustc/cargo 1.94.1, Node 22.22.2, npm (lokal), PostgreSQL 16 (lokal), Docker CLI (Daemon nicht genutzt)

## Ausgeführt

| Kommando | Ergebnis |
|---|---|
| `cargo fmt --all -- --check` | ✅ sauber |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | ✅ sauber (rustc 1.94). **Nicht** mit dem neueren CI-`stable` (1.99) reproduziert, siehe A-02 |
| `cargo test --workspace --all-features` (mit `DATABASE_URL`, Postgres 16) | ✅ 331 passed, 0 failed (davor auf `main`: 329 inkl. 1 ignorierten) — neu: 2 DB-Tests (Session-Hash, Migration) + 1 Unit-Test (SHA-256-Vektor) |
| `cargo build --release -p battle-api --locked` | ✅ |
| `cargo check --workspace --all-targets --all-features --locked` (nach Entfernen der 4 Dependencies) | ✅ |
| Migrationen auf leerer DB (alle Tests legen pro Test eine frische DB an) | ✅ inkl. neuer `202610070001` |
| Migration `202610070001` auf Bestandsdaten (Test `hash_migration_keeps_existing_sessions_valid_and_is_idempotent`) | ✅ Sessions bleiben gültig, Re-Run ändert nichts |
| `npm run lint` / `npm run i18n:check` / `npx tsc` / `npx vitest run` / `npm run build` (battle-frontend) | ✅ 27 Dateien, 106 Tests |
| `docker compose config --quiet` (mit Platzhalter-`.env.production` und `POSTGRES_PASSWORD`, wie in CI) | ✅ |
| `python -c yaml.safe_load` auf `ci.yml` | ✅ |
| Secret-Scan (Regex über getrackte Dateien: Private-Key-Header, AKIA, ghp_, xox, lange Hex-Secrets, Mnemonic-Zuweisungen) | ✅ keine Treffer; nur öffentliche Testnet-Adressen in Tests/Fallback (A-07). Getrackte `.env*`: nur `*.example` |
| `npm audit --omit=dev` | ⚠️ 2 moderate (`react-router` 6.x; Fix = Major-Upgrade auf 7.x, bekannt aus F-10) |

## Nicht möglich / nicht ausgeführt

| Prüfung | Grund | Befehl für den Betreiber |
|---|---|---|
| `cargo audit` | Tool nicht installiert (keine globalen Installationen) | `cargo install cargo-audit --locked && cargo audit` (läuft in CI als advisory) |
| `cargo deny check` | Tool nicht installiert, keine `deny.toml` im Repo | siehe oben; Konfiguration wäre eine eigene Entscheidung |
| Trivy / Container-Scan, hadolint | nicht installiert, Docker-Daemon nicht genutzt | `trivy image …` (läuft in `release.yml`) |
| Docker-Image-Build, Caddy-Validierung | kein Daemon/`caddy` | CI-Job `frontend-image`; `caddy validate --config Caddyfile` |
| Gitleaks/TruffleHog über die Git-Historie | nicht installiert | CI-Job `security` (TruffleHog, advisory) |
| Coverage (`cargo llvm-cov`) | nicht installiert | — |
| Clippy mit CI-`stable` 1.99 | älteres Toolchain lokal | der CI-Lauf dieses PR |
| Playwright-E2E | lief im vorigen Auftrag gegen Free Play (grün); in diesem Branch nicht wiederholt — Änderungen betreffen nur Session-Speicherung (per Tests abgedeckt) | `node e2e/free-play.e2e.mjs`, siehe `development-quality-gates.md` |
| Prüfung gegen Produktionsdaten | kein Zugang | Backup vor Deploy (Migration schreibt `sessions`) |

## Bekannte Einschränkungen / verbleibende Risiken

Siehe `code-security-architecture-audit.md`, Abschnitt „Weiterhin offen" und A-04/A-06/A-09/A-10/A-11. Besonders: F-11 (Secrets in Git-Historie, Eigentümeraktion), kein Request-Timeout, God Module `api/mod.rs`, Actions nicht per SHA gepinnt.

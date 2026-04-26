# KaspaBattle2 Comprehensive Code Review (2026-04-26)

## Scope
- Repository-wide static review across Rust backend/workspace, frontend, Docker/Compose, docs, and root-level ops files.
- Security-focused review plus hygiene, architecture, consistency, and documentation checks.

---

## Findings

### 1) Critical — Build artifacts are committed (`target/` in git)
- **Category:** Repository Hygiene & Unnecessary Files
- **File & Line:** `kaspabattle/target/CACHEDIR.TAG:1`
- **Description:** Rust build output is tracked in the repository (`kaspabattle/target/**`). This bloats history, slows clones, and can accidentally leak compiled/debug artifacts.
- **Recommendation:** Remove tracked `kaspabattle/target/**` from git history/index and enforce ignore rules (`git rm -r --cached kaspabattle/target`).

### 2) High — Sensitive mnemonic data is logged in plaintext diagnostics
- **Category:** Security Review (Secrets & Credentials / Blockchain Security)
- **File & Line:** `kaspabattle/battle-api/src/main.rs:232-249`
- **Description:** The startup diagnostic logs mnemonic metadata and individual words of `KASPA_MNEMONIC` (`tracing::warn!("word[i] = ...")`). This is secret material and should never be logged.
- **Recommendation:** Remove mnemonic logging entirely; if needed, log only redacted presence checks and a non-reversible fingerprint.

### 3) High — Deposit submission endpoint trusts client-supplied player role
- **Category:** Security Review (Authentication & Authorization / Input Validation)
- **File & Line:** `kaspabattle/battle-api/src/api/mod.rs:1012-1058`
- **Description:** `submit_deposit` updates A/B deposit columns from client `player_role` without binding role to authenticated user identity (creator/opponent). A user can submit as the other player.
- **Recommendation:** Derive role server-side from `SessionUser` + match participants; reject mismatches, ignore client role for authority decisions.

### 4) High — Payout signature endpoint explicitly does not verify signature
- **Category:** Security Review (Blockchain Security / Authentication & Authorization)
- **File & Line:** `kaspabattle/battle-api/src/api/mod.rs:2147-2151`
- **Description:** Endpoint comment confirms `signature_hex` is not verified in MVP. This weakens payout integrity guarantees.
- **Recommendation:** Verify winner signature cryptographically against expected message and on-chain/public key identity before payout execution.

### 5) High — CSRF webhook bypass path mismatch likely blocks FACEIT webhooks
- **Category:** Security Review (FACEIT Webhook Verification / Availability)
- **File & Line:** `kaspabattle/battle-api/src/api/csrf_guard.rs:64-67`, `kaspabattle/battle-api/src/api/mod.rs:124`
- **Description:** CSRF skip checks `/api/v1/webhooks/...` (plural), but route is `/api/v1/webhook/faceit` (singular). Real webhooks may fail CSRF due missing Origin/Referer.
- **Recommendation:** Align path checks (`/api/v1/webhook/`) or use route-level middleware exclusion.

### 6) Medium — No explicit wager lower-bound validation in API or DB schema
- **Category:** Security Review (Blockchain amount validation)
- **File & Line:** `kaspabattle/battle-api/src/api/mod.rs:824-849`, `kaspabattle/migrations/202602280000_init_pg.sql:66-77`
- **Description:** `wager_sompi` is accepted as `i64` and inserted directly; base schema has no `CHECK (stake_kas > 0)` / equivalent safeguard.
- **Recommendation:** Enforce minimum/maximum wager constraints both in request validation and DB-level CHECK constraints.

### 7) Medium — Rate limiter trusts `X-Forwarded-For` from untrusted clients
- **Category:** Security Review (Rate limiting & DoS)
- **File & Line:** `kaspabattle/battle-api/src/api/rate_limit.rs:83-95`
- **Description:** Middleware prioritizes `X-Forwarded-For` without trusted proxy validation, enabling IP spoofing and rate-limit bypass.
- **Recommendation:** Only trust forwarding headers from known reverse proxies (or rely on socket IP internally).

### 8) Medium — Development compose publishes DB with default credentials
- **Category:** DevOps & Infrastructure / Security Review
- **File & Line:** `docker-compose.yml:8-11`, `docker-compose.yml:55`
- **Description:** Postgres is exposed on host port 5432 with `postgres/postgres` credentials.
- **Recommendation:** Keep for local-only profile or override in `.env`; avoid publishing DB port by default.

### 9) Medium — Mixed/legacy server stack artifacts create maintenance risk
- **Category:** Architecture & Design Patterns / Consistency
- **File & Line:** `kaspabattle/battle-api/src/routes/oracle.rs:1-59`
- **Description:** Legacy Actix route file exists alongside Axum API architecture and includes non-English comments/messages. This suggests dead or divergent code paths.
- **Recommendation:** Remove dead modules or fully migrate and wire consistently in one HTTP framework.

### 10) Low — `.gitignore` is incomplete for current tracked artifacts
- **Category:** Repository Hygiene & Unnecessary Files
- **File & Line:** `.gitignore:1-5`, `.gitignore:89-93`, `battle-frontend/vite.config.ts.timestamp-1772343211572-2ea89150bd0d.mjs:1-5`
- **Description:** Despite ignore patterns, compiled artifacts are tracked and generated timestamped Vite build/transpile file is committed.
- **Recommendation:** Add explicit ignore for `battle-frontend/*.timestamp-*.mjs`, untrack accidental files, and run pre-commit checks.

### 11) Low — Empty root and backend `package-lock.json` files appear as leftovers
- **Category:** Repository Hygiene & Unnecessary Files
- **File & Line:** `package-lock.json:1-6`, `kaspabattle/battle-api/package-lock.json:1-6`
- **Description:** Both lockfiles are effectively empty and unrelated to Rust backend workflows.
- **Recommendation:** Remove unless intentionally required by CI tooling.

### 12) Low — Linux/Docker-first repo has Windows-only orchestration script without parity docs
- **Category:** Repository Hygiene / DevOps
- **File & Line:** `start.ps1:1-33`, `start.ps1:196-216`
- **Description:** `start.ps1` is useful but OS-specific; no equivalent shell script in repo root.
- **Recommendation:** Document it clearly as optional Windows helper and add parity `start.sh` (or Makefile) for cross-platform onboarding.

### 13) Low — Non-English comments and user-facing strings reduce maintainability
- **Category:** Code Comments & Documentation (English-only)
- **File & Line:** `.env.docker.example:19-20`, `kaspabattle/battle-core/src/secret_provider.rs:116-121`, `kaspabattle/battle-api/src/routes/oracle.rs:27-33`
- **Description:** German comments/messages are present in production code and templates.
- **Recommendation:** Standardize on English for code comments and operational messages.

### 14) Low — Outdated documentation variables drift from implementation
- **Category:** Consistency (Configuration / Frontend-Backend contract)
- **File & Line:** `docs/08-CONTRIBUTING.md:69-70`
- **Description:** Docs mention `ESCROW_MNEMONIC` and `SESSION_SECRET`, while backend currently uses `KASPA_MNEMONIC`, `TREASURY_MNEMONIC`, etc.
- **Recommendation:** Update docs to match actual runtime configuration and deprecate old names.

### 15) Info — Dependency vulnerability audit could not be completed in current environment
- **Category:** Security Review (Dependency Vulnerabilities)
- **File & Line:** `kaspabattle/Cargo.toml`, `battle-frontend/package.json`
- **Description:** `cargo audit` is not installed, and `npm audit` returned 403 from npm advisory API in this environment.
- **Recommendation:** Add CI jobs for `cargo-audit` + `npm audit` (or `pnpm audit`) with authenticated registry access.

---

## TODO / FIXME / HACK / XXX Inventory
- `kaspabattle/battle-kaspa/src/escrow.rs:34` — TODO(F-001) off-chain custody model replacement.
- `kaspabattle/battle-kaspa/src/oracle.rs:34` — TODO(F-002) multi-oracle consensus before production.
- `kaspabattle/battle-core/src/workers/oracle_worker.rs:10` — TODO(cleanup) delete file after watcher parity confirmation.
- `kaspabattle/battle-api/src/api/faceit.rs:438` and `:501` — TODO(R-01) persist/return cached games list.

---

## Summary Table (grouped by severity)

### Critical
1. Build artifacts committed (`target/`).

### High
1. Mnemonic secret logging.
2. Deposit role authorization gap.
3. Payout signature not verified.
4. CSRF/webhook path mismatch.

### Medium
1. Missing wager bounds validation.
2. `X-Forwarded-For` spoofable rate-limit key.
3. Dev compose DB exposure + default creds.
4. Legacy mixed HTTP-stack artifact risk.

### Low
1. `.gitignore` incompleteness for generated files.
2. Empty leftover lockfiles.
3. Windows-only orchestration without parity docs.
4. Non-English comments/messages.
5. Config docs drift.

### Info
1. Vulnerability scanning tooling/environment incomplete.

---

## Top 5 Priority Actions
1. **Immediately remove mnemonic logging** and rotate all secrets that may have been exposed via logs.
2. **Fix `submit_deposit` authorization** by deriving player role from authenticated user and match ownership.
3. **Enforce payout signature verification** before any broadcast/update path.
4. **Fix webhook CSRF exclusion path** to avoid blocking legitimate FACEIT webhook traffic.
5. **Purge tracked build artifacts (`target/`)** and enforce tighter ignore/pre-commit hygiene.

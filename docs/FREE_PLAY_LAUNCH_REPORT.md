# Free Play – Launch Report

Status: **code complete and verified locally; NOT deployed.** Production deployment was not possible
from the development session (no SSH access, no deploy secrets, no SMTP credentials available). Production
and the OpenClaw installation were not touched.

## Git
- Branch: `feat/free-play-email-auth-connect-four` (pushed, no force-push, `main` untouched).
- Commits: `feat(auth)` e-mail identities + mailer, `feat(free-play)` backend/engine/bot/PvP,
  `feat(ui)` onboarding/lobby/game/notice, `fix(auth)` rehash + clippy/fmt, `test(e2e)`, `docs`.

## What was built
- **Modes**: `free_play` lives in separate tables (`free_play_*`) with no FK to matches/escrow/payments and
  DB CHECK constraints (stake = 0, mode fixed). `matches.game_mode` is constrained to `kaspa_testnet`.
  Manipulated API requests cannot create stake/deposit/payout in Free Play (covered by tests).
  Wallet / FACEIT / tournament / escrow code is unchanged.
- **Accounts**: username + e-mail + password on the existing `users` table (`has_password_login`), Argon2id
  (m=19456, t=2, p=1, random salt, PHC, automatic rehash), case-insensitive unique e-mail/username,
  username policy, password >= 12 chars with common-password / personal-data checks, generic login errors,
  timing equalisation, sliding-window rate limits + progressive delay, honeypot, hashed single-use expiring
  verification/reset tokens (reset revokes sessions), session cookies HttpOnly/SameSite with rotation,
  newsletter opt-in (default off, consent timestamp/version/source, revocable), audit log without secrets.
- **Free Play game**: lobbies (human / bot easy-medium-hard), authoritative Connect Four engine (shared with
  native games), optimistic versioning + idempotent `clientNonce`, turn timeout / disconnect rules, rematch,
  WebSocket with per-connection authorization, sequence numbers, resync, presence, history + server stats.
- **Bot**: deterministic for a seed, time-budgeted, runs off the async executor.
- **UI**: register / login / forgot / reset / verify / account pages, lobby, game page, landing notice
  (public beta, node help, `mailto:` from `VITE_PUBLIC_CONTACT_EMAIL`, default `info@kaspabattle.com`),
  uniform "Kaspa-Testnet-Modus – demnächst verfügbar" status on wallet/FACEIT/testnet pages. Kaspa WASM no
  longer gates the app, so Free Play works with a dead node.

## Verification (local, Postgres 16)
| Gate | Result |
|---|---|
| `cargo fmt --all -- --check` | pass |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | pass |
| `cargo test --workspace` (with DATABASE_URL) | pass (battle-api 91, battle-core 100, …) |
| `cargo build --release -p battle-api` | pass |
| Migrations on empty DB (release binary) + idempotent re-run test | pass |
| Frontend `tsc`, `eslint`, `i18n:check`, `vitest` (all files), `vite build` | pass |
| Playwright E2E (`battle-frontend/e2e/free-play.e2e.mjs`): landing notice, register, login, reload keeps session, PvP in two browser contexts incl. win/loss and reload, bot game, no wallet/FACEIT/escrow API call, unreachable Kaspa node | pass |
| Dependency audit | npm: 2 moderate (`react-router`, pre-existing, not changed here); `cargo audit` not installed |
| Secret scan (regex over diff) | no findings |
| `docker compose config` | not validated: requires production env values (POSTGRES_PASSWORD) |
| Caddy validation | `caddy` not installed locally; Caddyfile not changed |
Not tested against a *prior production schema dump* (none available here); the new migrations are additive
and idempotent.

## Deployment – BLOCKED
Not performed: production backup, migration, deploy of the Kaspa-Battle services, healthchecks, production
smoke test, OpenClaw before/after read-only check, rollback.
- Blocker: no server access in this session (missing: SSH host/user/key and deploy credentials; names unknown
  and deliberately not guessed).
- Production state: unchanged. OpenClaw: untouched.
- Next safe steps (operator):
  1. `pg_dump` backup of the Kaspa-Battle database; record current image tags (rollback target).
  2. Merge the PR, build and deploy **only** the `backend` and `frontend` services of this compose project
     (`docker compose up -d --no-deps backend frontend`); never `system prune`; do not touch other projects.
  3. Migrations run on backend start; watch logs for `DB migrations applied`.
  4. Smoke test: landing notice + mailto, register/login, bot game, two-account PvP, `/api/v1/features`.
  5. Rollback if any step fails: redeploy previous image tags (migrations are additive, DB restore not needed).

## Open items
- **SMTP**: set `SMTP_HOST/PORT/USERNAME/PASSWORD/FROM_EMAIL/FROM_NAME/TLS_MODE` and `PUBLIC_APP_URL` in
  `.env.production`, then `REQUIRE_EMAIL_VERIFICATION=true`. Until SMTP works, run with
  `REQUIRE_EMAIL_VERIFICATION=false` (temporary risk: unverified addresses; profile shows
  "E-Mail noch nicht bestätigt"; reset mails cannot be delivered and the UI does not claim they were).
- **DNS**: SPF, DKIM, DMARC for the sending domain (manual).
- Verify the contact mailbox `info@kaspabattle.com` exists (taken from `.env.docker.example`; production
  config was not readable).
- Reliable Kaspa testnet node for `kaspa_testnet` mode (as announced on the landing page).
- Set `AUDIT_IP_SALT` to a random value in production.

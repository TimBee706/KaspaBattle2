# Development quality gates

Run these before opening a PR. They mirror `.github/workflows/ci.yml`; where CI is looser or stricter the difference is noted.

## Rust (`kaspabattle/`)

```bash
cargo fmt --all -- --check                                          # enforced in CI
cargo clippy --workspace --all-targets --all-features -- -D warnings # CI uses the moving `stable` toolchain
cargo test --workspace --all-features
cargo build --release -p battle-api
```

Database-backed tests (`account_tests`, `free_play_tests`, `native_tests`, migration tests) need Postgres 16 and skip themselves with a notice when `DATABASE_URL` is unset — a green run **without** it did not exercise them:

```bash
export DATABASE_URL=postgres://postgres:postgres@localhost:5432/postgres   # throw-away databases are created per test
```

Optional, not installed by default: `cargo audit`, `cargo deny check`, `cargo llvm-cov`.

## Frontend (`battle-frontend/`)

```bash
npm ci
npm run lint && npm run i18n:check && npm test && npm run build
npm audit --omit=dev
```

End-to-end (Free Play; needs the backend, Postgres and `npm run dev` running; no wallet/FACEIT/Kaspa node required):

```bash
NODE_PATH=$(npm root -g) E2E_BASE_URL=http://localhost:5173 node e2e/free-play.e2e.mjs
```

## Containers and config

```bash
touch .env.production && POSTGRES_PASSWORD=placeholder docker compose config --quiet
docker build -f battle-frontend/Dockerfile --target prod .
```

## Migrations

Migrations are applied by `sqlx::migrate!` at backend start and must be additive and re-runnable in tests (see `new_migrations_leave_existing_users_and_matches_untouched`, `hash_migration_keeps_existing_sessions_valid_and_is_idempotent`). Back up the database before deploying a release that contains new migrations.

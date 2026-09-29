# Private TN10 node + FACEIT OAuth — deployment runbook

Status: code and config are in this branch. The node was **not** installed and nothing was
deployed: SSH credentials (`NODE_SSH_USER`) were not available to the agent. Every server step
below is a prepared, unexecuted runbook.

## 1. FACEIT OAuth (final flow)

1. SPA calls `GET /api/v1/faceit/auth-url` → backend stores `sha256(state)`, PKCE verifier,
   optional user and safe relative `return_to` in `faceit_oauth_states` (TTL 10 min, single use).
2. Browser does a full-page redirect to FACEIT (no `redirect_popup`, PKCE S256).
3. FACEIT redirects to `https://www.<DOMAIN>/auth/faceit/callback?code&state`. Caddy rewrites this
   to the backend `/api/v1/faceit/callback` (same origin).
4. Backend: atomic `DELETE … RETURNING` redeems the state → token exchange (HTTP Basic with
   **standard** Base64) → userinfo → user → **FACEIT link saved (errors are fatal)** → session.
5. Response: `303 See Other` to `https://www.<DOMAIN>/lobby?linked=1` with
   `Set-Cookie: kaspabattle-auth=…; HttpOnly; Path=/; SameSite=Lax; Secure; Max-Age=604800`
   (host-only, no `Domain`). No token in any URL. Failures redirect to `/?error=<code>`.
6. SPA sees `linked=1`, reloads `/auth/me` and `/faceit/status`, then cleans the URL.

Root causes fixed: state only in process memory (lost on restart / multi-instance);
URL-safe Base64 in HTTP Basic; `redirect_popup=true` with a full-page flow; frontend iframe
"frame-bust" hop; session token passed in a query string (`session-bounce`); `let _ =` swallowing
`save_faceit_link` errors; `return_to` taken from Origin/Referer headers.

**FACEIT Developer Portal (manual, one value):** the redirect URI must be exactly
`https://www.<DOMAIN>/auth/faceit/callback` and equal to `FACEIT_REDIRECT_URI`. If the portal
already holds that value (the previous default did), nothing changes.

## 2. Node server preflight (read-only) — run before anything else

```bash
hostname; curl -s https://ifconfig.me; echo         # compare with expected server
cat /etc/os-release | head -2; uname -r; nproc; free -h; uptime; df -hT
lsblk -o NAME,SIZE,TYPE,ROTA,MOUNTPOINT; ss -tulpn; docker --version
docker ps; docker compose ls; ufw status verbose 2>/dev/null || nft list ruleset | head -50
docker stats --no-stream        # OpenClaw usage; never print container env
```
Go/no-go (conservative): ≥ 8 cores, ≥ 16 GB RAM **free after OpenClaw**, ≥ 640 GB SSD/NVMe with
≥ 15 % headroom after expected growth, no port/volume conflicts (16211, 17210,
`/var/lib/kaspabattle-kaspa`). Compare with the release notes of the pinned rusty-kaspa version.

## 3. Install (only after go)

Files: `deploy/kaspa-node/` → `/opt/kaspabattle-kaspa-node`, data `/var/lib/kaspabattle-kaspa/tn10`.
Pin `KASPAD_IMAGE` (tag + digest). Verify flags/ports with `kaspad --help` of that release
(expected TN10: P2P 16211, Borsh wRPC 17210). Then
`docker compose -p kaspabattle-kaspa-node up -d`.

Firewall (additive; record `ufw status numbered` first, keep SSH):
```bash
ufw allow 16211/tcp comment 'kaspa tn10 p2p'
ufw allow from <KASPA_BATTLE_SERVER_IP> to any port 17210 proto tcp comment 'kaspa tn10 wrpc'
```
Docker-published ports bypass ufw INPUT rules; also bind `RPC_BIND_IP` to a private/specific IP or
add a `DOCKER-USER` rule limiting 17210 to the app server.

Verify before switching the app: container healthy/no restarts, logs free of panic/RocksDB/OOM,
peers > 0, `getServerInfo` → `networkId=testnet-10`, `isSynced=true`, `hasUtxoIndex=true`,
`getBalanceByAddress` for a (masked) testnet address, and a wRPC test from the app server and
through `wss://www.<DOMAIN>/kaspa-rpc`. The node is not ready until `isSynced` is true.

## 4. App switch (prepared, requires approval)

`.env`: `VITE_KASPA_NODE_URL=wss://www.<DOMAIN>/kaspa-rpc`, `KASPA_RPC_HTTP_TARGET=http://<NODE-IP>:17210`,
backend `KASPA_NODE_URL`/`KASPA_USE_EXPLICIT_NODE` per backend docs; `FRONTEND_URL` = canonical origin.
`VITE_*` are baked at build time:
```bash
git fetch origin fix/faceit-oauth-private-tn10-node-20260929 && git checkout <that branch>
docker compose build frontend backend && docker compose up -d frontend backend caddy
```
Merge the new Caddy blocks into the live `Caddyfile` (only the `/auth/faceit/callback` and
`/kaspa-rpc` blocks) and `docker exec <caddy> caddy reload --config /etc/caddy/Caddyfile`.

## 5. Rollback (never touches OpenClaw)

1. `.env`: restore previous `VITE_KASPA_NODE_URL` / `KASPA_NODE_URL` values; rebuild frontend.
2. Remove the `/kaspa-rpc` block from the Caddyfile, `caddy reload`.
3. `docker compose -p kaspabattle-kaspa-node stop` — do **not** `down -v`, do **not** delete
   `/var/lib/kaspabattle-kaspa`.
4. `git checkout <previous tag/commit>`; rebuild/restart only the Kaspa Battle services.
5. `docker ps` before and after: OpenClaw containers must be unchanged (same IDs/uptime).
   The migration `faceit_oauth_states` is additive and can stay.

## 6. Tests

- `cargo test -p battle-core faceit_oauth` with `TEST_DATABASE_URL=postgres://…` (DB tests skip
  without it): Basic-auth vector, PKCE vector, exact redirect URI, persisted+hashed state, restart,
  expiry, replay, mock token exchange (ok/fail), link-save failure, return-path validation.
- `cargo test -p battle-api --bin battle-api`: cookie attributes, post-login redirect.
- `npm test` in `battle-frontend`: post-redirect reload (`linked=1`), StrictMode once-guard, RPC
  error classification, no silent public fallback.
- Not automated yet: Playwright E2E (wallet → FACEIT mock → lobby), wallet/UTXO subscription tests
  against a real TN10 node.

# KaspaBattle — Operations Manual

## 1. Key Rotation

### ORACLE_PRIVATE_KEY
The Oracle private key is used for multisig transaction signing. Rotating it requires coordination:

1. Generate new key: `openssl rand -hex 32`
2. Update `.env.docker` or CI/CD secrets with the new value
3. **Important:** Any in-flight PSKTs signed with the old key become invalid. Ensure no matches are in `READY_FOR_PAYOUT` state before rotating.
4. Restart the backend service
5. Verify by checking logs for successful oracle key derivation

### ADMIN_API_KEY
Used for admin REST endpoints. Rotation is simpler:

1. Generate: `openssl rand -hex 32`
2. Update environment variable
3. Restart backend — takes effect immediately

### ORACLE_API_KEYS
Comma-separated list of API keys for oracle authentication:

1. Add new key to the list (don't remove old one yet)
2. Restart backend
3. Migrate any clients to use the new key
4. Remove old key from list and restart

### FACEIT_WEBHOOK_SECRET
1. Rotate in FACEIT Developer Portal first
2. Update environment variable to match
3. Restart backend

---

## 2. Kaspa Node Connectivity

### Resolver Mode (Default, Recommended)
The backend uses the Kaspa Public Node Network (PNN) Resolver by default. No explicit node URL is needed.

- Set `KASPA_USE_EXPLICIT_NODE=false` (or leave unset)
- Leave `KASPA_NODE_URL` empty or unset
- The Resolver automatically discovers the fastest available node

### Explicit Node Mode (Opt-in)
For environments with a dedicated Kaspa node:

- Set `KASPA_USE_EXPLICIT_NODE=true`
- Set `KASPA_NODE_URL=ws://<host>:16111`
- The node must have `--utxoindex` enabled

### Troubleshooting
- Check node sync status: `GET /health` endpoint returns node info
- If deposits aren't being detected, verify DAA score is non-zero in episode runner logs
- The backend auto-reconnects with exponential backoff (3 attempts)

---

## 3. Maintenance Mode

### Stopping Payout Processing
To temporarily stop payouts without taking the API offline:

```sql
-- Prevent payout worker from picking up new matches
UPDATE matches 
SET status = 'DISPUTED' 
WHERE status = 'FINISHED_FACEIT';
```

### Emergency API Shutdown
```bash
docker compose stop backend
```

### Database Maintenance
```bash
# Backup before any maintenance
docker exec kaspabattle-postgres pg_dump -U postgres kaspabattle > backup.sql
```

---

## 4. Match State Recovery

### Legacy Matches (DISPUTED due to missing multisig)
After the security hardening update, legacy matches without multisig escrow are moved to `DISPUTED` status. To resolve:

```sql
-- List all legacy-disputed matches
SELECT id, status, payout_status, winner_kas_address 
FROM matches 
WHERE payout_status = 'legacy_unsupported';
```

Manual resolution options:
1. **Refund:** Process manual refund via admin endpoint
2. **Manual payout:** Use admin tools to trigger payout after verification

### Stuck Matches
```sql
-- Find matches stuck in transitional states for > 24h
SELECT id, status, created_at 
FROM matches 
WHERE status IN ('AWAITING_FUNDING', 'FUNDED', 'LOCKED')
  AND created_at < NOW() - INTERVAL '24 hours';
```

---

## 5. Monitoring Checklist

| Check | Endpoint/Command | Expected |
|---|---|---|
| API Health | `GET /health` | 200 OK |
| DB Connection | Logs: `DB migrations applied` | On startup |
| Kaspa Node | Logs: `Connected to Kaspa node` | On startup |
| Payout Worker | Logs: `Payout Worker started` | On startup |
| Episode Runner | Logs: `Episode runner starting` | On startup |
| WebSocket | `ws://host:8080/ws` | Connection accepted |

---

## 6. Environment Variables Reference

| Variable | Required | Default | Description |
|---|---|---|---|
| `DATABASE_URL` | ✅ | — | PostgreSQL connection string |
| `ORACLE_PRIVATE_KEY` | ✅ | — | 32-byte hex key for oracle signing |
| `KASPA_NETWORK` | ❌ | `testnet-12` | Kaspa network identifier |
| `KASPA_NODE_URL` | ❌ | — | Explicit node URL (Resolver if empty) |
| `KASPA_USE_EXPLICIT_NODE` | ❌ | `false` | Enable explicit node mode |
| `KASPA_MNEMONIC` | ✅ | — | Escrow wallet mnemonic |
| `TREASURY_MNEMONIC` | ✅ | — | Treasury wallet mnemonic |
| `FRONTEND_URL` | ❌ | `http://localhost:5173` | CORS + CSRF origin |
| `FACEIT_CLIENT_ID` | ✅ | — | FACEIT OAuth client ID |
| `FACEIT_CLIENT_SECRET` | ✅ | — | FACEIT OAuth client secret |
| `FACEIT_REDIRECT_URI` | ✅ | — | OAuth callback URL |
| `FACEIT_WEBHOOK_SECRET` | ✅ | — | HMAC secret for webhooks |
| `ADMIN_API_KEY` | ✅ | — | Admin endpoint authentication |
| `ORACLE_API_KEYS` | ✅ | — | Comma-separated oracle API keys |
| `RUST_ENV` | ❌ | — | `production` for prod settings |
| `TEST_MODE` | ❌ | `false` | Enable test mode (dev only) |

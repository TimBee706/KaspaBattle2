# API Reference

## Battle API (REST)

Base URL: `/api/v1`

### Authentication Endpoints

#### POST /auth/register
Register a new user.

**Request:**
```json
{
  "username": "string",
  "password": "string"
}
```

**Response:** Auth tokens

#### POST /auth/login
Login user.

**Request:**
```json
{
  "username": "string",
  "password": "string"
}
```

**Response:** Auth tokens

#### POST /auth/logout
Logout user.

**Headers:** Authorization: Bearer <token>

#### GET /auth/me
Get current user info.

**Headers:** Authorization: Bearer <token>

**Response:** User object

#### PUT /auth/kaspa-address
Set user's Kaspa address.

**Headers:** Authorization: Bearer <token>

**Request:**
```json
{
  "kaspa_address": "string"
}
```

### FACEIT Endpoints

#### GET /faceit/link
Initiate FACEIT account linking.

**Headers:** Authorization: Bearer <token>

**Response:** Redirect to FACEIT OAuth

#### GET /faceit/login
FACEIT OAuth login.

**Response:** Redirect to FACEIT OAuth

#### GET /faceit/callback
FACEIT OAuth callback.

**Query Params:** code, state

**Response:** Redirect to frontend with token

### Oracle Endpoints

#### POST /oracle/jobs
Create oracle job for match result verification.

**Request:**
```json
{
  "match_id": "uuid",
  "faceit_match_id": "string"
}
```

**Response:** OracleJob object

#### GET /oracle/jobs/{job_id}
Get oracle job status.

**Response:** OracleJob object

### Match Endpoints

#### POST /matches
Create new match.

**Headers:** Authorization: Bearer <token>

**Request:**
```json
{
  "game_id": "string",
  "stake_kas": 1.0,
  "mode": "1v1",
  "escrow_address": "optional_string"
}
```

**Response:** Match object

#### GET /matches
List matches.

**Response:** Array of Match objects

#### POST /matches/{id}/deposit
Deposit stake for match.

**Request:**
```json
{
  "tx_hash": "string",
  "player_role": "A|B"
}
```

#### POST /matches/payout
Trigger payout (admin only).

**Request:**
```json
{
  "match_id": "uuid",
  "admin_token": "string"
}
```

## Kaspa RPC API

Based on rusty-kaspa RPC implementation.

### Core Methods

#### ping()
Test connection.

#### get_system_info()
Get node system information.

#### get_connections(include_profile_data)
Get peer connections.

#### get_metrics(process, connection, bandwidth, consensus, storage, custom)
Get node metrics.

#### get_server_info()
Get server info for connection negotiation.

#### get_sync_status()
Get node sync status.

### Wallet Methods

#### get_balance(address)
Get address balance.

#### send_transaction(tx)
Broadcast transaction.

#### get_utxos(address)
Get UTXOs for address.

### Consensus Methods

#### get_block(hash)
Get block by hash.

#### get_virtual_daa_score()
Get current DAA score.

#### submit_block(block)
Submit mined block.

## CLI Commands (rusty-kaspa)

### Wallet Commands

- `kaspa-cli wallet balance`
- `kaspa-cli wallet send <address> <amount>`
- `kaspa-cli wallet address`
- `kaspa-cli wallet utxos`

### Node Commands

- `kaspa-cli node status`
- `kaspa-cli node peers`
- `kaspa-cli node metrics`

### Message Commands

- `kaspa-cli message sign <address> <message>`
- `kaspa-cli message verify <address> <signature> <message>`
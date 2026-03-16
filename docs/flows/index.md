# Use Cases & Flows

## User Registration & Authentication

### FACEIT OAuth Flow
1. User clicks "Login with FACEIT"
2. Frontend redirects to `/api/v1/faceit/login`
3. Backend generates OAuth URL and redirects to FACEIT
4. FACEIT redirects back to `/api/v1/faceit/callback` with code
5. Backend exchanges code for tokens, fetches user data
6. Backend creates/updates user record
7. Backend redirects to frontend with session token
8. Frontend stores token and fetches user profile

### Manual Registration
1. User submits registration form
2. Frontend calls `POST /api/v1/auth/register`
3. Backend validates input, creates user
4. Backend returns auth tokens
5. Frontend stores tokens

## Match Creation & Participation

### Creating a Match
1. Authenticated user navigates to create match page
2. User selects game, stake amount, mode
3. Frontend calls `POST /api/v1/matches` with match details
4. Backend validates user has sufficient balance
5. Backend generates escrow address using multisig
6. Backend creates match record with status "Open"
7. Backend broadcasts match to lobby via WebSocket
8. Frontend updates lobby display

### Joining a Match
1. User browses open matches in lobby
2. User clicks "Join" on desired match
3. Frontend calls `POST /api/v1/matches/{id}/join`
4. Backend validates match is still open
5. Backend updates match with player B details
6. Backend changes match status to "Locked"
7. Both players must deposit stake to escrow

### Depositing Stake
1. Player generates transaction to escrow address
2. Player submits `POST /api/v1/matches/{id}/deposit` with tx_hash
3. Backend validates transaction on blockchain
4. Backend monitors for confirmation
5. Once both deposits confirmed, match status becomes "InProgress"

## Match Resolution & Payout

### Oracle Verification
1. Admin calls `POST /api/v1/matches/payout` with admin token
2. Backend creates oracle job via `POST /api/v1/oracle/jobs`
3. Oracle service queries FACEIT API for match result
4. Oracle verifies result with double confirmation
5. Oracle updates job status to "Completed"

### Payout Execution
1. Backend determines winner based on oracle result
2. Backend creates payout transaction from escrow
3. Backend signs transaction with multisig keys
4. Backend broadcasts transaction to Kaspa network
5. Backend monitors for confirmation
6. Backend updates match with payout tx_hash and status "Finished"

## Wallet Operations

### Balance Checking
1. Frontend calls Kaspa WASM `getBalance()` for user address
2. WASM connects to Kaspa RPC node
3. RPC queries UTXO set for address
4. WASM calculates total balance from UTXOs
5. Frontend displays balance

### Transaction Creation
1. User initiates send/transfer
2. Frontend collects recipient, amount
3. WASM `createTransaction()` selects UTXOs, creates tx
4. WASM signs transaction with user private key
5. WASM broadcasts via `sendTransaction()`
6. Frontend monitors transaction status

## Node Operation (rusty-kaspa)

### Starting a Node
1. User runs `kaspad` with config args
2. Binary parses args, validates configuration
3. Creates database connections (consensus, UTXO index, meta)
4. Initializes consensus manager with genesis
5. Starts RPC server, P2P networking
6. Node syncs with network, downloads blocks
7. Node participates in consensus, relays transactions

### Wallet CLI Usage
1. User runs `kaspa-cli wallet balance`
2. CLI connects to local or remote Kaspa node
3. RPC call retrieves UTXOs for wallet addresses
4. CLI calculates and displays balance
5. Similar flow for send, address generation, etc.

## Error Handling Flows

### Network Disconnection
- Frontend detects WebSocket disconnect
- Shows reconnection indicator
- Automatically attempts reconnection
- Buffers actions during disconnect
- Applies buffered actions on reconnect

### Transaction Failure
- WASM detects broadcast failure
- Frontend shows error message
- User can retry transaction
- Backend monitors for stuck transactions
- Admin can intervene for stuck payouts

### Oracle Failure
- Oracle job fails after retries
- Admin notified of failure
- Manual resolution process initiated
- Match status set to disputed
- Human arbitration for payout
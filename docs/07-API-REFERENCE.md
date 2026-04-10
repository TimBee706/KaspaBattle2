# API Reference

This document provides a complete reference for all public APIs in KaspaBattle — both the REST/WebSocket API and the kdapp framework's Rust API.

---

## Table of Contents

- [REST API](#rest-api)
- [WebSocket API](#websocket-api)
- [kdapp Rust API](#kdapp-rust-api)
- [KaspaBattle Rust Crates](#kaspabattle-rust-crates)
- [Error Types](#error-types)
- [Data Models](#data-models)

---

## REST API

All REST endpoints are served under `/api/v1` by the `battle-api` crate (Axum).

### Authentication

| Method | Endpoint | Auth | Description |
|--------|----------|------|-------------|
| `GET` | `/auth/faceit` | — | Initiate FACEIT OAuth flow |
| `GET` | `/auth/faceit/callback` | — | FACEIT OAuth callback (PKCE) |
| `GET` | `/auth/me` | 🔒 | Get current user profile |
| `POST` | `/auth/logout` | 🔒 | End session, clear cookies |

**Session management**: Authentication uses HTTP-only cookies with configurable lifetime (default 7 days).

### Matches

| Method | Endpoint | Auth | Description |
|--------|----------|------|-------------|
| `POST` | `/matches` | 🔒 | Create a new challenge |
| `GET` | `/matches` | — | List open challenges (lobby) |
| `GET` | `/matches/{id}` | — | Get match details |
| `POST` | `/matches/{id}/join` | 🔒 | Join an open challenge |
| `POST` | `/matches/{id}/cancel` | 🔒 | Cancel a challenge |
| `POST` | `/matches/{id}/faceit-match-id` | 🔒 | Submit FACEIT match ID |
| `GET` | `/matches/{id}/deposits` | 🔒 | Check deposit status |

### Create Match — Request Body

```json
{
  "game_id": "cs2",
  "match_mode": "bo1",
  "wager_amount_sompi": 10000000000
}
```

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `game_id` | string | ✅ | Game identifier: `cs2`, `valorant`, `rocket_league`, `dota2`, `lol` |
| `match_mode` | string | ✅ | Match format: `bo1` or `bo3` |
| `wager_amount_sompi` | u64 | ✅ | Wager per player in sompi (min: 1,000,000,000 = 10 KAS) |

### Create Match — Response

```json
{
  "id": "550e8400-e29b-41d4-a716-446655440000",
  "game_id": "cs2",
  "match_mode": "bo1",
  "status": "OPEN",
  "wager_amount_sompi": 10000000000,
  "escrow_address": "kaspatest:qz7yjd...",
  "player_a_faceit_nickname": "PlayerOne",
  "created_at": "2026-04-10T12:00:00Z"
}
```

### Join Match — Request Body

```json
{
  "kaspa_address": "kaspatest:qz7yjd..."
}
```

### Deposit Status — Response

```json
{
  "status": "PARTIAL",
  "deposited_sompi": 10000000000,
  "required_sompi": 20000000000,
  "deposited_kas": 100.0,
  "required_kas": 200.0,
  "player_a_deposited": true,
  "player_b_deposited": false,
  "utxo_count": 1
}
```

| Status | Meaning |
|--------|---------|
| `NONE` | No deposits received |
| `PARTIAL` | One player deposited |
| `COMPLETE` | Both players deposited (match locks) |

---

## WebSocket API

### Connection

```
ws://localhost:8080/ws
```

The WebSocket connection delivers real-time match events. No authentication is required for the connection itself; events are filtered server-side.

### Event Types

```json
{
  "type": "match_update",
  "match_id": "550e8400-...",
  "status": "WaitingForDeposits",
  "data": { ... }
}
```

| Event Type | Trigger | Data |
|------------|---------|------|
| `match_created` | New challenge posted | Match details |
| `match_joined` | Player B joined | Match details |
| `deposit_confirmed` | Deposit detected on-chain | Player ID, amount |
| `match_locked` | Both deposits confirmed | Match ID |
| `match_in_game` | FACEIT match started | FACEIT match ID |
| `match_resolved` | Oracle determined winner | Winner ID, score |
| `match_cancelled` | Match cancelled | Reason |
| `payout_complete` | Payout TX confirmed | TX hash, amount |

---

## kdapp Rust API

### Core Types

#### `Episode` Trait

```rust
pub trait Episode {
    type Command: BorshSerialize + BorshDeserialize + Debug + Clone;
    type CommandRollback: BorshSerialize + BorshDeserialize;
    type CommandError: Error + 'static;

    fn initialize(participants: Vec<PubKey>, metadata: &PayloadMetadata) -> Self;
    fn execute(
        &mut self,
        cmd: &Self::Command,
        authorization: Option<PubKey>,
        metadata: &PayloadMetadata,
    ) -> Result<Self::CommandRollback, EpisodeError<Self::CommandError>>;
    fn rollback(&mut self, rollback: Self::CommandRollback) -> bool;
}
```

#### `EpisodeEventHandler` Trait

```rust
pub trait EpisodeEventHandler<G: Episode> {
    fn on_initialize(&self, episode_id: EpisodeId, episode: &G);
    fn on_command(
        &self,
        episode_id: EpisodeId,
        episode: &G,
        cmd: &G::Command,
        authorization: Option<PubKey>,
        metadata: &PayloadMetadata,
    );
    fn on_rollback(&self, episode_id: EpisodeId, episode: &G);
}
```

#### `EpisodeId`

```rust
pub type EpisodeId = u32;
```

#### `PayloadMetadata`

```rust
pub struct PayloadMetadata {
    pub accepting_hash: Hash,     // Block hash
    pub accepting_daa: u64,       // DAA score
    pub accepting_time: u64,      // Block timestamp
    pub tx_id: Hash,              // Transaction hash
}
```

#### `EpisodeError<E>`

```rust
pub enum EpisodeError<E: Error + 'static> {
    Unauthorized,
    InvalidSignature,
    InvalidCommand(E),
    DeleteEpisode,
}
```

### Engine Types

#### `EpisodeMessage<G: Episode>`

```rust
pub enum EpisodeMessage<G: Episode> {
    NewEpisode { episode_id: EpisodeId, participants: Vec<PubKey> },
    SignedCommand { episode_id: EpisodeId, cmd: G::Command, pubkey: PubKey, sig: Sig },
    UnsignedCommand { episode_id: EpisodeId, cmd: G::Command },
    Revert { episode_id: EpisodeId },
}
```

**Factory method:**

```rust
impl<G: Episode> EpisodeMessage<G> {
    pub fn new_signed_command(
        episode_id: EpisodeId,
        cmd: G::Command,
        sk: SecretKey,
        pk: PubKey,
    ) -> Self;
}
```

#### `EngineMsg`

```rust
pub enum EngineMsg {
    BlkAccepted {
        accepting_hash: Hash,
        accepting_daa: u64,
        accepting_time: u64,
        associated_txs: Vec<(Hash, Vec<u8>)>,
    },
    BlkReverted { accepting_hash: Hash },
    Exit,
}
```

#### `Engine<G, H>`

```rust
impl<G: Episode, H: EpisodeEventHandler<G>> Engine<G, H> {
    pub fn new(receiver: Receiver<EngineMsg>) -> Self;
    pub fn start(&mut self, handlers: Vec<H>);
}
```

### Generator Types

#### `TransactionGenerator`

```rust
impl TransactionGenerator {
    pub fn new(signer: Keypair, pattern: PatternType, prefix: PrefixType) -> Self;

    pub fn build_transaction(
        &self,
        utxos: &[(TransactionOutpoint, UtxoEntry)],
        send_amount: u64,
        num_outs: u64,
        recipient: &Address,
        payload: Vec<u8>,
    ) -> Transaction;

    pub fn build_command_transaction<G: Episode>(
        &self,
        utxo: (TransactionOutpoint, UtxoEntry),
        recipient: &Address,
        cmd: &EpisodeMessage<G>,
        fee: u64,
    ) -> Transaction;
}
```

#### Helper Functions

```rust
pub fn check_pattern(tx_id: Hash, pattern: &PatternType) -> bool;
pub fn get_first_output_utxo(tx: &Transaction) -> (TransactionOutpoint, UtxoEntry);
```

#### Type Aliases

```rust
pub type PatternType = [(u8, u8); 10];  // 10 bit positions
pub type PrefixType = u32;               // 4-byte payload prefix
```

### PKI Types and Functions

```rust
pub struct PubKey(pub PublicKey);
pub struct Sig(pub Signature);

pub fn generate_keypair() -> (SecretKey, PubKey);
pub fn to_message<T: BorshSerialize>(object: &T) -> Message;
pub fn sign_message(secret_key: &SecretKey, message: &Message) -> Sig;
pub fn verify_signature(public_key: &PubKey, message: &Message, signature: &Sig) -> bool;
```

### Proxy Functions

```rust
pub async fn connect_client(
    network_id: NetworkId,
    rpc_url: Option<String>,
) -> Result<KaspaRpcClient, Error>;

pub async fn run_listener(
    kaspad: KaspaRpcClient,
    engines: EngineMap,
    exit_signal: Arc<AtomicBool>,
);

pub type EngineMap = HashMap<PrefixType, (PatternType, Sender<EngineMsg>)>;
```

---

## KaspaBattle Rust Crates

### battle-core

#### `MatchState` Enum

```rust
pub enum MatchState {
    WaitingForOpponent,
    WaitingForDeposits { player_a_deposited: bool, player_b_deposited: bool },
    Locked,
    GameIdInput { faceit_id_a: Option<String>, faceit_id_b: Option<String> },
    InGame { faceit_match_id: String },
    FinishedFaceit { winner_id: String, loser_id: String, score: String },
    ReadyForPayout { winner_id: String, pskt_hex: String },
    Resolved { winner_id: String },
    Disputed { reason: String, disputed_by: String },
    Cancelled { reason: String },
    Completed,
}
```

#### `MatchAction` Enum

```rust
pub enum MatchAction {
    Join { player_id: String, kaspa_address: String },
    Cancel { player_id: String, reason: String },
    DepositConfirmed { player_id: String, tx_hash: String, amount: u64 },
    TransitionToGameIdInput,
    SubmitFaceitMatchId { player_id: String, faceit_match_id: String },
    FaceitMatchFinished { winner_id: String, loser_id: String, score: String },
    PsktCreated { winner_id: String, pskt_hex: String },
    PayoutBroadcast { tx_hash: String, winner_id: String },
    ResolveWinner { winner_id: String },
    InitiateDispute { player_id: String, reason: String },
}
```

#### `transition()` Function

```rust
pub fn transition(
    current_state: &MatchState,
    action: &MatchAction,
    player_a_id: &str,
    player_b_id: Option<&str>,
) -> Result<MatchState, MatchError>;
```

#### Constants

```rust
pub const PLATFORM_FEE_PERCENT: u64 = 5;
pub const SESSION_LIFETIME_DAYS_DEFAULT: i64 = 7;
pub const WS_BROADCAST_CAPACITY_DEFAULT: usize = 100;
```

### battle-kaspa

#### `EscrowService`

```rust
impl EscrowService {
    pub fn new(wallet: Arc<EscrowWallet>, rpc: Arc<dyn KaspaBackend>) -> Self;
    pub fn create_escrow(&self, challenge_id: &str, wager_amount_sompi: u64) -> Result<EscrowInfo>;
    pub async fn check_deposits(&self, escrow_address: &str, wager_per_player: u64) -> Result<DepositStatus>;
    pub fn calculate_payout(&self, total_pot: u64) -> PayoutBreakdown;
}
```

#### `KaspaBackend` Trait

```rust
pub trait KaspaBackend: Send + Sync {
    async fn get_balance(&self, address: &str) -> Result<u64>;
    async fn get_utxos(&self, address: &str) -> Result<Vec<UtxoInfo>>;
    async fn submit_transaction(&self, tx: &Transaction) -> Result<String>;
}
```

---

## Error Types

### kdapp Errors

| Error | When |
|-------|------|
| `EpisodeError::Unauthorized` | Participant not in episode |
| `EpisodeError::InvalidSignature` | ECDSA verification failed |
| `EpisodeError::InvalidCommand(e)` | Application-specific validation error |
| `EpisodeError::DeleteEpisode` | Rollback past episode creation |

### KaspaBattle Errors (MatchError)

| Error | When |
|-------|------|
| `SamePlayerCannotJoin` | Player A tries to join their own match |
| `MatchFull` | Match already has two players |
| `NotAPlayer` | Non-participant tries to act |
| `AlreadyDeposited` | Player tries to deposit twice |
| `CannotCancelLockedMatch` | Cancel attempt after funds locked |
| `InvalidTransition` | Invalid state transition attempted |
| `FaceitMatchIdMismatch` | Players submitted different FACEIT IDs |

### Escrow Errors

| Error | When |
|-------|------|
| `InvalidPublicKey` | Public key validation failed |
| `DerivationFailed` | Escrow key derivation failed |
| `InsufficientFunds` | Not enough balance for payout |

---

## Data Models

### Frontend Types (TypeScript)

```typescript
interface BattleMatch {
  id: string;
  game_id: GameId;
  match_mode: MatchMode;
  status: string;
  wager_amount_sompi: number;
  escrow_address: string;
  player_a_faceit_nickname: string;
  player_b_faceit_nickname?: string;
  created_at: string;
}

type GameId = 'cs2' | 'valorant' | 'rocket_league' | 'dota2' | 'lol';
type MatchMode = 'bo1' | 'bo3';

interface PlayerAccount {
  faceit: { userId: string; nickname: string; eloLevel: number; avatarUrl: string } | null;
  wallet: { address: string; connectedAt: number } | null;
  isFullyConnected: boolean;
}
```

### Supported Games

```typescript
const SUPPORTED_GAMES = [
  { id: 'cs2', name: 'Counter-Strike 2', icon: '/game_logo_cs2.svg', platform: 'FACEIT' },
  { id: 'valorant', name: 'Valorant', icon: '/game_logo_valorant.svg', platform: 'FACEIT' },
  { id: 'rocket_league', name: 'Rocket League', icon: '/game_logo_rocket_league.svg', platform: 'FACEIT' },
  { id: 'dota2', name: 'Dota 2', icon: '/game_logo_dota2.svg', platform: 'FACEIT' },
  { id: 'lol', name: 'League of Legends', icon: '/game_logo_lol.svg', platform: 'FACEIT' },
];
```

---

## Next Steps

- [Architecture Overview](01-ARCHITECTURE.md) — System-level architecture
- [KaspaBattle Integration](06-KASPA-BATTLE-INTEGRATION.md) — Wager flow details
- [Contributing](08-CONTRIBUTING.md) — How to add new endpoints

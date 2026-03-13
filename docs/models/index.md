# Data Models

## Core Models

### User
```rust
struct User {
    id: Uuid,
    username: String,
    faceit_id: Option<String>,
    kaspa_address: Option<String>,
    display_name: String,
    avatar: Option<String>,
    created_at: DateTime<Utc>,
}
```

### Match
```rust
struct BattleMatch {
    id: Uuid,
    player_a_kas_address: String,
    player_b_kas_address: String,
    player_a_faceit_id: String,
    player_b_faceit_id: String,
    faceit_match_id: Option<String>,
    wager_amount_sompi: u64,
    escrow_address: String,
    status: MatchStatus,
    winner_kas_address: Option<String>,
    payout_tx_hash: Option<String>,
    oracle_result_signature: Option<String>,
    created_at: DateTime<Utc>,
    locked_at: Option<DateTime<Utc>>,
    resolved_at: Option<DateTime<Utc>>,
    timeout_at: DateTime<Utc>,
}
```

### MatchStatus
```rust
enum MatchStatus {
    Open,
    Locked,
    InProgress,
    Finished,
    Cancelled,
}
```

### OracleJob
```rust
struct OracleJob {
    job_id: String,
    match_id: String,
    faceit_match_id: String,
    status: OracleJobStatus,
    reported_winner: Option<String>,
    error_message: Option<String>,
    created_at: String,
    updated_at: String,
    resolved_at: Option<String>,
}
```

### OracleJobStatus
```rust
enum OracleJobStatus {
    Pending,
    Processing,
    Completed,
    Failed,
}
```

## Kaspa Models

### Transaction
```rust
struct Transaction {
    id: Hash,
    inputs: Vec<TransactionInput>,
    outputs: Vec<TransactionOutput>,
    lock_time: u64,
    subnetwork_id: SubnetworkId,
    gas: u64,
    payload: Vec<u8>,
}
```

### UtxoEntry
```rust
struct UtxoEntry {
    amount: u64,
    script_public_key: ScriptPublicKey,
    block_daa_score: u64,
    is_coinbase: bool,
}
```

### Address
```rust
enum Address {
    PublicKey(PublicKeyAddress),
    PublicKeyECDSA(PublicKeyECDSAAddress),
    ScriptHash(ScriptHashAddress),
}
```

## FACEIT Models

### FaceitPlayer
```rust
struct FaceitPlayer {
    player_id: String,
    nickname: String,
    avatar: Option<String>,
    country: String,
    skill_level: u32,
}
```

### FaceitMatch
```rust
struct FaceitMatch {
    match_id: String,
    game: String,
    status: String,
    winner: Option<String>,
    teams: Vec<FaceitTeam>,
    started_at: Option<i64>,
    finished_at: Option<i64>,
}
```

## Request/Response Models

### LoginRequest
```rust
struct LoginRequest {
    username: String,
    password: String,
}
```

### CreateMatchRequest
```rust
struct CreateReq {
    game_id: String,
    stake_kas: i64,
    mode: MatchMode,
    escrow_address: Option<String>,
}
```

### DepositRequest
```rust
struct DepositReq {
    tx_hash: String,
    player_role: String,
}
```

### AuthResponse
```rust
struct AuthResponse {
    user: User,
    access_token: String,
    refresh_token: String,
    expires_at: i64,
}
```
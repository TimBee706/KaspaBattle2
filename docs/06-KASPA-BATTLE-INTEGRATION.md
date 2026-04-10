# KaspaBattle Integration Guide

This document explains how KaspaBattle works as a P2P esports wagering platform built on Kaspa and the kdapp Episode framework.

---

## Table of Contents

- [Concept Overview](#concept-overview)
- [How kdapp Powers KaspaBattle](#how-kdapp-powers-kaspabattle)
- [Escrow Mechanism](#escrow-mechanism)
- [Oracle Integration](#oracle-integration)
- [Wager Flow — End to End](#wager-flow--end-to-end)
- [Match State Machine](#match-state-machine)
- [Frontend Integration](#frontend-integration)
- [API Design](#api-design)
- [Fee Structure](#fee-structure)
- [Deployment Strategy](#deployment-strategy)
- [Future: SilverScript Integration](#future-silverscript-integration)

---

## Concept Overview

KaspaBattle enables **trustless P2P esports wagering** on the Kaspa blockchain. Two players agree on a wager amount, deposit KAS into a per-match escrow, play their match on FACEIT, and the winner receives the pot — all automated, with blockchain-verified settlement.

```
Player A                                            Player B
   │                                                    │
   │── "I'll wager 100 KAS on CS2" ────────────────────▶│
   │                                                    │
   │◄── "I accept your challenge" ─────────────────────│
   │                                                    │
   │── Deposit 100 KAS ──▶ [Escrow Address] ◄── 100 KAS│
   │                              │                     │
   │                     Both deposits confirmed        │
   │                              │                     │
   │◄── FACEIT Match Created ─────┤                     │
   │                              │                     │
   │ ─ ─ ─ ─ Play on FACEIT ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─│
   │                              │                     │
   │                     Oracle: A wins!                │
   │                              │                     │
   │◄── 190 KAS (95%) ───────────┤                     │
   │                              │──▶ 10 KAS (5%) Platform Fee
```

**Key guarantees:**

- ✅ Both players must deposit before the match starts
- ✅ Match results come from FACEIT (verifiable third-party)
- ✅ Payouts are automatic and on-chain
- ✅ Players can dispute results
- ✅ Cancellation triggers automatic refunds

---

## How kdapp Powers KaspaBattle

KaspaBattle is built on top of the kdapp framework's Episode pattern. While the current implementation uses a hybrid approach (off-chain escrow + database state), the architecture is designed to migrate to full on-chain Episodes.

### Current Architecture (v1.0)

```
┌─────────────────────────────────────────────┐
│  battle-core: MatchState (state machine)    │
│  ──────────────────────────────────────────  │
│  Pure transition function:                   │
│  transition(state, action) → new_state       │
│                                              │
│  This is the "Episode logic" running         │
│  off-chain in the backend.                   │
└─────────────────────────────────────────────┘
         │                          │
         ▼                          ▼
┌─────────────────┐     ┌──────────────────────┐
│  PostgreSQL     │     │  battle-kaspa         │
│  (state persist)│     │  (escrow, payout)     │
└─────────────────┘     └──────────────────────┘
```

### Target Architecture (v2.0)

```
┌─────────────────────────────────────────────┐
│  battle-kdapp: BattleEpisode                │
│  ──────────────────────────────────────────  │
│  impl Episode for BattleEpisode {           │
│    fn execute(&mut self, cmd, auth, meta)   │
│    fn rollback(&mut self, rollback)         │
│  }                                           │
│                                              │
│  Runs in kdapp Engine with full DAG-aware   │
│  rollback support.                           │
└─────────────────────────────────────────────┘
         │                          │
         ▼                          ▼
┌─────────────────┐     ┌──────────────────────┐
│  kdapp Engine   │     │  Kaspa DAG            │
│  (in-memory     │     │  (on-chain state)     │
│   state + stack)│     │                       │
└─────────────────┘     └──────────────────────┘
```

---

## Escrow Mechanism

### Per-Match Escrow Addresses

Each match creates a unique escrow address on the Kaspa blockchain:

```
Escrow Address = Kaspa(secp256k1_pubkey(SHA256(match_id || pk_a || pk_b)))
```

**Properties:**
- Deterministic: same inputs → same address
- Unique: each match gets its own address
- Auditable: anyone can verify the derivation

### Deposit Tracking

The backend continuously monitors escrow addresses for deposits:

```
Deposit States:
  NONE     → No deposits received
  PARTIAL  → One player has deposited
  COMPLETE → Both players have deposited (match locks)
```

Deposits are verified by querying the Kaspa node for UTXOs at the escrow address. The backend compares the total balance against the required amount (2× wager).

### Payout Execution

When the Oracle resolves a match:

1. Build a transaction spending all escrow UTXOs
2. Create outputs: winner gets 95%, platform gets 5%
3. Sign with the escrow key (current) or cooperatively (multisig)
4. Submit to the Kaspa network
5. Mark match as `Completed` after confirmation

---

## Oracle Integration

### FACEIT as Data Source

The Oracle uses FACEIT's official API to determine match outcomes:

```
┌──────────────┐     ┌───────────────┐     ┌──────────────┐
│  FACEIT API  │────▶│  Oracle       │────▶│  Match State │
│              │     │  Service      │     │  Machine     │
│  /match/{id} │     │              │     │              │
│  /match/{id} │     │  Polls every  │     │  Transitions │
│  /stats      │     │  30 seconds   │     │  to Resolved │
└──────────────┘     └───────────────┘     └──────────────┘
```

### Oracle Workflow

1. **Match enters `InGame` state** → Oracle starts polling FACEIT
2. **FACEIT reports "finished"** → Oracle extracts winner, loser, score
3. **State transitions to `FinishedFaceit`** → PSKT creation triggered
4. **Oracle signs PSKT** → State transitions to `ReadyForPayout`
5. **Winner co-signs** → TX broadcast, state transitions to `Resolved`

### Result Verification

The Oracle verifies results by:

- Matching FACEIT player nicknames to KaspaBattle player accounts
- Checking match type and game compatibility
- Verifying the FACEIT match ID submitted by both players matches

---

## Wager Flow — End to End

### Phase 1: Challenge Creation

```
Player A → POST /api/v1/matches
  {
    game_id: "cs2",
    match_mode: "bo1",
    wager_amount_sompi: 10_000_000_000  // 100 KAS
  }

Backend:
  1. Create match record in PostgreSQL
  2. Derive escrow address
  3. Return match details + escrow address
```

### Phase 2: Challenge Acceptance

```
Player B → POST /api/v1/matches/{id}/join
  {
    kaspa_address: "kaspatest:q..."
  }

Backend:
  1. Validate Player B is not Player A
  2. Transition: WaitingForOpponent → WaitingForDeposits
  3. Return escrow address for deposits
```

### Phase 3: Deposits

```
Player A → Send wager TX to escrow address (via Kaspa WASM wallet)
Player B → Send wager TX to escrow address (via Kaspa WASM wallet)

Backend (background worker):
  1. Poll escrow UTXOs every 5 seconds
  2. Detect deposits, update state
  3. When both deposited → WaitingForDeposits → Locked → GameIdInput
```

### Phase 4: FACEIT Match

```
Both players → Submit FACEIT match ID
  POST /api/v1/matches/{id}/faceit-match-id
  { faceit_match_id: "..." }

Backend:
  1. Verify both IDs match → GameIdInput → InGame
  2. Start Oracle polling on FACEIT match
```

### Phase 5: Resolution

```
FACEIT match finishes → Oracle detects result
  1. InGame → FinishedFaceit (winner determined)
  2. Create PSKT → FinishedFaceit → ReadyForPayout
  3. Winner signs → ReadyForPayout → Resolved
  4. TX confirmed → Resolved → Completed
```

---

## Match State Machine

The match lifecycle is a formal state machine implemented in `battle-core/src/match_state.rs`:

```
WaitingForOpponent
    │
    │ Join
    ▼
WaitingForDeposits { a_deposited, b_deposited }
    │
    │ DepositConfirmed (both)
    ▼
Locked
    │
    │ TransitionToGameIdInput
    ▼
GameIdInput { faceit_id_a, faceit_id_b }
    │
    │ SubmitFaceitMatchId (both, matching)
    ▼
InGame { faceit_match_id }
    │
    │ FaceitMatchFinished
    ▼
FinishedFaceit { winner_id, loser_id, score }
    │
    │ PsktCreated
    ▼
ReadyForPayout { winner_id, pskt_hex }
    │
    │ PayoutBroadcast
    ▼
Resolved { winner_id }
    │                    │
    │ (auto)             │ InitiateDispute
    ▼                    ▼
Completed          Disputed { reason, disputed_by }
```

**Cancellation** is possible from: `WaitingForOpponent`, `WaitingForDeposits`, `GameIdInput`

**Cancellation is blocked** from: `Locked`, `InGame`, `FinishedFaceit`, `ReadyForPayout`, `Resolved`, `Completed`, `Disputed`

---

## Frontend Integration

### Kaspa WASM SDK

The frontend uses the Kaspa WASM SDK for wallet operations:

- **Key generation**: In-browser keypair creation
- **Balance queries**: Direct wRPC connection to kaspad
- **Deposit transactions**: Build, sign, and submit deposit TXs
- **No backend involvement**: Wallet keys never leave the browser

### State Management (Zustand)

The frontend uses three Zustand stores:

| Store | Purpose |
|-------|---------|
| `useAuthStore` | FACEIT auth, session, wallet connection |
| `useLobbyStore` | Open challenges, lobby polling |
| `useMatchStore` | Active match state, real-time updates |

### WebSocket Events

Real-time match updates are delivered via WebSocket:

```
Client → WS /ws
Server → { type: "match_update", match_id: "...", status: "..." }
Server → { type: "deposit_confirmed", match_id: "...", player: "..." }
Server → { type: "match_resolved", match_id: "...", winner: "..." }
```

---

## API Design

### REST Endpoints

| Method | Path | Description |
|--------|------|-------------|
| `POST` | `/api/v1/matches` | Create a new challenge |
| `GET` | `/api/v1/matches` | List open challenges |
| `GET` | `/api/v1/matches/{id}` | Get match details |
| `POST` | `/api/v1/matches/{id}/join` | Join a challenge |
| `POST` | `/api/v1/matches/{id}/cancel` | Cancel a challenge |
| `POST` | `/api/v1/matches/{id}/faceit-match-id` | Submit FACEIT match ID |
| `GET` | `/api/v1/matches/{id}/deposits` | Check deposit status |
| `GET` | `/api/v1/auth/me` | Get current user profile |
| `POST` | `/api/v1/auth/logout` | End session |

→ See [API Reference](07-API-REFERENCE.md) for complete documentation.

---

## Fee Structure

| Component | Percentage | Recipient |
|-----------|-----------|-----------|
| Winner payout | 95% | Match winner |
| Platform fee | 5% | KaspaBattle treasury |
| Network fee | ~0.00001 KAS | Kaspa miners |

Example with 100 KAS wager per player (200 KAS pot):

| Item | Amount |
|------|--------|
| Total pot | 200 KAS |
| Winner receives | 190 KAS |
| Platform fee | 10 KAS |
| Network fee | ~0.00001 KAS |

---

## Deployment Strategy

### Development (Local)

```bash
docker-compose up -d postgres
cd kaspabattle && cargo run -p battle-api
cd battle-frontend && npm run dev
```

### Staging (Docker)

```bash
docker-compose -f docker-compose.prod.yml up -d
```

Includes: PostgreSQL, backend (battle-api), frontend (nginx), Caddy reverse proxy.

### Production (Mainnet)

1. Configure production `.env` with mainnet Kaspa node URL
2. Set up Caddy reverse proxy with HTTPS (see `Caddyfile.example`)
3. Use Docker secrets for sensitive credentials
4. Enable database backups
5. Monitor escrow balances and payout status

---

## Future: SilverScript Integration

The Kaspa roadmap includes **SilverScript** — a covenant-based scripting extension that will enable complex spending conditions directly on L1.

### Impact on KaspaBattle

With SilverScript, the escrow mechanism can move fully on-chain:

```
SilverScript Escrow (conceptual):
  IF oracle_pubkey CHECKSIG AND winner_pubkey CHECKSIG
    → winner receives pot
  ELSE IF timeout_expired
    → refund to both players
  ENDIF
```

This eliminates the need for:
- Server-held escrow keys
- Backend payout signing
- Trust in the platform operator

**Timeline**: SilverScript is expected to reach testnet in 2026-2027. KaspaBattle's architecture is designed to adopt it as soon as it's available.

---

## Next Steps

- [Architecture Overview](01-ARCHITECTURE.md) — System-level architecture
- [Security Model](03-SECURITY.md) — Trust model and threat analysis
- [API Reference](07-API-REFERENCE.md) — Complete API documentation
- [Contributing](08-CONTRIBUTING.md) — How to contribute

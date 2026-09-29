# Security Model & Threat Analysis

This document describes the trust assumptions, cryptographic guarantees, attack vectors, and mitigations in KaspaBattle and the kdapp framework.

---

## Table of Contents

- [Trust Model](#trust-model)
- [Cryptographic Guarantees](#cryptographic-guarantees)
- [Escrow Security](#escrow-security)
- [Attack Vectors](#attack-vectors)
- [Timeout and Fallback Mechanisms](#timeout-and-fallback-mechanisms)
- [Multisig Constructs](#multisig-constructs)
- [Known Limitations](#known-limitations)
- [Security Checklist](#security-checklist)
- [Roadmap to Trustlessness](#roadmap-to-trustlessness)

---

## Trust Model

### What Is Trustless

| Component | Trust Level | Explanation |
|-----------|------------|-------------|
| **Kaspa consensus** | Trustless | GhostDAG ordering is enforced by all nodes |
| **UTXO ownership** | Trustless | Only the private key holder can spend a UTXO |
| **Transaction finality** | Trustless | Once deeply confirmed, transactions are immutable |
| **kdapp signature verification** | Trustless | Schnorr/ECDSA signatures are verified cryptographically |
| **kdapp rollback correctness** | Trustless | Rollbacks mechanically reverse prior state transitions |

### What Requires Trust

| Component | Trust Level | Explanation |
|-----------|------------|-------------|
| **Escrow custody (current)** | ⚠️ Trusted | Server holds escrow private keys (custodial model) |
| **Oracle (FACEIT results)** | ⚠️ Trusted | Backend fetches results from FACEIT API |
| **Match resolution** | ⚠️ Trusted | Backend Oracle determines the winner |
| **FACEIT data integrity** | ⚠️ Trusted | We trust FACEIT's API to report correct results |

> **Goal**: Migrate escrow custody from server-held keys to trustless on-chain mechanisms (multisig UTXO or kdapp Episodes) before mainnet launch.

---

## Cryptographic Guarantees

### Schnorr Signatures (Kaspa)

Kaspa uses **Schnorr signatures** over secp256k1 for transaction authorization:

- Every UTXO is locked to a public key
- To spend, the owner must produce a valid Schnorr signature
- Signature verification is performed by every node in the network

### ECDSA Signatures (kdapp)

The kdapp framework uses **ECDSA signatures** (secp256k1) for command authorization:

```
Command → Borsh serialize → SHA-256 hash → ECDSA sign with SecretKey → Signature
```

The Engine verifies the signature before executing any signed command. If verification fails, the command is rejected with `EpisodeError::InvalidSignature`.

### Key Generation

kdapp uses `secp256k1::SecretKey` generated from `OsRng` (operating system randomness):

- 32 bytes of entropy from the OS CSPRNG
- Public key derived deterministically from the secret key
- Keys are ephemeral per game session (not stored on disk)

---

## Escrow Security

### Current Model: Off-Chain Custody

```
┌──────────────────────────────────────────────────┐
│                    SERVER                         │
│                                                  │
│   Match Created → SHA-256(match_id + pubkeys)    │
│                        │                         │
│                        ▼                         │
│           Secret Key (held in memory)            │
│                        │                         │
│                        ▼                         │
│           Escrow Address derived                 │
│                                                  │
│   Players deposit → Server monitors UTXOs        │
│   Oracle resolves → Server signs payout TX       │
│                                                  │
│   ⚠️ Server has unilateral spending authority    │
└──────────────────────────────────────────────────┘
```

**Risk**: If the server is compromised, all escrowed funds can be stolen.

**Mitigations in place:**

1. Keys are zeroized on drop (`zeroize` crate pattern)
2. Each match has a unique, deterministic escrow address
3. Escrow addresses are never reused
4. Deposit tracking is per-address with UTXO verification
5. Timeout-based automatic refunds protect against griefing

### Target Model: Multisig Escrow

```
┌────────────────────────────────────────────────────────┐
│                 2-of-2 MULTISIG                        │
│                                                        │
│   Escrow UTXO locked to: player_a AND oracle           │
│                    OR:    player_b AND oracle           │
│                                                        │
│   Payout requires:                                     │
│     1. Oracle signs (certifying winner)                 │
│     2. Winner signs (claiming payout)                   │
│                                                        │
│   ✅ Neither party can spend unilaterally              │
│   ✅ Oracle cannot steal (needs winner's signature)    │
│   ✅ Winner cannot fake (needs Oracle's signature)     │
└────────────────────────────────────────────────────────┘
```

→ This is implemented in `battle-kaspa/src/multisig/` and uses PSKTs (Partially Signed Kaspa Transactions) for cooperative signing.

---

## Attack Vectors

### 1. Double-Spend on Open Escrow

**Threat**: An attacker deposits funds to the escrow, then attempts to spend the same UTXO in a competing transaction before the escrow deposit is confirmed.

**Mitigation**:
- Kaspa's 10 BPS block rate makes double-spend windows extremely short
- The backend only marks deposits as confirmed after observing the UTXO in the DAG (not the mempool)
- The escrow service waits for sufficient DAA score depth before locking the match

### 2. Griefing (Fund Freezing)

**Threat**: A player deposits funds but never plays the match, permanently locking the opponent's deposit.

**Mitigation**:
- Timeout-based cancellation: If both deposits are not received within the timeout, the match is cancelled
- Automatic refunds: The backend's refund mechanism returns funds to both players on cancellation
- Match state machine prevents indefinite locking: all states have defined exit transitions

### 3. Oracle Manipulation

**Threat**: A compromised Oracle reports false match results, directing payouts to the wrong player.

**Mitigation**:
- Oracle only reads from FACEIT's official API (trusted third-party data source)
- FACEIT match IDs are verified by both players before the match starts (SubmitFaceitMatchId state)
- Dispute mechanism (F-009): Players can dispute results, blocking payout until admin review
- Multiple Oracle confirmations can be required in future versions

### 4. DAG Reorganization

**Threat**: A confirmed transaction (deposit or payout) is reversed due to a DAG reorg, causing state inconsistency.

**Mitigation**:
- The kdapp Engine maintains a rollback stack for every command
- On `BlkReverted` events, the Engine mechanically reverses state transitions
- The Proxy tracks the virtual selected parent chain (VSPC) and detects reorgs automatically
- Episode creation itself can be rolled back (deleting the episode entirely)

### 5. Replay Attacks

**Threat**: A previously valid command is resubmitted to execute it again.

**Mitigation**:
- Each kdapp command includes the `PayloadMetadata` (accepting_hash, DAA score, tx_id)
- The Engine tracks which transactions have been processed
- Duplicate transaction IDs are rejected by the Kaspa network itself (UTXO model prevents double-spend)

### 6. Timing Attacks

**Threat**: A player observes the opponent's move on the DAG before their own transaction is confirmed, gaining an unfair advantage.

**Mitigation**:
- For turn-based games (like TicTacToe), moves are only valid when it's the player's turn
- The state machine enforces turn order
- For simultaneous-reveal games, commitment-reveal schemes can be used

### 7. Session Hijacking

**Threat**: An attacker steals a user's session token to impersonate them.

**Mitigation**:
- Sessions use HTTP-only cookies (not accessible via JavaScript)
- FACEIT OAuth uses PKCE (Proof Key for Code Exchange) to prevent authorization code interception
- Sessions have a configurable lifetime (default: 7 days)
- Rate limiting on auth endpoints prevents brute-force attempts

---

## Timeout and Fallback Mechanisms

### Episode Lifetime

The kdapp Engine automatically cleans up old episodes:

- Episodes older than `EPISODE_LIFETIME` DAA ticks are removed
- Cleanup runs every `SAMPLE_REMOVAL_TIME` DAA ticks
- This prevents memory exhaustion from abandoned sessions

### Match Timeouts

KaspaBattle implements timeouts at the application level:

| State | Timeout | Action |
|-------|---------|--------|
| WaitingForOpponent | 24 hours | Auto-cancel |
| WaitingForDeposits | 1 hour | Auto-cancel + refund |
| GameIdInput | 30 minutes | Auto-cancel + refund |
| InGame | Match duration + buffer | Oracle resolution |

---

## Multisig Constructs

### PSKT Flow (Partially Signed Kaspa Transaction)

The multisig module uses PSKTs for multi-party signing:

```
1. Oracle creates unsigned TX (winner receives pot minus fee)
2. Oracle signs with its key → PSKT (partially signed)
3. PSKT sent to winner via API
4. Winner signs with their key → fully signed TX
5. TX broadcast to Kaspa network
```

**Security properties:**

- **Oracle alone cannot steal**: Needs winner's signature
- **Winner alone cannot claim**: Needs Oracle's signature
- **Loser cannot intercept**: Their key is not part of the signing set
- **Server compromise recovery**: Oracle key rotation invalidates pending PSKTs

### Player Key Derivation (SEC-MULTISIG-01)

Player keys in the 2-of-3 redeem script (`[player_a, player_b, oracle]`) are derived
deterministically per match, not stored, so they can be reconstructed after a
backend restart. They are derived as:

```
player_key = HMAC-SHA256(MULTISIG_KEY_DERIVATION_SECRET, "{match_id}-{role}-kaspabattle-multisig-v2")
```

**Fixed 2026-09-29**: the original formula was `SHA256("{match_id}-{role}-kaspabattle-multisig")`
— a function of the match ID alone, which is public (`GET /lobbies`, `GET /matches/:id`
require no auth). That meant anyone could reconstruct *both* player keys for any match
and satisfy the 2-of-3 threshold without the Oracle key and without compromising the
server at all — not a custodial-trust issue, but an anyone-can-steal bug. Keying the
derivation with a server-only secret (`MULTISIG_KEY_DERIVATION_SECRET`, required at
startup, same handling as `ORACLE_PRIVATE_KEY`) closes this. The model is still
custodial (the server derives all three roles); full decentralization is the
SilverScript L1 covenant migration described below.

---

## Known Limitations

### L1 Script Limitations

Kaspa's scripting language is intentionally limited (similar to Bitcoin Script):

- No loops or general computation
- No on-chain state storage
- No smart contract VM (unlike Ethereum)
- Limited opcode set for spending conditions

**Impact**: Complex escrow logic (conditional payouts, time-locks, multi-party arbitration) must be implemented off-chain or via future script extensions.

### No Native Smart Contracts (Yet)

Kaspa does not currently support general-purpose smart contracts. The planned **SilverScript** extension (targeting 2026-2027) will add:

- Covenants (output spending conditions)
- Introspection opcodes (examining transaction fields)
- More expressive locking scripts

This will enable trustless escrow directly on L1 without the need for multisig workarounds.

---

## Security Checklist

### For Episode Developers

- [ ] Validate all command inputs before state transitions
- [ ] Implement rollback correctly — `rollback()` must exactly reverse `execute()`
- [ ] Handle the `Unauthorized` error for signed commands
- [ ] Set appropriate `EPISODE_LIFETIME` for your use case
- [ ] Test rollback behavior with simulated DAG reorgs
- [ ] Never store secrets in Episode state (it's serialized on-chain)
- [ ] Handle `DeleteEpisode` errors gracefully (rollback past creation)

### For KaspaBattle Deployment

- [ ] Use a strong, unique mnemonic for the escrow wallet
- [ ] Enable HTTPS/WSS for all API and RPC connections
- [ ] Rotate Oracle keys periodically
- [ ] Monitor escrow balances for unexpected withdrawals
- [ ] Set up alerts for failed payout transactions
- [ ] Enable rate limiting on all public endpoints
- [ ] Use Docker secrets (not env files) for production credentials
- [ ] Back up the PostgreSQL database regularly
- [ ] Run kaspad with `--utxoindex` and monitor sync status

---

## Roadmap to Trustlessness

```
Current (v1.0)          Near-term (v1.5)         Future (v2.0)
──────────────          ────────────────         ─────────────
Off-chain escrow        Multisig PSKT            kdapp Episode
(server custody)        (co-signing)             (on-chain logic)
                                                 SilverScript
⚠️ Trusted              ✓ Semi-trustless          ✓ Trustless
```

1. **v1.0 (Current)**: Server-managed escrow with security hardening
2. **v1.5 (In Progress)**: 2-of-2 multisig PSKT escrow — neither party can spend unilaterally
3. **v2.0 (Planned)**: Full kdapp Episode integration — wager lifecycle as on-chain state machine
4. **v3.0 (Future)**: SilverScript/Covenants for fully trustless L1 escrow

---

## Next Steps

- [Architecture Overview](01-ARCHITECTURE.md) — System-level architecture
- [Kaspa Integration](02-KASPA-INTEGRATION.md) — RPC, UTXO, wallet details
- [Episode Framework](04-EPISODE-FRAMEWORK.md) — Building trustless applications

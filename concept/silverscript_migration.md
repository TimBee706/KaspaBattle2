# KaspaBattle — SilverScript & Covenant Migration Concept

## Current State (v1): P2SH Multisig

The current implementation uses Kaspa's native P2SH with `OP_CHECKMULTISIG` for 2-of-3 escrows. This provides basic trustless custody but has limitations:

| Capability | Current (P2SH) | With Covenants |
|-----------|----------------|----------------|
| M-of-N signing | ✅ | ✅ |
| Time-locked refunds | ⚠️ Global only | ✅ Per-branch |
| Conditional spending paths | ❌ | ✅ |
| On-chain state tracking | ❌ | ✅ Covenant IDs |
| Automatic payouts | ❌ (off-chain) | ✅ Oracle attestation |
| Dispute resolution | ❌ (manual) | ✅ Scripted |

## Covenant Hard Fork (Planned May 2026)

### New Capabilities
- **Transaction introspection** — scripts can inspect their own outputs
- **8-byte arithmetic** — enabling numeric conditions and calculations
- **Covenant IDs** — programmable identifiers enforcing output rules
- **Conditional branches** — `IF/ELSE/ENDIF` with different spend paths

### SilverScript
A high-level language compiling to Kaspa scripts, making covenant logic accessible.

## Migration Plan

### Phase 1: Keep Current P2SH (Now → May 2026)
No changes needed. Current multisig module works correctly.

### Phase 2: Add Covenant Escrow (Post May 2026)

#### New Script Structure (SilverScript Pseudo-code)

```silverscript
// KaspaBattle Escrow Covenant v2
covenant BattleEscrow {
  // State: tracks match lifecycle on-chain
  state matchId: bytes32;
  state wager: u64;
  state deadline: u64;

  // Path 1: Normal payout (oracle attests winner)
  spend payout(
    winnerPubkey: pubkey,
    oracleSignature: sig,
    oracleAttestation: bytes   // match_id + winner_pubkey
  ) {
    // Verify oracle signed the attestation
    require(checkSig(oracleSignature, ORACLE_PUBKEY));
    require(sha256(oracleAttestation) == expectedHash);

    // Output: winner gets pot minus fee
    output(0).value >= self.wager * 2 * 95 / 100;
    output(0).scriptPubKey == p2pk(winnerPubkey);

    // Output: platform fee
    output(1).scriptPubKey == PLATFORM_ADDRESS;
  }

  // Path 2: Timeout refund (after deadline)
  spend refund(sigA: sig, sigB: sig) {
    require(blockTime >= self.deadline);
    require(checkSig(sigA, PLAYER_A_PUBKEY));

    // Equal split back to both players
    output(0).value >= self.wager;
    output(1).value >= self.wager;
  }

  // Path 3: Mutual cancellation (any time)
  spend cancel(sigA: sig, sigB: sig) {
    require(checkMultiSig([sigA, sigB], [PLAYER_A, PLAYER_B], 2));
    // Return deposits to original addresses
  }
}
```

### What Changes in the Code

| Module | Current | After Migration |
|--------|---------|-----------------|
| `scripts.rs` | `multisig_redeem_script()` | `build_covenant_script()` using SilverScript compiler |
| `transaction.rs` | Manual sighash + signing | Attestation-based spending (oracle signs match result) |
| `service.rs` | Backend coordinates 2-of-3 | Covenant enforces rules on-chain |
| `types.rs` | `SignatureBundle` | `CovenantSpendProof` |
| Frontend API | Same endpoints | Add attestation display |

### Migration Strategy

1. **Dual Mode**: Run both P2SH and Covenant escrows side-by-side
2. **Feature Flag**: `ESCROW_MODE=p2sh|covenant` in environment
3. **Gradual Rollout**: New matches use covenants, existing continue on P2SH
4. **Monitoring**: Compare gas costs and confirmation times between modes

### Benefits After Migration

- **Fully trustless**: No backend-held keys needed
- **On-chain state**: Match status visible on explorer
- **Automatic refunds**: Time-lock triggers without manual intervention
- **Lower trust assumptions**: Oracle only proves results, cannot steal funds
- **Gas efficiency**: Single covenant TX vs multiple signatures

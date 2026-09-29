# Interpreter tests for `match_escrow.sil`

`interpreter_tests.rs` runs `match_escrow.sil` against the real Kaspa script
interpreter (`kaspa_txscript::TxScriptEngine` with `covenants_enabled: true`)
— real compilation, real Schnorr signatures, real sighashes, real covenant
input/output authorization. It is **not** a `.test.json` file for
`cli-debugger` (that format has no built-in way to produce real signatures);
it's a Rust integration test, following the exact pattern SilverScript's own
`silverscript-lang/tests/chess_apps_tests.rs` uses.

## Why it can't run inside this repo

KaspaBattle's own Cargo workspace (`kaspabattle/Cargo.toml`) pins `kaspa-*`
crates at `0.15` from crates.io — a much older rusty-kaspa line with no
covenant support at all. This test needs the SilverScript-pinned rusty-kaspa
revision (`a41a333b08848f41bf737b72592e463a6011b8ac`, via git dependency),
which is incompatible with the 0.15 tree in the same Cargo workspace. Until
`kaspabattle/battle-silverscript` exists as its own standalone crate/workspace
with the git-pinned dependencies (see
`docs/SILVERSCRIPT-INTEGRATION-PLAN.md`, section 11), this test only runs
against a separate checkout of the SilverScript compiler itself.

## How to run it

1. Clone `kaspanet/silverscript` and check out the pinned tag:
   ```bash
   git clone https://github.com/kaspanet/silverscript.git
   cd silverscript
   git checkout v1.0.0   # 3ed973335b59269293564805cc2c58a14595ec03
   ```
2. Copy this file to `silverscript-lang/tests/kaspabattle_match_escrow_tests.rs`
   inside that checkout (it reuses `silverscript-lang/tests/common.rs`, which
   is already there).
3. Edit the `include_str!` path in `source()` to point at this repo's
   `contracts/silverscript/match_escrow.sil`.
4. `cargo test -p silverscript-lang --test kaspabattle_match_escrow_tests`.
   - Requires `libclang` (a dev-dependency, `kaspa-consensus`, needs it for
     `librocksdb-sys`). On Windows, `winget install --id LLVM.LLVM` and set
     `LIBCLANG_PATH` to `...\LLVM\bin` if `cargo test` fails with "Unable to
     find libclang".

## What's covered

12 tests, all passing as of 2026-09-29, covering all five entry points:

- `join`: happy path, rejects the creator joining their own match, rejects an
  under-funded continuation output.
- `cancel_unjoined`: happy path (proves the single-party disposal really is
  unconstrained once the signature is proven), rejects a non-`player_a`
  signer.
- `mutual_settle`: happy path with an arbitrary agreed split, rejects amounts
  that don't sum to the full pot.
- `oracle_settle`: happy path, rejects a forged oracle signature, rejects a
  payout redirected to an attacker's address while keeping a validly-signed
  attestation for the real winner (the "Output substitution" threat model
  entry).
- `refund_timeout`: succeeds exactly at the timeout boundary, rejects a claim
  one DAA tick early.

`this.ageDaa >= N` turned out to be directly testable with this same
lightweight harness, contrary to an earlier assumption in this repo's history
(see `docs/LEARNINGS.md`): it lowers to `OpCheckSequenceVerify`, which reads
the spending `TransactionInput`'s own `sequence` field — no simnet or live
consensus context needed. `tx_input_with_sequence` sets it directly.

## Findings this uncovered (see `docs/LEARNINGS.md` for the full write-up)

Two real bugs in the first draft of `match_escrow.sil`, found and fixed by
actually running this, not by reading docs or guessing:

1. `tx.outputs[idx].scriptPubKey` is `version (2 bytes, big-endian) || script
   bytes`, not just the raw script — verified against
   `kaspa_txscript::SpkEncoding::to_bytes`. Every scriptPubKey comparison in
   the contract now prepends `byte[2](0x0000)`.
2. A terminal (non-continuing) payout output must reuse the **same**
   `covenant_id` as the spent input to be counted by
   `OpAuthOutputCount`/`OpAuthOutputIdx` ("continuation"). A different
   `covenant_id` is treated as a brand-new covenant "genesis", which requires
   a protocol-derived id (from the spent outpoint) and is rejected otherwise
   (`CovenantsError::WrongGenesisCovenantId`) — verified against
   `kaspa_txscript::covenants`.

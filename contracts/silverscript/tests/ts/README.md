# TypeScript attestation reference

`attestation.ts` mirrors `kaspabattle/battle-silverscript/src/attestation.rs`
byte-for-byte. Both must always agree — that's what the known-answer digest
vector in `attestation.test.ts` checks, and it's the same vector produced by
actually running the compiled `match_escrow.sil` contract in
`contracts/silverscript/tests/interpreter_tests.rs`. Three independent
implementations (interpreter, Rust, TypeScript) agree on the same digest for
the same inputs.

Deliberately **not** part of `battle-frontend/package.json`: this is Gate 2/3
cross-language verification material for the SilverScript migration, not yet
wired into the real wallet/signing flow (that's Phase 7 implementation work
— see `docs/SILVERSCRIPT-INTEGRATION-PLAN.md` section 9's open questions).

## Run it

```bash
npm install
npm test
```

Requires Node 22.6+ (uses `--experimental-strip-types` to run `.ts` files
directly, no build step or bundler).

## Dependencies

- [`@noble/hashes`](https://github.com/paulmillr/noble-hashes) for BLAKE2b-256.
- [`@noble/curves`](https://github.com/paulmillr/noble-curves) for Schnorr
  sign/verify.

Both are widely used, audited, dependency-free libraries — chosen over a
hand-rolled BLAKE2b/Schnorr implementation given this code sits directly on
the path that decides where match payouts go.

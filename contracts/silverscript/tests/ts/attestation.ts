// `KASPABATTLE_RESULT_V1` result attestation: canonical serialization,
// digest, and Schnorr sign/verify.
//
// TypeScript mirror of `kaspabattle/battle-silverscript/src/attestation.rs`.
// Both MUST produce byte-identical output for the same inputs -- that is
// what `attestation.test.ts`'s known-answer vector checks. See
// docs/SILVERSCRIPT-INTEGRATION-PLAN.md section 8 for the wire format.
//
// Deliberately not part of battle-frontend's package.json/build yet: this is
// Gate 2/3 reference material for the SilverScript migration, not wired into
// the real wallet/signing flow (Phase 7 implementation work).

import { blake2b } from '@noble/hashes/blake2b.js';
import { schnorr } from '@noble/curves/secp256k1.js';

export const DOMAIN_TAG = new TextEncoder().encode('KASPABATTLE_RESULT_V1');

/** `contract_version` this module's encoding matches. Bump together with the
 * `.sil` contract and the Rust reference if the wire format ever changes. */
export const CONTRACT_VERSION = 1n;

export type WinnerSelector = 'player_a' | 'player_b';

function winnerSelectorToInt(winner: WinnerSelector): bigint {
    return winner === 'player_a' ? 0n : 1n;
}

export class InvalidWinnerSelectorError extends Error {}
export class ValueTooLargeError extends Error {}
export class InvalidSignatureError extends Error {}

/**
 * Kaspa's classic `CScriptNum` little-endian encoding, matching
 * `kaspa_txscript::data_stack::serialize_i64` at the pinned rusty-kaspa
 * revision (`a41a333b08848f41bf737b72592e463a6011b8ac`) byte for byte -- what
 * SilverScript's `int as byte[N]` cast lowers to (`OpNum2Bin`). Verified
 * against that source directly (see docs/LEARNINGS.md), not assumed: it is
 * NOT big-endian, and not a naive fixed-width little-endian encoding either
 * for values whose minimal encoding would need the extra sign-guard byte the
 * real algorithm inserts -- irrelevant for the always-non-negative values
 * this contract uses, but this implementation matches the real algorithm,
 * not just the common case.
 *
 * Only non-negative `value`s are supported (this contract never encodes a
 * negative attestation field).
 */
export function scriptNumBytes(value: bigint, size: number): Uint8Array {
    if (value < 0n) {
        throw new ValueTooLargeError(`value ${value} is negative, not supported`);
    }
    const out = new Uint8Array(size);
    let positive = value;
    let len = 0;
    while (positive !== 0n) {
        if (len >= size) {
            throw new ValueTooLargeError(`value ${value} does not fit in ${size} bytes`);
        }
        out[len] = Number(positive & 0xffn);
        len += 1;
        positive >>= 8n;
    }
    return out;
}

export interface AttestationContext {
    /** 32 bytes. */
    networkDomain: Uint8Array;
    /** 16 bytes (raw UUID). */
    matchId: Uint8Array;
    /** 32 bytes. */
    gameIdHash: Uint8Array;
}

export interface Attestation {
    context: AttestationContext;
    winner: WinnerSelector;
    /** 32 bytes -- off-chain commitment to the full match details. */
    resultHash: Uint8Array;
    /** DAA score or Unix time the Oracle observed the result at. */
    observedAt: bigint;
    /** 32 bytes, Oracle-chosen. */
    nonce: Uint8Array;
}

function concatBytes(...chunks: Uint8Array[]): Uint8Array {
    const total = chunks.reduce((sum, c) => sum + c.length, 0);
    const out = new Uint8Array(total);
    let offset = 0;
    for (const chunk of chunks) {
        out.set(chunk, offset);
        offset += chunk.length;
    }
    return out;
}

/** Builds the exact byte sequence `MatchEscrow.oracle_settle` reconstructs
 * and hashes. Field order and encoding must never change without bumping
 * `CONTRACT_VERSION` in lockstep with the `.sil` source and the Rust
 * reference. */
export function canonicalBytes(attestation: Attestation): Uint8Array {
    return concatBytes(
        DOMAIN_TAG,
        attestation.context.networkDomain,
        scriptNumBytes(CONTRACT_VERSION, 4),
        attestation.context.matchId,
        attestation.context.gameIdHash,
        scriptNumBytes(winnerSelectorToInt(attestation.winner), 1),
        attestation.resultHash,
        scriptNumBytes(attestation.observedAt, 8),
        attestation.nonce,
    );
}

export function digest(attestation: Attestation): Uint8Array {
    return blake2b(canonicalBytes(attestation), { dkLen: 32 });
}

/** Signs this attestation's digest with the Oracle's private key. The
 * resulting 64-byte signature is a raw Schnorr signature over the digest --
 * no sighash-type byte, unlike a transaction-bound `checkSig` signature
 * (verified: `checkMsgSig`/`OpCheckSigFromStack` takes exactly 64 bytes). */
export function sign(attestation: Attestation, oraclePrivateKey: Uint8Array): Uint8Array {
    return schnorr.sign(digest(attestation), oraclePrivateKey);
}

/** Verifies a signature against this attestation's digest and the given
 * Oracle x-only public key. Callers must separately check that
 * `oraclePublicKey` matches the match's committed `result_oracle_commitment`
 * (`blake2b(pubkey) == result_oracle_commitment`) -- this function only
 * checks the signature, exactly mirroring the contract's own two separate
 * `require`s. */
export function verify(attestation: Attestation, signature: Uint8Array, oraclePublicKey: Uint8Array): void {
    if (!schnorr.verify(signature, digest(attestation), oraclePublicKey)) {
        throw new InvalidSignatureError('invalid oracle signature');
    }
}

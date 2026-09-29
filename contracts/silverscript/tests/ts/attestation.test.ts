// Plain assert-based test runner (no test framework dependency) for the
// standalone attestation reference. Run with `npm test` in this directory
// (see README.md in the parent tests/ directory).

import { strict as assert } from 'node:assert';
import { schnorr } from '@noble/curves/secp256k1.js';
import {
    canonicalBytes,
    digest,
    scriptNumBytes,
    sign,
    verify,
    InvalidSignatureError,
    type Attestation,
} from './attestation.ts';

function hex(bytes: Uint8Array): string {
    return Array.from(bytes)
        .map((b) => b.toString(16).padStart(2, '0'))
        .join('');
}

function filled(byte: number, len: number): Uint8Array {
    return new Uint8Array(len).fill(byte);
}

let passed = 0;
function test(name: string, fn: () => void) {
    try {
        fn();
        passed += 1;
        console.log(`  PASS  ${name}`);
    } catch (err) {
        console.error(`  FAIL  ${name}`);
        console.error(err);
        process.exitCode = 1;
    }
}

console.log('running attestation.ts tests');

test('known_answer_digest_matches_rust_and_interpreter', () => {
    // Same fixture as kaspabattle/battle-silverscript/src/attestation.rs's
    // `known_answer_digest_matches_interpreter_test`, which is itself
    // cross-checked against the real compiled contract via
    // contracts/silverscript/tests/interpreter_tests.rs. All three
    // (interpreter, Rust, TypeScript) must agree byte for byte.
    const attestation: Attestation = {
        context: { networkDomain: filled(0x01, 32), matchId: filled(0x02, 16), gameIdHash: filled(0x03, 32) },
        winner: 'player_a',
        resultHash: filled(0x07, 32),
        observedAt: 123_456n,
        nonce: filled(0x08, 32),
    };
    assert.equal(hex(digest(attestation)), 'dc848da95b52e226f93992932784bccb64f24e7891fffd45f3ea43aa031acf90');
});

test('sign_then_verify_round_trips', () => {
    const privateKey = filled(0xb2, 32);
    const publicKey = schnorr.getPublicKey(privateKey);
    const attestation: Attestation = {
        context: { networkDomain: filled(0x01, 32), matchId: filled(0x02, 16), gameIdHash: filled(0x03, 32) },
        winner: 'player_b',
        resultHash: filled(0xaa, 32),
        observedAt: 1n,
        nonce: filled(0xbb, 32),
    };
    const sig = sign(attestation, privateKey);
    verify(attestation, sig, publicKey); // must not throw
});

test('verify_rejects_forged_signature', () => {
    const privateKey = filled(0xb2, 32);
    const publicKey = schnorr.getPublicKey(privateKey);
    const attestation: Attestation = {
        context: { networkDomain: filled(0x01, 32), matchId: filled(0x02, 16), gameIdHash: filled(0x03, 32) },
        winner: 'player_a',
        resultHash: filled(0xaa, 32),
        observedAt: 1n,
        nonce: filled(0xbb, 32),
    };
    const sig = sign(attestation, privateKey);
    const forged = new Uint8Array(sig);
    forged[0] ^= 0x01;
    assert.throws(() => verify(attestation, forged, publicKey), InvalidSignatureError);
});

test('verify_rejects_tampered_attestation', () => {
    // Same signature, but the winner was flipped after signing -- must not
    // verify. This is exactly what stops a compromised backend from
    // redirecting a payout by relabeling an already-signed attestation.
    const privateKey = filled(0xb2, 32);
    const publicKey = schnorr.getPublicKey(privateKey);
    const attestation: Attestation = {
        context: { networkDomain: filled(0x01, 32), matchId: filled(0x02, 16), gameIdHash: filled(0x03, 32) },
        winner: 'player_a',
        resultHash: filled(0xaa, 32),
        observedAt: 1n,
        nonce: filled(0xbb, 32),
    };
    const sig = sign(attestation, privateKey);
    const tampered: Attestation = { ...attestation, winner: 'player_b' };
    assert.throws(() => verify(tampered, sig, publicKey), InvalidSignatureError);
});

test('script_num_bytes_matches_kaspa_cscriptnum_encoding', () => {
    assert.deepEqual(scriptNumBytes(0n, 4), new Uint8Array([0, 0, 0, 0]));
    assert.deepEqual(scriptNumBytes(1n, 4), new Uint8Array([1, 0, 0, 0]));
    assert.deepEqual(scriptNumBytes(256n, 4), new Uint8Array([0, 1, 0, 0]));
    assert.deepEqual(scriptNumBytes(123_456n, 8), new Uint8Array([0x40, 0xe2, 0x01, 0, 0, 0, 0, 0]));
});

test('script_num_bytes_rejects_oversized_values', () => {
    assert.throws(() => scriptNumBytes(256n, 1));
});

test('canonical_bytes_length_matches_expected_wire_format', () => {
    const attestation: Attestation = {
        context: { networkDomain: filled(0x01, 32), matchId: filled(0x02, 16), gameIdHash: filled(0x03, 32) },
        winner: 'player_a',
        resultHash: filled(0x07, 32),
        observedAt: 123_456n,
        nonce: filled(0x08, 32),
    };
    // 21 (domain tag, "KASPABATTLE_RESULT_V1".length) + 32 + 4 + 16 + 32 + 1 + 32 + 8 + 32
    assert.equal(canonicalBytes(attestation).length, 21 + 32 + 4 + 16 + 32 + 1 + 32 + 8 + 32);
});

console.log(`${passed} tests passed`);
if (process.exitCode) {
    console.error('SOME TESTS FAILED');
} else {
    console.log('ALL TESTS PASSED');
}

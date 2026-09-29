import { describe, it, expect } from 'vitest';
import { deriveAccess, canPlayMatch, canPlayProvider, getAccessBlocker } from '../../domain/access';

const base = { isAuthenticated: true, walletConnected: true, isFaceitConnected: false };

describe('deriveAccess', () => {
    it('logged-out visitors can play nothing', () => {
        const a = deriveAccess({ isAuthenticated: false, walletConnected: false, isFaceitConnected: false });
        expect(a).toEqual({ isAuthenticated: false, hasWallet: false, hasFaceit: false, canPlayNative: false, canPlayFaceit: false });
    });

    it('native games need login + wallet but NOT FACEIT', () => {
        const a = deriveAccess(base);
        expect(a.hasFaceit).toBe(false);
        expect(a.canPlayNative).toBe(true);
        expect(a.canPlayFaceit).toBe(false);
    });

    it('FACEIT games need login + wallet + FACEIT', () => {
        expect(deriveAccess({ ...base, isFaceitConnected: true }).canPlayFaceit).toBe(true);
        expect(deriveAccess({ ...base, walletConnected: false, isFaceitConnected: true }).canPlayFaceit).toBe(false);
        expect(deriveAccess({ ...base, walletConnected: false, isFaceitConnected: true }).canPlayNative).toBe(false);
    });

    it('FACEIT alone (no wallet) is not enough for anything', () => {
        const a = deriveAccess({ isAuthenticated: true, walletConnected: false, isFaceitConnected: true });
        expect(a.hasFaceit).toBe(true);
        expect(a.canPlayNative).toBe(false);
        expect(a.canPlayFaceit).toBe(false);
    });

    it('testMode only relaxes the FACEIT requirement in the UI, never the wallet requirement', () => {
        expect(deriveAccess({ ...base, testMode: true }).canPlayFaceit).toBe(true);
        expect(deriveAccess({ ...base, walletConnected: false, testMode: true }).canPlayFaceit).toBe(false);
    });

    it('provider helpers agree with the flags', () => {
        const a = deriveAccess(base);
        expect(canPlayProvider(a, 'NATIVE')).toBe(true);
        expect(canPlayProvider(a, 'FACEIT')).toBe(false);
        expect(canPlayMatch(a, { provider: 'NATIVE' })).toBe(true);
        // legacy matches without a provider are FACEIT matches
        expect(canPlayMatch(a, {})).toBe(false);
    });

    it('explains what is missing', () => {
        expect(getAccessBlocker(deriveAccess({ isAuthenticated: false, walletConnected: false, isFaceitConnected: false }), 'NATIVE')).toBe('login');
        expect(getAccessBlocker(deriveAccess({ isAuthenticated: true, walletConnected: false, isFaceitConnected: false }), 'NATIVE')).toBe('wallet');
        expect(getAccessBlocker(deriveAccess(base), 'NATIVE')).toBeNull();
        expect(getAccessBlocker(deriveAccess(base), 'FACEIT')).toBe('faceit');
    });
});

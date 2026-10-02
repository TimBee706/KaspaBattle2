/**
 * Access rules for playing on KaspaBattle — one place instead of scattered UI conditions.
 *
 *  - isAuthenticated : there is a backend session (wallet login or FACEIT login)
 *  - hasWallet       : a Kaspa wallet address is linked to that session
 *  - hasFaceit       : a FACEIT account is linked (optional account link)
 *  - canPlayNative   : browser games need login + wallet, but NO FACEIT
 *  - canPlayFaceit   : FACEIT games need login + wallet + FACEIT (the server enforces this too)
 */
import type { BattleMatch, MatchProvider } from '../api/types';
import { getMatchProvider } from '../api/types';

export interface AccessInput {
    isAuthenticated: boolean;
    walletConnected: boolean;
    isFaceitConnected: boolean;
    /** Dev/mock "TestUser" mode: only relaxes the FACEIT requirement in the UI. */
    testMode?: boolean;
}

export interface AccessState {
    isAuthenticated: boolean;
    hasWallet: boolean;
    hasFaceit: boolean;
    canPlayNative: boolean;
    canPlayFaceit: boolean;
    /** Free Play (off-chain, stake-free) needs a login and nothing else: no wallet, no FACEIT. */
    canPlayFreePlay: boolean;
}

export function deriveAccess(input: AccessInput): AccessState {
    const isAuthenticated = input.isAuthenticated;
    const hasWallet = isAuthenticated && input.walletConnected;
    const hasFaceit = isAuthenticated && input.isFaceitConnected;
    return {
        isAuthenticated,
        hasWallet,
        hasFaceit,
        canPlayFreePlay: isAuthenticated,
        canPlayNative: isAuthenticated && hasWallet,
        canPlayFaceit: isAuthenticated && hasWallet && (hasFaceit || !!input.testMode),
    };
}

/** Can this user create / join / play a match of the given provider? */
export function canPlayProvider(access: AccessState, provider: MatchProvider): boolean {
    return provider === 'NATIVE' ? access.canPlayNative : access.canPlayFaceit;
}

export function canPlayMatch(access: AccessState, match: Pick<BattleMatch, 'provider'>): boolean {
    return canPlayProvider(access, getMatchProvider(match));
}

/** Why the user cannot play — drives the hint under buttons. `null` = allowed. */
export type AccessBlocker = 'login' | 'wallet' | 'faceit' | null;

export function getAccessBlocker(access: AccessState, provider: MatchProvider): AccessBlocker {
    if (canPlayProvider(access, provider)) return null;
    if (!access.isAuthenticated) return 'login';
    if (!access.hasWallet) return 'wallet';
    return 'faceit';
}

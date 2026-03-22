import { describe, expect, it } from 'vitest';
import type { BattleMatch, PaymentStatus } from '../../api/types';
import {
    getLobbyRole,
    getPaymentInfoForPlayer,
    getPlayerRoleForLobby,
    hasPlayerDeposited,
    isAvailableChallenge,
    isLocalDepositForIdentity,
    isMyLobby,
    needsPlayerDeposit,
} from '../../domain/lobby';

const baseLobby: BattleMatch = {
    id: 'match-1',
    creator_user_id: 'user-a',
    opponent_user_id: null,
    player_a_kas_address: 'kaspatest:qa',
    player_b_kas_address: null,
    player_a_faceit_id: '',
    player_b_faceit_id: null,
    player_a_faceit_nickname: 'Player A',
    player_b_faceit_nickname: null,
    faceit_match_id: null,
    wager_amount_sompi: 1_000_000_000,
    escrow_address: 'kaspatest:qescrow',
    status: 'OPEN',
    game_id: 'cs2',
    match_mode: 'BO1',
    winner_kas_address: null,
    winner_faceit_nickname: null,
    payout_tx_hash: null,
    score: null,
    player_a_deposit_tx_hash: null,
    player_b_deposit_tx_hash: null,
    created_at: '2026-03-22T00:00:00.000Z',
    locked_at: null,
    resolved_at: null,
    timeout_at: '2026-03-22T01:30:00.000Z',
};

const paymentStatus: PaymentStatus = {
    escrow_address: 'kaspatest:qescrow',
    required_per_player_sompi: 1_000_000_000,
    min_confirmations_required: 10,
    playerA: { paid: true, confirmed_sompi: 1_000_000_000, payment_count: 1, min_confirmations: 12 },
    playerB: { paid: false, confirmed_sompi: 0, payment_count: 0, min_confirmations: 0 },
    both_paid: false,
};

describe('lobby domain helpers', () => {
    it('classifies my lobbies and available challenges by current user', () => {
        expect(getLobbyRole(baseLobby, 'user-a')).toBe('creator');
        expect(getLobbyRole(baseLobby, 'user-b')).toBe('viewer');
        expect(isMyLobby(baseLobby, 'user-a')).toBe(true);
        expect(isMyLobby(baseLobby, 'user-b')).toBe(false);
        expect(isAvailableChallenge(baseLobby, 'user-b')).toBe(true);
        expect(isAvailableChallenge(baseLobby, 'user-a')).toBe(false);
    });

    it('derives player-specific deposit state instead of using global lobby flags', () => {
        const joinedLobby: BattleMatch = {
            ...baseLobby,
            status: 'AWAITING_FUNDING',
            opponent_user_id: 'user-b',
            player_b_kas_address: 'kaspatest:qb',
            player_b_faceit_nickname: 'Player B',
            player_a_deposit_tx_hash: 'tx-a',
        };

        expect(getPlayerRoleForLobby(joinedLobby, 'user-a')).toBe('A');
        expect(getPlayerRoleForLobby(joinedLobby, 'user-b')).toBe('B');
        expect(getPaymentInfoForPlayer(paymentStatus, 'A')?.paid).toBe(true);
        expect(getPaymentInfoForPlayer(paymentStatus, 'B')?.paid).toBe(false);
        expect(hasPlayerDeposited(paymentStatus, 'A')).toBe(true);
        expect(hasPlayerDeposited(paymentStatus, 'B')).toBe(false);
        expect(needsPlayerDeposit(joinedLobby, 'user-a')).toBe(false);
        expect(needsPlayerDeposit(joinedLobby, 'user-b')).toBe(true);
    });

    it('scopes local deposit hints to match, user, and wallet identity', () => {
        const localDeposit = {
            txHash: 'tx-local',
            matchId: 'match-1',
            userId: 'user-a',
            walletAddress: 'kaspatest:qa',
        };

        expect(isLocalDepositForIdentity(localDeposit, {
            matchId: 'match-1',
            userId: 'user-a',
            walletAddress: 'kaspatest:qa',
        })).toBe(true);

        expect(isLocalDepositForIdentity(localDeposit, {
            matchId: 'match-1',
            userId: 'user-b',
            walletAddress: 'kaspatest:qb',
        })).toBe(false);
    });
});

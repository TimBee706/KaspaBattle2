import type { BattleMatch } from '../../api/types';
import type { NativeGameSnapshot } from '../../domain/nativeGame';

export const ALICE = '11111111-1111-4111-8111-111111111111';
export const BOB = '22222222-2222-4222-8222-222222222222';
export const MATCH_ID = '99999999-9999-4999-8999-999999999999';

export function emptyBoard(): number[][] {
    return Array.from({ length: 6 }, () => Array(7).fill(0));
}

export function makeSnapshot(overrides: Partial<NativeGameSnapshot> = {}): NativeGameSnapshot {
    return {
        matchId: MATCH_ID,
        gameType: 'CONNECT_FOUR',
        status: 'ACTIVE',
        matchStatus: 'IN_GAME',
        version: 3,
        board: emptyBoard(),
        columns: 7,
        rows: 6,
        moveCount: 0,
        currentPlayerId: ALICE,
        players: [
            { userId: ALICE, displayName: 'Alice', slot: 1, color: 'blue' },
            { userId: BOB, displayName: 'Bob', slot: 2, color: 'red' },
        ],
        winnerUserId: null,
        result: null,
        endReason: null,
        winningLine: [],
        lastMove: null,
        turnDeadline: new Date(Date.now() + 60_000).toISOString(),
        stateHash: 'hash-0',
        startedAt: new Date().toISOString(),
        finishedAt: null,
        serverTime: new Date().toISOString(),
        you: { userId: ALICE, slot: 1 },
        ...overrides,
    };
}

export function makeMatch(overrides: Partial<BattleMatch> = {}): BattleMatch {
    return {
        id: MATCH_ID,
        creator_user_id: ALICE,
        opponent_user_id: BOB,
        player_a_kas_address: 'kaspatest:a',
        player_b_kas_address: 'kaspatest:b',
        player_a_faceit_id: '',
        player_b_faceit_id: null,
        player_a_faceit_nickname: '',
        player_b_faceit_nickname: null,
        faceit_match_id: null,
        wager_amount_sompi: 1_000_000_000,
        escrow_address: 'kaspatest:escrow',
        status: 'IN_GAME',
        game_id: 'connect-four',
        match_mode: 'BO1',
        winner_kas_address: null,
        winner_faceit_nickname: null,
        payout_tx_hash: null,
        score: null,
        player_a_deposit_tx_hash: 'a',
        player_b_deposit_tx_hash: 'b',
        created_at: new Date().toISOString(),
        locked_at: null,
        resolved_at: null,
        timeout_at: new Date().toISOString(),
        provider: 'NATIVE',
        native_game_type: 'CONNECT_FOUR',
        requires_faceit: false,
        player_a_display_name: 'Alice',
        player_b_display_name: 'Bob',
        ...overrides,
    };
}

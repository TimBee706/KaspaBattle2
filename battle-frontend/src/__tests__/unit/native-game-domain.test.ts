import { describe, it, expect } from 'vitest';
import {
    hasGameSession, isColumnFull, isMyTurn, isWinningCell, lowestFreeRow, pickNewerSnapshot, getSlotOf,
} from '../../domain/nativeGame';
import { getMatchProvider, isNativeMatch } from '../../api/types';
import { ALICE, BOB, emptyBoard, makeSnapshot } from '../helpers/nativeFixtures';

describe('native game domain helpers', () => {
    it('finds the lowest free row (row 0 is the bottom)', () => {
        const board = emptyBoard();
        expect(lowestFreeRow(board, 3)).toBe(0);
        board[0][3] = 1;
        board[1][3] = 2;
        expect(lowestFreeRow(board, 3)).toBe(2);
        for (let r = 0; r < 6; r++) board[r][0] = 1;
        expect(lowestFreeRow(board, 0)).toBe(-1);
        expect(isColumnFull(board, 0)).toBe(true);
        expect(isColumnFull(board, 1)).toBe(false);
    });

    it('turn and seat detection come from the server snapshot', () => {
        const snap = makeSnapshot({ currentPlayerId: BOB });
        expect(isMyTurn(snap, BOB)).toBe(true);
        expect(isMyTurn(snap, ALICE)).toBe(false);
        expect(isMyTurn(snap, null)).toBe(false);
        expect(isMyTurn(makeSnapshot({ status: 'FINISHED', currentPlayerId: null }), ALICE)).toBe(false);
        expect(getSlotOf(snap, ALICE)).toBe(1);
        expect(getSlotOf(snap, 'someone-else')).toBeNull();
    });

    it('never replaces a newer snapshot with an older WebSocket event', () => {
        const v5 = makeSnapshot({ version: 5 });
        const v4 = makeSnapshot({ version: 4 });
        expect(pickNewerSnapshot(v5, v4)).toBe(v5);
        expect(pickNewerSnapshot(v4, v5)).toBe(v5);
        expect(pickNewerSnapshot(null, v4)).toBe(v4);
        // a different match always wins
        const other = makeSnapshot({ matchId: 'other', version: 1 });
        expect(pickNewerSnapshot(v5, other)).toBe(other);
    });

    it('marks winning cells', () => {
        const snap = makeSnapshot({ winningLine: [[0, 0], [1, 0], [2, 0], [3, 0]] });
        expect(isWinningCell(snap, 2, 0)).toBe(true);
        expect(isWinningCell(snap, 4, 0)).toBe(false);
    });

    it('knows in which match states a game session exists', () => {
        expect(hasGameSession({ status: 'IN_GAME' })).toBe(true);
        expect(hasGameSession({ status: 'FINISHED_GAME' })).toBe(true);
        expect(hasGameSession({ status: 'REFUND_PENDING' })).toBe(true);
        expect(hasGameSession({ status: 'OPEN' })).toBe(false);
        expect(hasGameSession({ status: 'AWAITING_FUNDING' })).toBe(false);
    });

    it('treats matches without a provider (legacy / older backend) as FACEIT', () => {
        expect(getMatchProvider({})).toBe('FACEIT');
        expect(getMatchProvider({ provider: 'FACEIT' })).toBe('FACEIT');
        expect(getMatchProvider({ provider: 'NATIVE' })).toBe('NATIVE');
        expect(isNativeMatch({ provider: 'NATIVE' })).toBe(true);
        expect(isNativeMatch({})).toBe(false);
    });
});

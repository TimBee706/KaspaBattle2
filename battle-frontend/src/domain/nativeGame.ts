/** Types + pure helpers for native browser games (mirrors the backend `GameSnapshot`). */
import type { MatchStatus } from '../api/types';

export const CONNECT_FOUR_COLUMNS = 7;
export const CONNECT_FOUR_ROWS = 6;

export type NativeSessionStatus = 'READY' | 'ACTIVE' | 'FINISHED';
export type NativeResult = 'WIN' | 'DRAW';
export type NativeEndReason = 'CONNECT_FOUR' | 'DRAW' | 'RESIGNATION' | 'TIMEOUT';

export interface NativePlayer {
    userId: string;
    displayName: string;
    /** 1 = creator (moves first, blue), 2 = opponent (red) */
    slot: 1 | 2;
    color: 'blue' | 'red';
}

export interface NativeLastMove {
    column: number;
    row: number;
    playerId: string | null;
}

export interface NativeGameSnapshot {
    matchId: string;
    gameType: 'CONNECT_FOUR';
    status: NativeSessionStatus;
    matchStatus: MatchStatus;
    version: number;
    /** board[row][column], row 0 = BOTTOM. 0 empty, 1 blue (player one), 2 red (player two). */
    board: number[][];
    columns: number;
    rows: number;
    moveCount: number;
    currentPlayerId: string | null;
    players: NativePlayer[];
    winnerUserId: string | null;
    result: NativeResult | null;
    endReason: NativeEndReason | null;
    /** [row, column] pairs of the winning run */
    winningLine: Array<[number, number]>;
    lastMove: NativeLastMove | null;
    turnDeadline: string | null;
    stateHash: string;
    startedAt: string | null;
    finishedAt: string | null;
    serverTime: string;
    /** Present on GET /game: who the caller is (null when not logged in). */
    you?: { userId: string; slot: 1 | 2 | null } | null;
    /** Present on mutating responses: request was an idempotent replay. */
    replay?: boolean;
}

export type NativeGameEvent =
    | { type: 'native_game_started'; match_id: string; version: number; state: NativeGameSnapshot }
    | { type: 'native_game_state'; match_id: string; version: number; state: NativeGameSnapshot }
    | {
        type: 'native_game_move';
        match_id: string;
        version: number;
        move: { sequence: number; column: number; row: number; playerId: string; stateHash: string };
        state: NativeGameSnapshot;
    }
    | { type: 'native_game_finished'; match_id: string; version: number; state: NativeGameSnapshot }
    | { type: 'match_update'; match: { id: string } & Record<string, unknown> };

export const NATIVE_EVENT_TYPES = [
    'native_game_started',
    'native_game_state',
    'native_game_move',
    'native_game_finished',
] as const;

/** Lowest empty row of a column, or -1 if the column is full. */
export function lowestFreeRow(board: number[][], column: number): number {
    for (let row = 0; row < board.length; row++) {
        if (board[row]?.[column] === 0) return row;
    }
    return -1;
}

export function isColumnFull(board: number[][], column: number): boolean {
    return lowestFreeRow(board, column) === -1;
}

export function getSlotOf(snapshot: Pick<NativeGameSnapshot, 'players'>, userId: string | null | undefined): 1 | 2 | null {
    if (!userId) return null;
    return snapshot.players.find((p) => p.userId === userId)?.slot ?? null;
}

export function isMyTurn(snapshot: NativeGameSnapshot, userId: string | null | undefined): boolean {
    return !!userId && snapshot.status === 'ACTIVE' && snapshot.currentPlayerId === userId;
}

/** Applies a server event only if it is newer than what we already show (WS may reorder / replay). */
export function pickNewerSnapshot(
    current: NativeGameSnapshot | null,
    incoming: NativeGameSnapshot,
): NativeGameSnapshot {
    if (!current || current.matchId !== incoming.matchId) return incoming;
    return incoming.version >= current.version ? incoming : current;
}

export function isWinningCell(snapshot: Pick<NativeGameSnapshot, 'winningLine'>, row: number, column: number): boolean {
    return snapshot.winningLine.some(([r, c]) => r === row && c === column);
}

/** Statuses in which a native game session can exist. */
const GAME_STATUSES = new Set<MatchStatus>([
    'READY_TO_PLAY', 'IN_GAME', 'FINISHED_GAME', 'READY_FOR_PAYOUT', 'RESOLVED', 'PAID_OUT',
    'REFUND_PENDING', 'REFUNDED', 'DISPUTED',
]);

export function hasGameSession(match: { status: MatchStatus }): boolean {
    return GAME_STATUSES.has(match.status);
}

/** The minimal data the Connect Four board needs – shared by paid native games and Free Play. */
export type BoardView = Pick<NativeGameSnapshot, 'board' | 'rows' | 'columns' | 'status' | 'lastMove' | 'winningLine'>;

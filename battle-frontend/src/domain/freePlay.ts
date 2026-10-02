/** Types + pure helpers for Free Play (mirrors the backend `FpSnapshot`). */
import type { NativeGameSnapshot } from './nativeGame';

export type FpOpponentKind = 'human' | 'bot';
export type FpDifficulty = 'easy' | 'medium' | 'hard';
export type FpStatus = 'open' | 'active' | 'finished' | 'cancelled';

export interface FpPlayer {
    slot: 1 | 2;
    userId: string | null;
    displayName: string;
    isBot: boolean;
    color: 'blue' | 'red';
}

export interface FpSnapshot {
    id: string;
    mode: 'free_play';
    gameType: 'connect_four';
    opponentKind: FpOpponentKind;
    botDifficulty: FpDifficulty | null;
    status: FpStatus;
    version: number;
    /** board[row][column], row 0 = bottom */
    board: number[][];
    columns: number;
    rows: number;
    moveCount: number;
    currentSlot: 1 | 2 | null;
    currentPlayerId: string | null;
    players: FpPlayer[];
    winnerSlot: 1 | 2 | null;
    winnerUserId: string | null;
    result: 'win' | 'draw' | 'abandoned' | null;
    endReason: 'connect_four' | 'draw' | 'resignation' | 'timeout' | 'left' | null;
    winningLine: Array<[number, number]>;
    lastMove: { column: number; row: number; slot: number } | null;
    turnDeadline: string | null;
    stateHash: string;
    createdAt: string;
    startedAt: string | null;
    finishedAt: string | null;
    rematchRequestedBy: string | null;
    rematchGameId: string | null;
    turnTimeoutSecs: number;
    serverTime: string;
    you?: { userId: string; slot: 1 | 2 | null };
    presence?: Record<string, boolean>;
    replay?: boolean;
}

export interface FpLobbyItem {
    id: string;
    creatorId: string;
    creatorName: string;
    createdAt: string;
    mine: boolean;
}

export interface FpActiveItem {
    id: string;
    opponentKind: FpOpponentKind;
    botDifficulty: FpDifficulty | null;
    opponentName: string;
    yourTurn: boolean;
    moveCount: number;
}

export interface FpHistoryItem {
    id: string;
    opponentKind: FpOpponentKind;
    botDifficulty: FpDifficulty | null;
    opponentName: string;
    youSlot: 1 | 2;
    outcome: 'win' | 'loss' | 'draw' | 'abandoned';
    endReason: string | null;
    moveCount: number;
    createdAt: string;
    finishedAt: string | null;
}

export interface FpStats {
    played: number;
    wins: number;
    losses: number;
    draws: number;
    abandoned: number;
    vsHumans: number;
    vsBots: number;
}

/** Newer-wins merge so a late WebSocket event can never roll the board back. */
export function pickNewer(current: FpSnapshot | null, incoming: FpSnapshot): FpSnapshot {
    if (!current || current.id !== incoming.id) return incoming;
    return incoming.version >= current.version ? incoming : current;
}

/** Adapts a Free Play snapshot to the board component's input (shared with the native game). */
export function toBoardView(s: FpSnapshot): Pick<NativeGameSnapshot, 'board' | 'rows' | 'columns' | 'status' | 'lastMove' | 'winningLine'> {
    return {
        board: s.board,
        rows: s.rows,
        columns: s.columns,
        status: s.status === 'active' ? 'ACTIVE' : s.status === 'open' ? 'READY' : 'FINISHED',
        lastMove: s.lastMove ? { column: s.lastMove.column, row: s.lastMove.row, playerId: null } : null,
        winningLine: s.winningLine,
    };
}

export function myMark(s: FpSnapshot, userId: string | null | undefined): 1 | 2 | null {
    if (!userId) return null;
    return s.players.find((p) => p.userId === userId)?.slot ?? null;
}

export function isMyTurn(s: FpSnapshot, userId: string | null | undefined): boolean {
    return s.status === 'active' && !!userId && s.currentPlayerId === userId;
}

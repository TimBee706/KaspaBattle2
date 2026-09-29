import apiClient from './client';
import type { NativeGameSnapshot } from '../domain/nativeGame';

export interface MoveRequest {
    column: number;
    expectedVersion: number;
    clientNonce: string;
}

/** Full snapshot. Always load this first, and again after every WebSocket reconnect. */
export async function getGame(matchId: string): Promise<NativeGameSnapshot> {
    const res = await apiClient.get<NativeGameSnapshot>(`/matches/${matchId}/game`);
    return res.data;
}

/** Idempotent: starting an already running game just returns the snapshot. */
export async function startGame(matchId: string): Promise<NativeGameSnapshot> {
    const res = await apiClient.post<NativeGameSnapshot>(`/matches/${matchId}/game/start`);
    return res.data;
}

/** The client only sends a column — the server decides the row, the turn and the winner. */
export async function postConnectFourMove(matchId: string, req: MoveRequest): Promise<NativeGameSnapshot> {
    const res = await apiClient.post<NativeGameSnapshot>(`/matches/${matchId}/connect-four/moves`, req);
    return res.data;
}

export async function resignGame(matchId: string): Promise<NativeGameSnapshot> {
    const res = await apiClient.post<NativeGameSnapshot>(`/matches/${matchId}/game/resign`);
    return res.data;
}

export async function getFeatures(): Promise<{ nativeGamesEnabled: boolean }> {
    const res = await apiClient.get<{ nativeGamesEnabled: boolean }>('/features');
    return res.data;
}

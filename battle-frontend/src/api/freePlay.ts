import apiClient from './client';
import type { FpActiveItem, FpDifficulty, FpHistoryItem, FpLobbyItem, FpSnapshot, FpStats } from '../domain/freePlay';

export const createFreePlayGame = (opponent: 'human' | 'bot', botDifficulty?: FpDifficulty) =>
    apiClient
        .post<FpSnapshot>('/free-play/games', { opponent, ...(opponent === 'bot' ? { botDifficulty: botDifficulty ?? 'easy' } : {}) })
        .then((r) => r.data);

export const listFreePlayLobbies = () =>
    apiClient.get<{ lobbies: FpLobbyItem[]; active: FpActiveItem[] }>('/free-play/lobbies').then((r) => r.data);

export const getFreePlayGame = (id: string) => apiClient.get<FpSnapshot>(`/free-play/games/${id}`).then((r) => r.data);
export const joinFreePlayGame = (id: string) => apiClient.post<FpSnapshot>(`/free-play/games/${id}/join`).then((r) => r.data);
export const leaveFreePlayGame = (id: string) => apiClient.post<FpSnapshot>(`/free-play/games/${id}/leave`).then((r) => r.data);
export const rematchFreePlayGame = (id: string) => apiClient.post<FpSnapshot>(`/free-play/games/${id}/rematch`).then((r) => r.data);

export const postFreePlayMove = (id: string, body: { column: number; expectedVersion: number; clientNonce: string }) =>
    apiClient.post<FpSnapshot>(`/free-play/games/${id}/moves`, body).then((r) => r.data);

export const getFreePlayHistory = (limit = 20) =>
    apiClient.get<{ games: FpHistoryItem[] }>('/free-play/history', { params: { limit } }).then((r) => r.data.games);

export const getFreePlayStats = () => apiClient.get<FpStats>('/free-play/stats').then((r) => r.data);

import apiClient from './client';
import type { FaceitProfileResponse, FaceitMatchHistoryResponse } from './types';

export interface FaceitStatusResponse {
    connected: boolean;
    faceit_nickname: string | null;
    faceit_avatar_url: string | null;
    faceit_elo: number | null;
    faceit_skill_level: number | null;
    linked_at: string | null;
}

export interface FaceitStatsResponse {
    game_id: string;
    lifetime: Record<string, any>;
    is_cached: boolean;
}

export const faceitApi = {
    /**
     * Schneller FACEIT-Verbindungsstatus (nur DB, kein FACEIT-API-Call).
     */
    getStatus: async (): Promise<FaceitStatusResponse> => {
        const response = await apiClient.get<FaceitStatusResponse>('/faceit/status');
        return response.data;
    },

    /**
     * Ruft das Faceit-Profil (Stats + Level + ELO) des aktuellen Users ab.
     */
    getProfile: async (): Promise<FaceitProfileResponse> => {
        const response = await apiClient.get<FaceitProfileResponse>('/faceit/profile');
        return response.data;
    },

    /**
     * Ruft Spiel-spezifische Stats ab.
     * @param game Standard: 'cs2'
     */
    getStats: async (game = 'cs2'): Promise<FaceitStatsResponse> => {
        const response = await apiClient.get<FaceitStatsResponse>(`/faceit/stats?game=${game}`);
        return response.data;
    },

    /**
     * Ruft die letzten Matches des aktuellen Users ab.
     * @param game Standard: 'cs2'
     * @param offset Startpunkt für Pagination
     * @param limit Anzahl der Matches (z.B. 20)
     */
    getMatches: async (game = 'cs2', offset = 0, limit = 20): Promise<FaceitMatchHistoryResponse> => {
        const response = await apiClient.get<FaceitMatchHistoryResponse>(`/faceit/matches?game=${game}&offset=${offset}&limit=${limit}`);
        return response.data;
    },

    /**
     * FACEIT-Verbindung trennen (User bleibt eingeloggt).
     */
    disconnect: async (): Promise<void> => {
        await apiClient.post('/faceit/disconnect');
    },
};

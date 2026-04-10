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

/**
 * Lifetime-Stats aus GET /faceit/stats?game=cs2
 * Spiegelt FaceitLifetimeStats (battle-core/models/faceit_data.rs).
 *
 * F-12: Alle FACEIT-Felder sind typisiert. Optionale Felder können fehlen
 * wenn ein Spieler noch keine Daten für ein Feld hat.
 */
export interface FaceitLifetimeStats {
    // Kern-Felder (immer vorhanden)
    Matches: string;
    'Win Rate %': string;
    'Recent Results': string[];
    Wins: string;

    // FPS-spezifische Felder (CS2, Valorant — nicht verfügbar für LoL/Rocket League)
    'Average K/D Ratio'?: string;
    'Average Headshots %'?: string;

    // Erweiterte Felder (F-12, optional)
    'Current Win Streak'?: string;
    'Longest Win Streak'?: string;
    'Total Headshots %'?: string;
    'Average K/R Ratio'?: string;
    Leaves?: string;
    'Average Kills'?: string;
    'Average Deaths'?: string;
    'Average Assists'?: string;
    'Maps Played'?: string;
}

export interface FaceitStatsResponse {
    game_id: string;
    lifetime: FaceitLifetimeStats;
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
     * @param game Optional: game_id für game-spezifischen ELO (Standard: 'cs2')
     */
    getProfile: async (game?: string): Promise<FaceitProfileResponse> => {
        const params = game ? `?game=${game}` : '';
        const response = await apiClient.get<FaceitProfileResponse>(`/faceit/profile${params}`);
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

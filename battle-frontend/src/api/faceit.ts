import apiClient from './client';
import type { FaceitProfileResponse, FaceitMatchHistoryResponse } from './types';

export const faceitApi = {
    /**
     * Ruft das Faceit-Profil (Stats + Level + ELO) des aktuellen Users ab.
     */
    getProfile: async (): Promise<FaceitProfileResponse> => {
        const response = await apiClient.get<FaceitProfileResponse>('/faceit/profile');
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
};

import { create } from 'zustand';
import type { BattleMatch, LobbyFilters } from '../api/types';

interface LobbyState {
    challenges: BattleMatch[];
    total: number;
    isLoading: boolean;
    error: string | null;
    filters: LobbyFilters;

    setChallenges: (challenges: BattleMatch[], total: number) => void;
    setLoading: (isLoading: boolean) => void;
    setError: (error: string | null) => void;
    setFilters: (filters: Partial<LobbyFilters>) => void;
    resetFilters: () => void;
}

const DEFAULT_FILTERS: LobbyFilters = {
    page: 1,
    per_page: 20,
};

export const useLobbyStore = create<LobbyState>((set, get) => ({
    challenges: [],
    total: 0,
    isLoading: false,
    error: null,
    filters: DEFAULT_FILTERS,

    setChallenges: (challenges, total) => set({ challenges, total, isLoading: false, error: null }),
    setLoading: (isLoading) => set({ isLoading }),
    setError: (error) => set({ error, isLoading: false }),
    setFilters: (newFilters) => set({ filters: { ...get().filters, ...newFilters, page: 1 } }),
    resetFilters: () => set({ filters: DEFAULT_FILTERS }),
}));

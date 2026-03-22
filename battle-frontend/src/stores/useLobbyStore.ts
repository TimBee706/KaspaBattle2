import { create } from 'zustand';
import type { BattleMatch } from '../api/types';

interface LobbyState {
    lobbies: BattleMatch[];
    history: BattleMatch[];
    isLoading: boolean;
    error: string | null;

    setLobbies: (lobbies: BattleMatch[]) => void;
    setHistory: (history: BattleMatch[]) => void;
    addOrUpdateLobby: (match: BattleMatch) => void;
    setLoading: (isLoading: boolean) => void;
    setError: (error: string | null) => void;
    reset: () => void;
}

export const useLobbyStore = create<LobbyState>((set) => ({
    lobbies: [],
    history: [],
    isLoading: false,
    error: null,

    setLobbies: (lobbies) => set({ lobbies, isLoading: false, error: null }),
    setHistory: (history) => set({ history, isLoading: false, error: null }),
    addOrUpdateLobby: (match) => set((state) => {
        const idx = state.lobbies.findIndex(l => l.id === match.id);
        const next = [...state.lobbies];
        if (idx >= 0) {
            next[idx] = match;
        } else {
            next.push(match);
        }
        return { lobbies: next };
    }),
    setLoading: (isLoading) => set({ isLoading }),
    setError: (error) => set({ error, isLoading: false }),
    reset: () => set({ lobbies: [], history: [], isLoading: false, error: null }),
}));

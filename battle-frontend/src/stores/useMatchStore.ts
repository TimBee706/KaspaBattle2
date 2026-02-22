import { create } from 'zustand';
import type { BattleMatch } from '../api/types';

interface MatchState {
    currentMatch: BattleMatch | null;
    isLoading: boolean;
    error: string | null;
    depositTxHash: string | null;
    isDepositing: boolean;

    setMatch: (match: BattleMatch) => void;
    setLoading: (isLoading: boolean) => void;
    setError: (error: string | null) => void;
    setDepositTxHash: (txHash: string) => void;
    setDepositing: (isDepositing: boolean) => void;
    clearMatch: () => void;
}

export const useMatchStore = create<MatchState>((set) => ({
    currentMatch: null,
    isLoading: false,
    error: null,
    depositTxHash: null,
    isDepositing: false,

    setMatch: (match) => set({ currentMatch: match, error: null }),
    setLoading: (isLoading) => set({ isLoading }),
    setError: (error) => set({ error, isLoading: false }),
    setDepositTxHash: (txHash) => set({ depositTxHash: txHash, isDepositing: false }),
    setDepositing: (isDepositing) => set({ isDepositing }),
    clearMatch: () => set({ currentMatch: null, depositTxHash: null, error: null }),
}));

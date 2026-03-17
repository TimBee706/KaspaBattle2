import { create } from 'zustand';
import type { BattleMatch, PaymentStatus } from '../api/types';

interface MatchState {
    currentMatch: BattleMatch | null;
    isLoading: boolean;
    error: string | null;
    depositTxHash: string | null;
    isDepositing: boolean;
    /** Live payment confirmation status from GET /matches/:id/payment-status */
    paymentStatus: PaymentStatus | null;

    setMatch: (match: BattleMatch) => void;
    setLoading: (isLoading: boolean) => void;
    setError: (error: string | null) => void;
    setDepositTxHash: (txHash: string) => void;
    setDepositing: (isDepositing: boolean) => void;
    setPaymentStatus: (status: PaymentStatus | null) => void;
    clearMatch: () => void;
}

export const useMatchStore = create<MatchState>((set) => ({
    currentMatch: null,
    isLoading: false,
    error: null,
    depositTxHash: null,
    isDepositing: false,
    paymentStatus: null,

    setMatch: (match) => set({ currentMatch: match, error: null }),
    setLoading: (isLoading) => set({ isLoading }),
    setError: (error) => set({ error, isLoading: false }),
    setDepositTxHash: (txHash) => set({ depositTxHash: txHash, isDepositing: false }),
    setDepositing: (isDepositing) => set({ isDepositing }),
    setPaymentStatus: (paymentStatus) => set({ paymentStatus }),
    clearMatch: () => set({ currentMatch: null, depositTxHash: null, error: null, paymentStatus: null }),
}));

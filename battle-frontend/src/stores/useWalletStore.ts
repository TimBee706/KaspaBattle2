import { create } from 'zustand';
import type { SafeAccount } from '../kaspa/wallet';

interface WalletState {
    isConnected: boolean;
    isConnecting: boolean;
    account: SafeAccount | null;
    address: string | null;
    walletType: 'mnemonic' | 'kasware' | null;
    balanceSompi: number;
    isFetchingBalance: boolean;
    balanceError: string | null;
    error: string | null;

    setWalletConnection: (account: SafeAccount, address: string, walletType: 'mnemonic' | 'kasware') => void;
    setBalance: (balanceSompi: number) => void;
    setFetchingBalance: (isFetching: boolean) => void;
    setBalanceError: (error: string | null) => void;
    setConnecting: (isConnecting: boolean) => void;
    setError: (error: string | null) => void;
    disconnect: () => void;
}

export const useWalletStore = create<WalletState>((set) => ({
    isConnected: false,
    isConnecting: false,
    account: null,
    address: null,
    walletType: null,
    balanceSompi: -1,
    isFetchingBalance: false,
    balanceError: null,
    error: null,

    setWalletConnection: (account, address, walletType) =>
        set({ account, address, walletType, isConnected: true, isConnecting: false, error: null }),

    setBalance: (balanceSompi) => set({ balanceSompi, isFetchingBalance: false, balanceError: null }),

    setFetchingBalance: (isFetchingBalance) => set({ isFetchingBalance }),

    setBalanceError: (balanceError) => set({ balanceError, isFetchingBalance: false }),

    setConnecting: (isConnecting) => set({ isConnecting }),

    setError: (error) => set({ error, isConnecting: false }),

    disconnect: () =>
        set({
            account: null,
            address: null,
            walletType: null,
            isConnected: false,
            balanceSompi: -1,
            isFetchingBalance: false,
            balanceError: null,
            error: null
        }),
}));

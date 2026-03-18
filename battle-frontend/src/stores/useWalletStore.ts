import { create } from 'zustand';
import type { Wallet } from 'kaspa-wasm';

interface WalletState {
    isConnected: boolean;
    isConnecting: boolean;
    wallet: Wallet | null;
    account: any; // Account used here is a custom object from our wallet.ts logic
    address: string | null;
    walletType: 'mnemonic' | 'kasware' | null;
    balanceSompi: number;
    isFetchingBalance: boolean;
    balanceError: string | null;
    error: string | null;

    setWalletConnection: (wallet: Wallet | null, account: any, address: string, walletType: 'mnemonic' | 'kasware') => void;
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
    wallet: null,
    account: null,
    address: null,
    walletType: null,
    balanceSompi: -1,
    isFetchingBalance: false,
    balanceError: null,
    error: null,

    setWalletConnection: (wallet, account, address, walletType) =>
        set({ wallet, account, address, walletType, isConnected: true, isConnecting: false, error: null }),

    setBalance: (balanceSompi) => set({ balanceSompi, isFetchingBalance: false, balanceError: null }),

    setFetchingBalance: (isFetchingBalance) => set({ isFetchingBalance }),

    setBalanceError: (balanceError) => set({ balanceError, isFetchingBalance: false }),

    setConnecting: (isConnecting) => set({ isConnecting }),

    setError: (error) => set({ error, isConnecting: false }),

    disconnect: () =>
        set({
            wallet: null,
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

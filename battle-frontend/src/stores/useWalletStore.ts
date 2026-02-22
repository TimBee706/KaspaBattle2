import { create } from 'zustand';
import type { Wallet, Account } from 'kaspa-wasm';

interface WalletState {
    isConnected: boolean;
    isConnecting: boolean;
    wallet: Wallet | null;
    account: Account | null;
    address: string | null;
    balanceSompi: number;
    error: string | null;

    setWalletConnection: (wallet: Wallet, account: Account, address: string) => void;
    setBalance: (balanceSompi: number) => void;
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
    balanceSompi: 0,
    error: null,

    setWalletConnection: (wallet, account, address) =>
        set({ wallet, account, address, isConnected: true, isConnecting: false, error: null }),

    setBalance: (balanceSompi) => set({ balanceSompi }),

    setConnecting: (isConnecting) => set({ isConnecting }),

    setError: (error) => set({ error, isConnecting: false }),

    disconnect: () =>
        set({ wallet: null, account: null, address: null, isConnected: false, balanceSompi: 0, error: null }),
}));

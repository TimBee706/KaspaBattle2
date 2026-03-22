import { create } from 'zustand';
import type { AuthTokens, UserProfile } from '../api/types';
import { logout as logoutApi } from '../api/auth';
import { useLobbyStore } from './useLobbyStore';
import { useMatchStore } from './useMatchStore';

interface AuthState {
    user: UserProfile | null;
    isAuthenticated: boolean;
    walletConnected: boolean;
    testMode: boolean;
    isFaceitConnected: boolean;

    setAuth: (user: UserProfile, _tokens?: AuthTokens | null) => void;
    fetchUser: () => Promise<void>;
    logout: () => Promise<void>;
    clearAuthState: () => void;
    updateKasAddress: (address: string) => void;
    setWalletConnected: (connected: boolean) => void;
    setTestMode: (enabled: boolean) => void;
}

export const useAuthStore = create<AuthState>()((set, get) => ({
    user: null,
    isAuthenticated: false,
    walletConnected: false,
    testMode: false,
    isFaceitConnected: false,

    setAuth: (user) => {
        useLobbyStore.getState().reset();
        useMatchStore.getState().clearMatch();
        set({
            user,
            isAuthenticated: true,
            isFaceitConnected: user.faceit_connected === true && !!user.faceit_id,
        });
    },

    fetchUser: async () => {
        const res = await (await import('../api/client')).default.get<UserProfile>('/auth/me');
        const user = res.data;
        if (import.meta.env.DEV) {
            const prev = get().user;
            if (prev?.faceit_connected && !user.faceit_connected) {
                console.warn('[AuthStore] ⚠️ fetchUser would lose FACEIT data!', { prev, next: user });
            }
        }
        set({
            user,
            isAuthenticated: true,
            isFaceitConnected: user.faceit_connected === true && !!user.faceit_id,
        });
    },

    logout: async () => {
        try {
            await logoutApi();
        } catch (error) {
            console.warn('[AuthStore] Logout request failed, clearing local state anyway', error);
        }
        get().clearAuthState();
    },

    clearAuthState: () => {
        useLobbyStore.getState().reset();
        useMatchStore.getState().clearMatch();
        set({ user: null, isAuthenticated: false, walletConnected: false, isFaceitConnected: false });
        sessionStorage.clear();
    },

    updateKasAddress: (address) => {
        const { user } = get();
        if (user) {
            set({ user: { ...user, kaspa_address: address }, walletConnected: true });
        }
    },

    setWalletConnected: (connected) => set({ walletConnected: connected }),
    setTestMode: (enabled) => set({ testMode: enabled }),
}));

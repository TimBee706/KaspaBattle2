import { create } from 'zustand';
import { persist } from 'zustand/middleware';
import type { AuthTokens, UserProfile } from '../api/types';
import { refreshToken as refreshTokenApi } from '../api/auth';

interface AuthState {
    user: UserProfile | null;
    tokens: AuthTokens | null;
    isAuthenticated: boolean;
    walletConnected: boolean;
    testMode: boolean;
    isFaceitConnected: boolean;

    setAuth: (user: UserProfile, tokens: AuthTokens) => void;
    setTokens: (tokens: AuthTokens) => void;
    fetchUser: () => Promise<void>;
    logout: () => void;
    refreshAccessToken: () => Promise<void>;
    updateKasAddress: (address: string) => void;
    setWalletConnected: (connected: boolean) => void;
    setTestMode: (enabled: boolean) => void;
}

export const useAuthStore = create<AuthState>()(
    persist(
        (set, get) => ({
            user: null,
            tokens: null,
            isAuthenticated: false,
            walletConnected: false,
            testMode: false,
            isFaceitConnected: false,

            setAuth: (user, tokens) => set({
                user,
                tokens,
                isAuthenticated: true,
                isFaceitConnected: user.faceit_connected === true && !!user.faceit_id,
            }),
            setTokens: (tokens) => set({ tokens }),
            fetchUser: async () => {
                const res = await (await import('../api/client')).default.get<UserProfile>('/auth/me');
                const user = res.data;
                set({
                    user,
                    isAuthenticated: true,
                    isFaceitConnected: user.faceit_connected === true && !!user.faceit_id,
                });
            },

            logout: () => {
                set({ user: null, tokens: null, isAuthenticated: false, walletConnected: false, isFaceitConnected: false });
                sessionStorage.clear();
            },

            refreshAccessToken: async () => {
                const { tokens } = get();
                if (!tokens?.refresh_token) throw new Error('No refresh token');
                const response = await refreshTokenApi(tokens.refresh_token);
                set({ tokens: response.tokens, user: response.user });
            },

            updateKasAddress: (address) => {
                const { user } = get();
                if (user) set({ user: { ...user, kaspa_address: address }, walletConnected: true });
            },

            setWalletConnected: (connected) => set({ walletConnected: connected }),
            setTestMode: (enabled) => set({ testMode: enabled }),
        }),
        { name: 'kaspabattle-auth' },
    ),
);

import { create } from 'zustand';
import { persist } from 'zustand/middleware';
import type { AuthTokens, UserProfile } from '../api/types';
import { refreshToken as refreshTokenApi } from '../api/auth';

interface AuthState {
    user: UserProfile | null;
    tokens: AuthTokens | null;
    isAuthenticated: boolean;

    setAuth: (user: UserProfile, tokens: AuthTokens) => void;
    logout: () => void;
    refreshAccessToken: () => Promise<void>;
    updateKasAddress: (address: string) => void;
}

export const useAuthStore = create<AuthState>()(
    persist(
        (set, get) => ({
            user: null,
            tokens: null,
            isAuthenticated: false,

            setAuth: (user, tokens) => set({ user, tokens, isAuthenticated: true }),

            logout: () => {
                set({ user: null, tokens: null, isAuthenticated: false });
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
                if (user) set({ user: { ...user, kas_address: address } });
            },
        }),
        { name: 'kaspabattle-auth' },
    ),
);

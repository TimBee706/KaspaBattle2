import { create } from 'zustand';
import { persist, createJSONStorage } from 'zustand/middleware';
import type { AuthTokens, PlayerAccount, UserProfile } from '../api/types';
import { logout as logoutApi } from '../api/auth';
import { useLobbyStore } from './useLobbyStore';
import { useMatchStore } from './useMatchStore';

const PLAYER_ACCOUNT_STORAGE_KEY = 'kaspa_battle_player_account';

function buildPlayerAccount(
    user: UserProfile | null,
    connectedAt: number | null,
): PlayerAccount {
    const faceit = user?.faceit_connected && user.faceit_id
        ? {
            userId: user.faceit_id,
            nickname: user.faceit_nickname,
            eloLevel: user.faceit_skill_level,
            avatarUrl: user.faceit_avatar,
        }
        : null;

    const wallet = user?.kaspa_address
        ? {
            address: user.kaspa_address,
            connectedAt,
        }
        : null;

    return {
        faceit,
        wallet,
        isFullyConnected: !!faceit && !!wallet,
    };
}

interface AuthState {
    user: UserProfile | null;
    playerAccount: PlayerAccount;
    isAuthenticated: boolean;
    isAuthLoading: boolean;
    walletConnected: boolean;
    testMode: boolean;
    isFaceitConnected: boolean;
    isFullyConnected: boolean;

    setAuth: (user: UserProfile, _tokens?: AuthTokens | null) => void;
    fetchUser: () => Promise<void>;
    logout: () => Promise<void>;
    clearAuthState: () => void;
    updateKasAddress: (address: string) => void;
    clearKasAddress: () => void;
    setWalletConnected: (connected: boolean) => void;
    setTestMode: (enabled: boolean) => void;
}

const emptyPlayerAccount: PlayerAccount = {
    faceit: null,
    wallet: null,
    isFullyConnected: false,
};

function applyUserState(set: (partial: Partial<AuthState>) => void, user: UserProfile) {
    const playerAccount = buildPlayerAccount(
        user,
        user.kaspa_address ? Date.now() : null,
    );

    set({
        user,
        playerAccount,
        isAuthenticated: true,
        walletConnected: !!playerAccount.wallet,
        isFaceitConnected: !!playerAccount.faceit,
        isFullyConnected: playerAccount.isFullyConnected,
    });
}

export const useAuthStore = create<AuthState>()(
    persist(
        (set, get) => ({
            user: null,
            playerAccount: emptyPlayerAccount,
            isAuthenticated: false,
            isAuthLoading: true,
            walletConnected: false,
            testMode: false,
            isFaceitConnected: false,
            isFullyConnected: false,

            setAuth: (user) => {
                useLobbyStore.getState().reset();
                useMatchStore.getState().clearMatch();
                applyUserState(set, user);
            },

            fetchUser: async () => {
                try {
                    const res = await (await import('../api/client')).default.get<UserProfile>('/auth/me');
                    const user = res.data;
                    if (import.meta.env.DEV) {
                        const prev = get().user;
                        if (prev?.faceit_connected && !user.faceit_connected) {
                            console.warn('[AuthStore] fetchUser would lose FACEIT data!', { prev, next: user });
                        }
                    }
                    applyUserState(set, user);
                } finally {
                    set({ isAuthLoading: false });
                }
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
                set({
                    user: null,
                    playerAccount: emptyPlayerAccount,
                    isAuthenticated: false,
                    walletConnected: false,
                    isFaceitConnected: false,
                    isFullyConnected: false,
                });
                sessionStorage.clear();
            },

            updateKasAddress: (address) => {
                const { user, playerAccount } = get();
                const wallet = { address, connectedAt: Date.now() };
                const nextPlayerAccount: PlayerAccount = {
                    faceit: playerAccount.faceit,
                    wallet,
                    isFullyConnected: !!playerAccount.faceit,
                };

                set({
                    user: user ? { ...user, kaspa_address: address } : user,
                    playerAccount: nextPlayerAccount,
                    walletConnected: true,
                    isFullyConnected: nextPlayerAccount.isFullyConnected,
                });
            },

            clearKasAddress: () => {
                const { user, playerAccount } = get();
                const nextPlayerAccount: PlayerAccount = {
                    faceit: playerAccount.faceit,
                    wallet: null,
                    isFullyConnected: false,
                };

                set({
                    user: user ? { ...user, kaspa_address: null } : user,
                    playerAccount: nextPlayerAccount,
                    walletConnected: false,
                    isFullyConnected: false,
                });
            },

            setWalletConnected: (connected) => {
                const { user, playerAccount } = get();
                const wallet = connected
                    ? playerAccount.wallet ?? (user?.kaspa_address ? { address: user.kaspa_address, connectedAt: Date.now() } : null)
                    : null;
                const nextPlayerAccount: PlayerAccount = {
                    faceit: playerAccount.faceit,
                    wallet,
                    isFullyConnected: !!playerAccount.faceit && !!wallet,
                };

                set({
                    playerAccount: nextPlayerAccount,
                    walletConnected: connected && !!wallet,
                    isFullyConnected: nextPlayerAccount.isFullyConnected,
                });
            },

            setTestMode: (enabled) => set({ testMode: enabled }),
        }),
        {
            name: PLAYER_ACCOUNT_STORAGE_KEY,
            storage: createJSONStorage(() => localStorage),
            partialize: (state) => ({
                playerAccount: state.playerAccount,
            }),
        },
    ),
);

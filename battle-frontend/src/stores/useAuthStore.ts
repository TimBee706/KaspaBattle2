import { create } from 'zustand';
import { persist, createJSONStorage } from 'zustand/middleware';
import type { AuthTokens, PlayerAccount, UserProfile } from '../api/types';
import { logout as logoutApi } from '../api/auth';
import { useLobbyStore } from './useLobbyStore';
import { useMatchStore } from './useMatchStore';
import { deriveAccess } from '../domain/access';

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
    // ── Access (derived, see domain/access.ts). FACEIT is an optional link: native browser
    // games need login + wallet only, FACEIT games additionally need a linked FACEIT account.
    hasWallet: boolean;
    hasFaceit: boolean;
    canPlayNative: boolean;
    canPlayFaceit: boolean;

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

/** Recomputes the derived access flags from the source-of-truth fields of the store. */
function accessFields(state: Pick<AuthState, 'isAuthenticated' | 'walletConnected' | 'isFaceitConnected' | 'testMode'>) {
    const access = deriveAccess(state);
    return {
        hasWallet: access.hasWallet,
        hasFaceit: access.hasFaceit,
        canPlayNative: access.canPlayNative,
        canPlayFaceit: access.canPlayFaceit,
    };
}

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
    syncAccess(set);
}

/** Zustand's `set` only accepts partials, so read the fresh state through `syncAccess`'s store ref. */
let storeRef: { getState: () => AuthState } | null = null;
function syncAccess(set: (partial: Partial<AuthState>) => void) {
    if (storeRef) set(accessFields(storeRef.getState()));
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
            hasWallet: false,
            hasFaceit: false,
            canPlayNative: false,
            canPlayFaceit: false,

            setAuth: (user) => {
                useLobbyStore.getState().reset();
                useMatchStore.getState().clearMatch();
                applyUserState(set, user);
            },

            fetchUser: async () => {
                try {
                    let attempts = 0;
                    const maxAttempts = 5;
                    while (attempts < maxAttempts) {
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
                            break; // Success, exit loop
                        } catch (err: unknown) {
                            attempts++;
                            const error = err as { response?: { status?: number } };
                            if (error?.response?.status === 429 && attempts < maxAttempts) {
                                console.warn(`[AuthStore] fetchUser rate limited (429). Retrying... (Attempt ${attempts} of ${maxAttempts})`);
                                await new Promise((resolve) => setTimeout(resolve, 2000 * attempts));
                            } else {
                                throw err;
                            }
                        }
                    }
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
                syncAccess(set);
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
                syncAccess(set);
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
                syncAccess(set);
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
                syncAccess(set);
            },

            setTestMode: (enabled) => {
                set({ testMode: enabled });
                syncAccess(set);
            },
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

storeRef = useAuthStore;

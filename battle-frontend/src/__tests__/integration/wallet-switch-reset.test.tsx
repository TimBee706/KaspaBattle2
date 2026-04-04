import { act, renderHook } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { useWallet } from '../../hooks/useWallet';
import { useAuthStore } from '../../stores/useAuthStore';
import { useLobbyStore } from '../../stores/useLobbyStore';
import { useMatchStore } from '../../stores/useMatchStore';
import { useWalletStore } from '../../stores/useWalletStore';

const { apiClient } = vi.hoisted(() => ({
    apiClient: {
        get: vi.fn(),
        post: vi.fn(),
    },
}));

vi.mock('kaspa-wasm', () => ({
    signMessage: vi.fn().mockResolvedValue('signed-message'),
}));

vi.mock('../../api/client', () => ({
    default: apiClient,
}));

vi.mock('../../kaspa/wallet', () => ({
    importWallet: vi.fn().mockResolvedValue({
        connection: {
            wallet: null,
            account: {
                receiveAddress: 'kaspatest:qnewwallet',
                escrowAddress: 'kaspatest:qescrow',
                xpub: 'xpub-test',
                publicKey: 'pub-key',
            },
            address: 'kaspatest:qnewwallet',
        },
        ephemeralPrivateKeyHex: 'ephemeral-priv-key',
    }),
    getBalanceByAddress: vi.fn().mockResolvedValue(1_500_000_000),
    getRpcClient: vi.fn().mockResolvedValue({
        subscribeUtxosChanged: vi.fn().mockResolvedValue(undefined),
        addEventListener: vi.fn(),
    }),
}));

describe('useWallet player account linking', () => {
    beforeEach(() => {
        apiClient.get.mockReset();
        apiClient.post.mockReset();
        useAuthStore.getState().clearAuthState();
        useLobbyStore.getState().reset();
        useMatchStore.getState().clearMatch();
        useWalletStore.getState().disconnect();
    });

    it('links a wallet to the active FACEIT session without logging the user out', async () => {
        useAuthStore.setState({
            user: {
                id: 'user-a',
                faceit_id: 'faceit-user-a',
                faceit_nickname: 'FaceitA',
                faceit_connected: true,
                display_name: 'Player A',
                faceit_avatar: 'avatar-a',
                faceit_elo: 1500,
                faceit_skill_level: 7,
                kaspa_address: null,
                total_matches: 0,
                wins: 0,
                losses: 0,
                total_wagered_sompi: 0,
                total_won_sompi: 0,
                created_at: '2026-03-22T00:00:00.000Z',
            },
            playerAccount: {
                faceit: {
                    userId: 'faceit-user-a',
                    nickname: 'FaceitA',
                    eloLevel: 7,
                    avatarUrl: 'avatar-a',
                },
                wallet: null,
                isFullyConnected: false,
            },
            isAuthenticated: true,
            walletConnected: false,
            isFaceitConnected: true,
            isFullyConnected: false,
            testMode: false,
        });
        useLobbyStore.setState({
            lobbies: [{
                id: 'match-old',
                creator_user_id: 'user-a',
                opponent_user_id: null,
                player_a_kas_address: 'kaspatest:qold',
                player_b_kas_address: null,
                player_a_faceit_id: 'faceit-user-a',
                player_b_faceit_id: null,
                player_a_faceit_nickname: 'FaceitA',
                player_b_faceit_nickname: null,
                faceit_match_id: null,
                wager_amount_sompi: 1_000_000_000,
                escrow_address: 'kaspatest:qescrow',
                status: 'OPEN',
                game_id: 'cs2',
                match_mode: 'BO1',
                winner_kas_address: null,
                winner_faceit_nickname: null,
                payout_tx_hash: null,
                score: null,
                player_a_deposit_tx_hash: null,
                player_b_deposit_tx_hash: null,
                created_at: '2026-03-22T00:00:00.000Z',
                locked_at: null,
                resolved_at: null,
                timeout_at: '2026-03-22T01:30:00.000Z',
            }],
            history: [],
            isLoading: false,
            error: null,
        });
        useMatchStore.setState({
            currentMatch: {
                id: 'match-old',
                creator_user_id: 'user-a',
                opponent_user_id: 'user-b',
                player_a_kas_address: 'kaspatest:qold',
                player_b_kas_address: 'kaspatest:qb',
                player_a_faceit_id: 'faceit-user-a',
                player_b_faceit_id: '',
                player_a_faceit_nickname: 'FaceitA',
                player_b_faceit_nickname: 'Player B',
                faceit_match_id: null,
                wager_amount_sompi: 1_000_000_000,
                escrow_address: 'kaspatest:qescrow',
                status: 'AWAITING_FUNDING',
                game_id: 'cs2',
                match_mode: 'BO1',
                winner_kas_address: null,
                winner_faceit_nickname: null,
                payout_tx_hash: null,
                score: null,
                player_a_deposit_tx_hash: 'tx-a',
                player_b_deposit_tx_hash: null,
                created_at: '2026-03-22T00:00:00.000Z',
                locked_at: null,
                resolved_at: null,
                timeout_at: '2026-03-22T01:30:00.000Z',
            },
            isLoading: false,
            error: null,
            localDeposit: {
                txHash: 'tx-a',
                matchId: 'match-old',
                userId: 'user-a',
                walletAddress: 'kaspatest:qold',
            },
            isDepositing: false,
            paymentStatus: null,
        });

        apiClient.post.mockImplementation(async (url: string, body?: any) => {
            if (url === '/auth/wallet-challenge') {
                return {
                    data: {
                        challenge_id: 'challenge-1',
                        message: 'sign me',
                        expires_at: '2026-03-22T00:05:00.000Z',
                    },
                };
            }
            if (url === '/auth/wallet-verify') {
                expect(body).toMatchObject({
                    kaspa_address: 'kaspatest:qnewwallet',
                    link_to_existing_user: true,
                });
                return { data: {} };
            }
            throw new Error(`Unexpected POST ${url}`);
        });

        apiClient.get.mockImplementation(async (url: string) => {
            if (url === '/auth/me') {
                return {
                    data: {
                        id: 'user-a',
                        faceit_id: 'faceit-user-a',
                        faceit_nickname: 'FaceitA',
                        faceit_connected: true,
                        display_name: 'Player A',
                        faceit_avatar: 'avatar-a',
                        faceit_elo: 1500,
                        faceit_skill_level: 7,
                        kaspa_address: 'kaspatest:qnewwallet',
                        total_matches: 0,
                        wins: 0,
                        losses: 0,
                        total_wagered_sompi: 0,
                        total_won_sompi: 0,
                        created_at: '2026-03-22T00:00:00.000Z',
                    },
                };
            }
            throw new Error(`Unexpected GET ${url}`);
        });

        const { result } = renderHook(() => useWallet());

        await act(async () => {
            await result.current.connectWithMnemonic('seed words');
        });

        expect(apiClient.post).not.toHaveBeenCalledWith('/auth/logout');
        expect(useAuthStore.getState().user?.id).toBe('user-a');
        expect(useAuthStore.getState().user?.faceit_id).toBe('faceit-user-a');
        expect(useAuthStore.getState().user?.kaspa_address).toBe('kaspatest:qnewwallet');
        expect(useAuthStore.getState().isFaceitConnected).toBe(true);
        expect(useAuthStore.getState().walletConnected).toBe(true);
        expect(useAuthStore.getState().isFullyConnected).toBe(true);
        expect(useLobbyStore.getState().lobbies).toEqual([]);
        expect(useMatchStore.getState().currentMatch).toBeNull();
        expect(useMatchStore.getState().localDeposit).toBeNull();
        expect(useWalletStore.getState().address).toBe('kaspatest:qnewwallet');
    });
});

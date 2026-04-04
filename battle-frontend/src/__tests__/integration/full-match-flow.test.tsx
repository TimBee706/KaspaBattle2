import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import App from '../../App';

const { apiClient } = vi.hoisted(() => ({
    apiClient: {
        get: vi.fn(),
        post: vi.fn(),
    },
}));

vi.mock('../../api/client', () => ({
    default: apiClient,
}));

// Mock the wallet module so we don't actually hit WASM/Network in this test
vi.mock('../../kaspa/wallet', () => ({
    importWallet: vi.fn().mockResolvedValue({
        connection: {
            wallet: null,
            account: {
                receiveAddress: 'kaspatest:qmockaddress123',
                escrowAddress: 'kaspatest:qescrow',
                xpub: 'xpub-test',
                publicKey: 'pub-key',
            },
            address: 'kaspatest:qmockaddress123',
        },
        ephemeralPrivateKeyHex: 'ephemeral-priv-key',
    }),
    getBalance: vi.fn().mockResolvedValue(50_000_000),
    getBalanceByAddress: vi.fn().mockResolvedValue(5_000_000),
    getRpcClient: vi.fn().mockResolvedValue({
        subscribeUtxosChanged: vi.fn().mockResolvedValue(undefined),
        addEventListener: vi.fn(),
    }),
    onBalanceChange: vi.fn(),
    sendDeposit: vi.fn().mockResolvedValue('mock-tx-id'),
}));

vi.mock('kaspa-wasm', async () => {
    const actual = await vi.importActual('kaspa-wasm');
    return {
        ...actual,
        signMessage: vi.fn().mockResolvedValue('signed-message'),
    };
});

vi.mock('../../hooks/useKaspaInit', () => ({
    useKaspaInit: vi.fn().mockReturnValue({ isReady: true, error: null }),
}));

describe('Full Match Flow Integration', () => {
    it('should handle wallet import and mock deposit', async () => {
        apiClient.post.mockImplementation(async (url: string) => {
            if (url === '/auth/logout') return { data: null };
            if (url === '/auth/wallet-challenge') {
                return {
                    data: {
                        challenge_id: 'challenge-1',
                        message: 'sign me',
                        expires_at: '2026-03-22T00:05:00.000Z',
                    },
                };
            }
            if (url === '/auth/wallet-verify') return { data: {} };
            throw new Error(`Unexpected POST ${url}`);
        });

        apiClient.get.mockImplementation(async (url: string) => {
            if (url === '/auth/me') {
                return {
                    data: {
                        id: 'user-wallet',
                        faceit_id: '',
                        faceit_nickname: '',
                        faceit_connected: false,
                        display_name: 'Wallet User',
                        faceit_avatar: '',
                        faceit_elo: null,
                        faceit_skill_level: null,
                        kaspa_address: 'kaspatest:qmockaddress123',
                        total_matches: 0,
                        wins: 0,
                        losses: 0,
                        total_wagered_sompi: 0,
                        total_won_sompi: 0,
                        created_at: '2026-03-22T00:00:00.000Z',
                    },
                };
            }
            if (url === '/lobbies') return { data: [] };
            throw new Error(`Unexpected GET ${url}`);
        });

        window.history.pushState({}, 'Test', '/wallet/import');

        render(<App />);

        let importInput: HTMLElement;
        await waitFor(() => {
            importInput = screen.getByPlaceholderText('word1 word2 word3...');
            expect(importInput).toBeInTheDocument();
        });
        const importBtn = screen.getByText('CONNECT WALLET');

        fireEvent.change(importInput!, { target: { value: 'test test test test test test test test test test test test' } });
        fireEvent.click(importBtn);

        await waitFor(() => {
            expect(screen.getByText(/0[,.]05/i)).toBeInTheDocument();
            expect(screen.queryByText('CONNECT WALLET')).not.toBeInTheDocument();
        });
    });
});

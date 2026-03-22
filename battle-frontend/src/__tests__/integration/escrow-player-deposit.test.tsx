import { beforeEach, describe, expect, it, vi } from 'vitest';
import { MemoryRouter, Route, Routes } from 'react-router-dom';
import { render, screen } from '@testing-library/react';
import { EscrowPage } from '../../pages/EscrowPage';
import { useAuthStore } from '../../stores/useAuthStore';
import { useMatchStore } from '../../stores/useMatchStore';
import { useWalletStore } from '../../stores/useWalletStore';

vi.mock('react-i18next', () => ({
    useTranslation: () => ({ t: (key: string) => key }),
}));

vi.mock('../../hooks/useMatchPolling', () => ({
    useMatchPolling: vi.fn(),
}));

vi.mock('../../hooks/usePaymentStatus', () => ({
    usePaymentStatus: vi.fn(),
}));

vi.mock('../../hooks/useEscrowDeposit', () => ({
    useEscrowDeposit: () => ({
        executeDeposit: vi.fn(),
        isDepositing: false,
        depositTxHash: null,
    }),
}));

describe('EscrowPage player-specific deposit status', () => {
    beforeEach(() => {
        useAuthStore.getState().clearAuthState();
        useMatchStore.getState().clearMatch();
        useWalletStore.getState().disconnect();
    });

    it('shows deposit CTA for player B when only player A has paid', () => {
        useAuthStore.setState({
            user: {
                id: 'user-b',
                faceit_id: '',
                faceit_nickname: '',
                faceit_connected: false,
                display_name: 'Player B',
                faceit_avatar: '',
                faceit_elo: null,
                faceit_skill_level: null,
                kaspa_address: 'kaspatest:qb',
                total_matches: 0,
                wins: 0,
                losses: 0,
                total_wagered_sompi: 0,
                total_won_sompi: 0,
                created_at: '2026-03-22T00:00:00.000Z',
            },
            isAuthenticated: true,
            walletConnected: true,
            isFaceitConnected: false,
            testMode: false,
        });

        useWalletStore.setState({
            isConnected: true,
            isConnecting: false,
            wallet: null,
            account: { mnemonic: 'test' },
            address: 'kaspatest:qb',
            walletType: 'mnemonic',
            balanceSompi: 2_000_000_000,
            isFetchingBalance: false,
            balanceError: null,
            error: null,
        });

        useMatchStore.setState({
            currentMatch: {
                id: 'match-1',
                creator_user_id: 'user-a',
                opponent_user_id: 'user-b',
                player_a_kas_address: 'kaspatest:qa',
                player_b_kas_address: 'kaspatest:qb',
                player_a_faceit_id: '',
                player_b_faceit_id: '',
                player_a_faceit_nickname: 'Player A',
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
            localDeposit: null,
            isDepositing: false,
            paymentStatus: {
                escrow_address: 'kaspatest:qescrow',
                required_per_player_sompi: 1_000_000_000,
                min_confirmations_required: 10,
                playerA: { paid: true, confirmed_sompi: 1_000_000_000, payment_count: 1, min_confirmations: 12 },
                playerB: { paid: false, confirmed_sompi: 0, payment_count: 0, min_confirmations: 0 },
                both_paid: false,
            },
        });

        render(
            <MemoryRouter initialEntries={['/escrow/match-1']}>
                <Routes>
                    <Route path="/escrow/:lobbyId" element={<EscrowPage />} />
                </Routes>
            </MemoryRouter>,
        );

        expect(screen.getByRole('button', { name: 'escrow.deposit_now' })).toBeEnabled();
        expect(screen.queryByText('escrow.waiting_opponent')).not.toBeInTheDocument();
        expect(screen.queryByText('deposit.success_title')).not.toBeInTheDocument();
    });
});

import { beforeEach, describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { MemoryRouter } from 'react-router-dom';
import { MatchDetailView } from '../../components/match/MatchDetailView';
import { useAuthStore } from '../../stores/useAuthStore';
import { useMatchStore } from '../../stores/useMatchStore';
import type { BattleMatch } from '../../api/types';

const { acceptMatch, getMatch, submitFaceitMatchId } = vi.hoisted(() => ({
    acceptMatch: vi.fn(),
    getMatch: vi.fn(),
    submitFaceitMatchId: vi.fn(),
}));

vi.mock('react-i18next', async (importOriginal) => {
    const actual = await importOriginal<typeof import('react-i18next')>();
    return {
        ...actual,
        useTranslation: () => ({ t: (key: string) => key }),
    };
});

vi.mock('../../api/matches', () => ({
    acceptMatch,
    getMatch,
    submitFaceitMatchId,
}));

vi.mock('../../components/match/DepositConfirmModal', () => ({
    DepositConfirmModal: () => null,
}));

const baseMatch: BattleMatch = {
    id: 'match-1',
    creator_user_id: 'user-a',
    opponent_user_id: 'user-b',
    player_a_kas_address: 'kaspatest:qa',
    player_b_kas_address: 'kaspatest:qb',
    player_a_faceit_id: 'faceit-a',
    player_b_faceit_id: 'faceit-b',
    player_a_faceit_nickname: 'Player A',
    player_b_faceit_nickname: 'Player B',
    faceit_match_id: null,
    faceit_match_id_player_a: null,
    faceit_match_id_player_b: null,
    faceit_match_id_final: null,
    faceit_match_status: null,
    wager_amount_sompi: 1_000_000_000,
    escrow_address: 'kaspatest:qescrow',
    status: 'GAME_ID_INPUT',
    game_id: 'cs2',
    match_mode: 'BO1',
    winner_kas_address: null,
    winner_faceit_nickname: null,
    payout_tx_hash: null,
    score: null,
    player_a_deposit_tx_hash: 'tx-a',
    player_b_deposit_tx_hash: 'tx-b',
    created_at: '2026-03-22T00:00:00.000Z',
    locked_at: null,
    resolved_at: null,
    timeout_at: '2026-03-22T01:30:00.000Z',
};

describe('MatchDetailView FACEIT match ID input', () => {
    beforeEach(() => {
        acceptMatch.mockReset();
        getMatch.mockReset();
        submitFaceitMatchId.mockReset();
        useAuthStore.getState().clearAuthState();
        useMatchStore.getState().clearMatch();

        useAuthStore.setState({
            user: {
                id: 'user-a',
                faceit_id: 'faceit-a',
                faceit_nickname: 'Player A',
                faceit_connected: true,
                display_name: 'Player A',
                faceit_avatar: '',
                faceit_elo: null,
                faceit_skill_level: null,
                kaspa_address: 'kaspatest:qa',
                total_matches: 0,
                wins: 0,
                losses: 0,
                total_wagered_sompi: 0,
                total_won_sompi: 0,
                created_at: '2026-03-22T00:00:00.000Z',
            },
            isAuthenticated: true,
            walletConnected: true,
            isFaceitConnected: true,
            isFullyConnected: true,
            testMode: false,
        });
    });

    it('submits a FACEIT match ID and refreshes the match state', async () => {
        const submittedId = '550e8400-e29b-41d4-a716-446655440000';

        submitFaceitMatchId.mockResolvedValue({
            status: 'submitted',
            both_submitted: false,
            match_status: 'GAME_ID_INPUT',
            message: 'Waiting',
        });

        getMatch.mockResolvedValue({
            ...baseMatch,
            faceit_match_id_player_a: submittedId,
        });

        render(
            <MemoryRouter>
                <MatchDetailView match={baseMatch} />
            </MemoryRouter>,
        );

        fireEvent.change(screen.getByPlaceholderText('match.faceit_input_placeholder'), {
            target: { value: submittedId },
        });
        fireEvent.click(screen.getByRole('button', { name: 'match.faceit_submit' }));

        await waitFor(() => {
            expect(submitFaceitMatchId).toHaveBeenCalledWith({
                match_id: 'match-1',
                faceit_match_id: submittedId,
            });
        });

        await waitFor(() => {
            expect(getMatch).toHaveBeenCalledWith('match-1');
            expect(useMatchStore.getState().currentMatch?.faceit_match_id_player_a).toBe(submittedId);
        });

        expect(await screen.findByText('match.faceit_submit_waiting')).toBeInTheDocument();
    });

    it('shows a validation error for an invalid FACEIT match ID without submitting', async () => {
        render(
            <MemoryRouter>
                <MatchDetailView match={baseMatch} />
            </MemoryRouter>,
        );

        fireEvent.change(screen.getByPlaceholderText('match.faceit_input_placeholder'), {
            target: { value: 'invalid-id' },
        });
        fireEvent.click(screen.getByRole('button', { name: 'match.faceit_submit' }));

        expect(submitFaceitMatchId).not.toHaveBeenCalled();
        expect(await screen.findByText('Please enter a valid FACEIT match ID in UUID format.')).toBeInTheDocument();
    });
});

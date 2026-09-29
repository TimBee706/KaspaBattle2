import { beforeEach, describe, expect, it, vi } from 'vitest';
import { MemoryRouter } from 'react-router-dom';
import { render, screen } from '@testing-library/react';
import i18n from '../../i18n';
import { MatchPage } from '../../pages/MatchPage';
import { AuthGuard } from '../../components/auth/AuthGuard';
import { useAuthStore } from '../../stores/useAuthStore';
import { useMatchStore } from '../../stores/useMatchStore';
import { useWalletStore } from '../../stores/useWalletStore';
import { ALICE, BOB, MATCH_ID, makeMatch } from '../helpers/nativeFixtures';
import { Routes, Route } from 'react-router-dom';

const { getMatch } = vi.hoisted(() => ({ getMatch: vi.fn() }));
vi.mock('../../api/matches', () => ({
    getMatch,
    getPaymentStatus: vi.fn().mockResolvedValue(null),
    acceptMatch: vi.fn(),
    requestRefund: vi.fn(),
    submitFaceitMatchId: vi.fn(),
    submitDeposit: vi.fn(),
}));
vi.mock('../../api/nativeGames', async () => {
    const { makeSnapshot: snapshot } = await import('../helpers/nativeFixtures');
    return {
        getGame: vi.fn().mockResolvedValue(snapshot()),
        startGame: vi.fn(),
        postConnectFourMove: vi.fn(),
        resignGame: vi.fn(),
        getFeatures: vi.fn().mockResolvedValue({ nativeGamesEnabled: true }),
    };
});
vi.mock('../../hooks/usePaymentStatus', () => ({ usePaymentStatus: vi.fn() }));

class MockWebSocket {
    onmessage: unknown = null;
    onopen: unknown = null;
    close = vi.fn();
}

function user(id: string, faceit = false) {
    return {
        id, faceit_id: faceit ? 'f1' : '', faceit_nickname: faceit ? 'nick' : '', faceit_connected: faceit,
        display_name: id === ALICE ? 'Alice' : 'Bob', faceit_avatar: '', faceit_elo: null, faceit_skill_level: null,
        kaspa_address: `kaspatest:${id}`, total_matches: 0, wins: 0, losses: 0, total_wagered_sompi: 0,
        total_won_sompi: 0, created_at: '2026-01-01T00:00:00Z',
    } as never;
}

function renderMatch() {
    return render(
        <MemoryRouter initialEntries={[`/match/${MATCH_ID}`]}>
            <Routes>
                <Route path="/match/:matchId" element={<AuthGuard><MatchPage /></AuthGuard>} />
            </Routes>
        </MemoryRouter>,
    );
}

describe('MatchPage renders by provider', () => {
    beforeEach(async () => {
        await i18n.changeLanguage('en');
        vi.stubGlobal('WebSocket', MockWebSocket);
        useAuthStore.getState().clearAuthState();
        useAuthStore.setState({ isAuthLoading: false });
        useMatchStore.getState().clearMatch();
        useWalletStore.setState({ isConnected: true, address: 'kaspatest:x' });
    });

    it('NATIVE → NativeMatchView + board; no FACEIT prompts for a FACEIT-free player', async () => {
        useAuthStore.setState({ testMode: false });
        useAuthStore.getState().setAuth(user(ALICE, false));
        getMatch.mockResolvedValue(makeMatch({ status: 'IN_GAME' }));
        renderMatch();

        expect(await screen.findByTestId('native-match-view')).toBeInTheDocument();
        expect(await screen.findByTestId('native-game')).toBeInTheDocument();
        expect(screen.getByRole('group', { name: 'Connect Four board' })).toBeInTheDocument();
        expect(screen.getByText('KaspaBattle (Browser)')).toBeInTheDocument();
        // the auth banner nags about FACEIT only for FACEIT matches
        expect(screen.queryByText(/FaceIT/i)).not.toBeInTheDocument();
        expect(screen.queryByText(/FACEIT match ID/i)).not.toBeInTheDocument();
    });

    it('a FACEIT-free visitor sees "accept" on an open browser challenge; a stranger without wallet does not', async () => {
        useAuthStore.getState().setAuth(user(BOB, false));
        getMatch.mockResolvedValue(makeMatch({ status: 'OPEN', opponent_user_id: null, player_b_display_name: null }));
        renderMatch();
        expect(await screen.findByRole('button', { name: 'ACCEPT CHALLENGE' })).toBeInTheDocument();
        expect(screen.queryByTestId('native-game')).not.toBeInTheDocument();
    });

    it('FACEIT (and legacy provider-less) matches keep the existing FACEIT flow', async () => {
        useAuthStore.getState().setAuth(user(ALICE, true));
        const legacy = makeMatch({ status: 'GAME_ID_INPUT', game_id: 'cs2' });
        delete (legacy as Partial<typeof legacy>).provider;
        getMatch.mockResolvedValue(legacy);
        renderMatch();

        expect(await screen.findByText('FACEIT match ID required')).toBeInTheDocument();
        expect(screen.queryByTestId('native-match-view')).not.toBeInTheDocument();
        expect(screen.queryByRole('group', { name: 'Connect Four board' })).not.toBeInTheDocument();
    });

    it('a FACEIT match still nags a FACEIT-free user about FACEIT', async () => {
        useAuthStore.setState({ testMode: false });
        useAuthStore.getState().setAuth(user(ALICE, false));
        getMatch.mockResolvedValue(makeMatch({ status: 'OPEN', opponent_user_id: null, provider: 'FACEIT', game_id: 'cs2' }));
        renderMatch();
        expect(await screen.findByText(/FaceIT-Konto/i)).toBeInTheDocument();
    });

    it('draw → refund pending copy, no payout button', async () => {
        useAuthStore.getState().setAuth(user(ALICE, false));
        getMatch.mockResolvedValue(makeMatch({ status: 'REFUND_PENDING' }));
        renderMatch();
        expect(await screen.findByText(/Both stakes are being refunded/)).toBeInTheDocument();
        expect(screen.queryByRole('button', { name: 'CLAIM PAYOUT' })).not.toBeInTheDocument();
    });

    it('only the engine winner gets the claim-payout action', async () => {
        useAuthStore.getState().setAuth(user(ALICE, false));
        getMatch.mockResolvedValue(makeMatch({ status: 'READY_FOR_PAYOUT', winner_user_id: ALICE, loser_user_id: BOB }));
        const { unmount } = renderMatch();
        expect(await screen.findByRole('button', { name: 'CLAIM PAYOUT' })).toBeEnabled();
        unmount();

        useMatchStore.getState().clearMatch();
        useAuthStore.getState().setAuth(user(BOB, false));
        getMatch.mockResolvedValue(makeMatch({ status: 'READY_FOR_PAYOUT', winner_user_id: ALICE, loser_user_id: BOB }));
        renderMatch();
        expect(await screen.findByText('Waiting for the winner to claim the payout.')).toBeInTheDocument();
        expect(screen.queryByRole('button', { name: 'CLAIM PAYOUT' })).not.toBeInTheDocument();
    });
});

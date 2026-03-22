import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { MemoryRouter, Route, Routes } from 'react-router-dom';
import { render, screen, waitFor, within } from '@testing-library/react';
import { LobbyPage } from '../../pages/LobbyPage';
import { useAuthStore } from '../../stores/useAuthStore';
import { useLobbyStore } from '../../stores/useLobbyStore';

vi.mock('react-i18next', () => ({
    useTranslation: () => ({ t: (key: string) => key }),
}));

const { apiGet } = vi.hoisted(() => ({
    apiGet: vi.fn(),
}));

vi.mock('../../api/client', () => ({
    default: {
        get: apiGet,
        post: vi.fn(),
    },
}));

class MockWebSocket {
    onmessage: ((event: MessageEvent) => void) | null = null;
    close = vi.fn();
}

const oldWebSocket = globalThis.WebSocket;

describe('LobbyPage identity-aware filtering', () => {
    beforeEach(() => {
        apiGet.mockReset();
        useAuthStore.getState().clearAuthState();
        useLobbyStore.getState().reset();
        // @ts-expect-error test stub
        globalThis.WebSocket = MockWebSocket;
    });

    afterEach(() => {
        globalThis.WebSocket = oldWebSocket;
    });

    it('shows A-created open lobby as available challenge for player B', async () => {
        const walletUserB = {
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
        };

        const openLobby = {
            id: 'match-1',
            creator_user_id: 'user-a',
            opponent_user_id: null,
            player_a_kas_address: 'kaspatest:qa',
            player_b_kas_address: null,
            player_a_faceit_id: '',
            player_b_faceit_id: null,
            player_a_faceit_nickname: 'Player A',
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
        };

        apiGet.mockImplementation(async (url: string) => {
            if (url === '/auth/me') return { data: walletUserB };
            if (url === '/lobbies') return { data: [openLobby] };
            throw new Error(`Unexpected GET ${url}`);
        });

        render(
            <MemoryRouter initialEntries={['/lobby']}>
                <Routes>
                    <Route path="/lobby" element={<LobbyPage />} />
                </Routes>
            </MemoryRouter>,
        );

        await waitFor(() => {
            expect(screen.getByRole('button', { name: 'Join' })).toBeInTheDocument();
        });

        const mySection = screen.getByText('lobby.my_lobbies').parentElement?.parentElement;
        const availableSection = screen.getByText('lobby.available_challenges').parentElement?.parentElement;

        expect(mySection).not.toBeNull();
        expect(availableSection).not.toBeNull();
        expect(within(mySection as HTMLElement).getByText('lobby.no_entries')).toBeInTheDocument();
        expect(within(availableSection as HTMLElement).getByRole('button', { name: 'Join' })).toBeInTheDocument();
    });
});

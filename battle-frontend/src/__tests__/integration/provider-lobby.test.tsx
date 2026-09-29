import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { MemoryRouter, Route, Routes } from 'react-router-dom';
import { render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import i18n from '../../i18n';
import { LobbyPage } from '../../pages/LobbyPage';
import { useAuthStore } from '../../stores/useAuthStore';
import { useLobbyStore } from '../../stores/useLobbyStore';
import { makeMatch } from '../helpers/nativeFixtures';

const { apiGet } = vi.hoisted(() => ({ apiGet: vi.fn() }));
vi.mock('../../api/client', () => ({ default: { get: apiGet, post: vi.fn() } }));

class MockWebSocket {
    onmessage: ((event: MessageEvent) => void) | null = null;
    close = vi.fn();
}
const oldWebSocket = globalThis.WebSocket;

const walletOnlyUser = {
    id: 'user-c', faceit_id: '', faceit_nickname: '', faceit_connected: false, display_name: 'Carol',
    faceit_avatar: '', faceit_elo: null, faceit_skill_level: null, kaspa_address: 'kaspatest:qc',
    total_matches: 0, wins: 0, losses: 0, total_wagered_sompi: 0, total_won_sompi: 0, created_at: '2026-01-01T00:00:00Z',
};

const nativeLobby = makeMatch({
    id: 'native-1', status: 'OPEN', opponent_user_id: null, creator_user_id: 'user-a',
    player_a_display_name: 'Alice', player_b_display_name: null, player_b_deposit_tx_hash: null,
});
const faceitLobby = makeMatch({
    id: 'faceit-1', status: 'OPEN', opponent_user_id: null, creator_user_id: 'user-b', game_id: 'cs2',
    provider: 'FACEIT', requires_faceit: true, native_game_type: null,
    player_a_faceit_nickname: 'FaceitFan', player_a_display_name: 'Bob', player_b_display_name: null,
});
// legacy row exactly as an old backend returns it: no provider fields at all
const legacyLobby = (() => {
    const m = makeMatch({ id: 'legacy-1', status: 'OPEN', opponent_user_id: null, creator_user_id: 'user-b', game_id: 'valorant', player_a_faceit_nickname: 'OldSchool' });
    delete (m as Partial<typeof m>).provider;
    delete (m as Partial<typeof m>).requires_faceit;
    delete (m as Partial<typeof m>).native_game_type;
    return m;
})();

function renderLobby() {
    return render(
        <MemoryRouter initialEntries={['/lobby']}>
            <Routes><Route path="/lobby" element={<LobbyPage />} /></Routes>
        </MemoryRouter>,
    );
}

describe('Lobby: provider labels, filters and join rules', () => {
    beforeEach(async () => {
        await i18n.changeLanguage('en');
        apiGet.mockReset();
        useAuthStore.getState().clearAuthState();
        useLobbyStore.getState().reset();
        // @ts-expect-error test stub
        globalThis.WebSocket = MockWebSocket;
        apiGet.mockImplementation(async (url: string) => {
            if (url === '/auth/me') return { data: walletOnlyUser };
            if (url === '/lobbies') return { data: [nativeLobby, faceitLobby, legacyLobby] };
            throw new Error(`Unexpected GET ${url}`);
        });
    });
    afterEach(() => {
        globalThis.WebSocket = oldWebSocket;
    });

    it('labels every card unambiguously (BROWSER GAME · … / FACEIT · …), legacy rows count as FACEIT', async () => {
        renderLobby();
        expect(await screen.findByText('BROWSER GAME · CONNECT FOUR')).toBeInTheDocument();
        expect(screen.getByText('FACEIT · COUNTER-STRIKE 2')).toBeInTheDocument();
        expect(screen.getByText('FACEIT · VALORANT')).toBeInTheDocument();
        expect(screen.getByText('Alice vs TBD')).toBeInTheDocument();
    });

    it('filters All / Browser Games / FACEIT', async () => {
        renderLobby();
        await screen.findByText('BROWSER GAME · CONNECT FOUR');
        const group = screen.getByRole('group', { name: 'Filter by game type' });

        await userEvent.click(within(group).getByRole('button', { name: 'Browser Games' }));
        expect(screen.getByText('BROWSER GAME · CONNECT FOUR')).toBeInTheDocument();
        expect(screen.queryByText('FACEIT · COUNTER-STRIKE 2')).not.toBeInTheDocument();
        expect(screen.queryByText('FACEIT · VALORANT')).not.toBeInTheDocument();

        await userEvent.click(within(group).getByRole('button', { name: 'FACEIT' }));
        expect(screen.queryByText('BROWSER GAME · CONNECT FOUR')).not.toBeInTheDocument();
        expect(screen.getByText('FACEIT · COUNTER-STRIKE 2')).toBeInTheDocument();
        expect(screen.getByText('FACEIT · VALORANT')).toBeInTheDocument();

        await userEvent.click(within(group).getByRole('button', { name: 'All' }));
        expect(screen.getAllByText(/^(BROWSER GAME|FACEIT) · /)).toHaveLength(3);
        expect(within(group).getByRole('button', { name: 'All' })).toHaveAttribute('aria-pressed', 'true');
    });

    it('a wallet-only user (no FACEIT) may join browser games but not FACEIT games', async () => {
        renderLobby();
        await screen.findByText('BROWSER GAME · CONNECT FOUR');
        await waitFor(() => expect(useAuthStore.getState().isAuthenticated).toBe(true));

        const joinButtons = await screen.findAllByRole('button', { name: 'Accept Challenge' });
        expect(joinButtons).toHaveLength(3);
        const cardOf = (label: string) => screen.getByText(label).closest('[role="button"]') as HTMLElement;
        await waitFor(() => {
            expect(within(cardOf('BROWSER GAME · CONNECT FOUR')).getByRole('button', { name: 'Accept Challenge' })).toBeEnabled();
        });
        expect(within(cardOf('FACEIT · COUNTER-STRIKE 2')).getByRole('button', { name: 'Accept Challenge' })).toBeDisabled();
        expect(within(cardOf('FACEIT · VALORANT')).getByRole('button', { name: 'Accept Challenge' })).toBeDisabled();
    });
});

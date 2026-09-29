import { beforeEach, describe, expect, it, vi } from 'vitest';
import { MemoryRouter } from 'react-router-dom';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import i18n from '../../i18n';
import { CreateMatchPage } from '../../pages/CreateMatchPage';
import { useAuthStore } from '../../stores/useAuthStore';
import { useWalletStore } from '../../stores/useWalletStore';
import { __setNativeGamesEnabledForTests } from '../../hooks/useNativeGamesEnabled';

vi.mock('../../api/nativeGames', () => ({
    getFeatures: vi.fn().mockResolvedValue({ nativeGamesEnabled: false }),
}));

function renderPage(entry = '/lobby/create') {
    return render(
        <MemoryRouter initialEntries={[entry]}>
            <CreateMatchPage />
        </MemoryRouter>,
    );
}

const walletUser = {
    id: 'u1', faceit_id: '', faceit_nickname: '', faceit_connected: false, display_name: 'Wallet Only',
    faceit_avatar: '', faceit_elo: null, faceit_skill_level: null, kaspa_address: 'kaspatest:qw',
    total_matches: 0, wins: 0, losses: 0, total_wagered_sompi: 0, total_won_sompi: 0, created_at: '2026-01-01T00:00:00Z',
};

describe('CreateMatchPage: play on KaspaBattle vs. via FACEIT', () => {
    beforeEach(async () => {
        await i18n.changeLanguage('en');
        useAuthStore.getState().clearAuthState();
        useAuthStore.getState().setAuth(walletUser as never);
        useAuthStore.setState({ isAuthLoading: false });
        useWalletStore.setState({ isConnected: true, address: walletUser.kaspa_address });
        __setNativeGamesEnabledForTests(true);
    });

    it('offers both providers; FACEIT keeps the existing games and best-of mode', () => {
        renderPage();
        expect(screen.getByRole('button', { name: /Play on KaspaBattle/ })).toBeEnabled();
        expect(screen.getByRole('button', { name: /Play via FACEIT/ })).toHaveAttribute('aria-pressed', 'true');
        for (const game of ['Counter-Strike 2', 'Valorant', 'Rocket League', 'Dota 2', 'League of Legends']) {
            expect(screen.getByRole('button', { name: new RegExp(game) })).toBeInTheDocument();
        }
        expect(screen.getByLabelText('Mode')).toBeInTheDocument();
        // a wallet-only user cannot publish a FACEIT challenge
        expect(screen.getByRole('button', { name: 'PUBLISH CHALLENGE' })).toBeDisabled();
    });

    it('under KaspaBattle only Connect Four is shown: 2 players, in the browser, no FACEIT required', async () => {
        renderPage();
        await userEvent.click(screen.getByRole('button', { name: /Play on KaspaBattle/ }));

        expect(screen.getByRole('button', { name: /Connect Four/ })).toBeInTheDocument();
        expect(screen.getByText('2 players · In the browser')).toBeInTheDocument();
        expect(screen.getByTestId('native-no-faceit-note')).toHaveTextContent('No FACEIT account required');
        expect(screen.queryByRole('button', { name: /Counter-Strike 2/ })).not.toBeInTheDocument();
        expect(screen.queryByLabelText('Mode')).not.toBeInTheDocument(); // best-of-1 only

        // the same wallet-only user CAN publish a browser game challenge
        expect(screen.getByRole('button', { name: 'PUBLISH CHALLENGE' })).toBeEnabled();
    });

    it('preselects browser games with ?provider=native', () => {
        renderPage('/lobby/create?provider=native');
        expect(screen.getByRole('button', { name: /Play on KaspaBattle/ })).toHaveAttribute('aria-pressed', 'true');
        expect(screen.getByRole('button', { name: /Connect Four/ })).toBeInTheDocument();
    });

    it('hides the browser option (and falls back to FACEIT) while the feature flag is off', () => {
        __setNativeGamesEnabledForTests(false);
        renderPage('/lobby/create?provider=native');
        expect(screen.getByRole('button', { name: /Play on KaspaBattle/ })).toBeDisabled();
        expect(screen.getByTestId('native-disabled-note')).toBeInTheDocument();
        expect(screen.getByRole('button', { name: /Play via FACEIT/ })).toHaveAttribute('aria-pressed', 'true');
        expect(screen.getByRole('button', { name: /Counter-Strike 2/ })).toBeInTheDocument();
    });

    it('a logged-out visitor cannot publish anything', async () => {
        useAuthStore.getState().clearAuthState();
        useAuthStore.setState({ isAuthLoading: false });
        useWalletStore.setState({ isConnected: false, address: null });
        renderPage();
        await userEvent.click(screen.getByRole('button', { name: /Play on KaspaBattle/ }));
        expect(screen.getByRole('button', { name: 'PUBLISH CHALLENGE' })).toBeDisabled();
    });
});

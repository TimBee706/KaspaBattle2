import { beforeEach, describe, expect, it, vi } from 'vitest';
import { MemoryRouter } from 'react-router-dom';
import { render, screen, within } from '@testing-library/react';
import i18n from '../../i18n';
import { LandingPage } from '../../pages/LandingPage';
import { __setNativeGamesEnabledForTests } from '../../hooks/useNativeGamesEnabled';

vi.mock('../../api/nativeGames', () => ({ getFeatures: vi.fn().mockResolvedValue({ nativeGamesEnabled: true }) }));

function renderLanding() {
    return render(<MemoryRouter><LandingPage /></MemoryRouter>);
}

describe('LandingPage: Browser Games and FACEIT Games', () => {
    beforeEach(async () => {
        await i18n.changeLanguage('en');
        __setNativeGamesEnabledForTests(true);
    });

    it('communicates both variants and that both use Kaspa escrow + settlement', () => {
        renderLanding();
        const browser = screen.getByTestId('landing-browser-games');
        const faceit = screen.getByTestId('landing-faceit-games');

        expect(within(browser).getByRole('heading', { name: 'Browser Games' })).toBeInTheDocument();
        expect(within(browser).getByText(/No FACEIT account required/)).toBeInTheDocument();
        expect(within(browser).getByText('No FACEIT needed — just your Kaspa wallet')).toBeInTheDocument();
        expect(within(browser).getByRole('link', { name: /Play Connect Four/ })).toHaveAttribute('href', '/lobby/create?provider=native');

        expect(within(faceit).getByRole('heading', { name: 'FACEIT Games' })).toBeInTheDocument();
        expect(within(faceit).getByText('Requires a linked FACEIT account')).toBeInTheDocument();
        expect(within(faceit).getByRole('link', { name: /Browse FACEIT matches/ })).toHaveAttribute('href', '/lobby');

        expect(screen.getByText('Both variants use Kaspa-based escrow and settlement.')).toBeInTheDocument();
    });

    it('still shows the FACEIT badge and adds the browser badge in the hero', () => {
        renderLanding();
        expect(screen.getByText(/Built for FACEIT matches/i)).toBeInTheDocument();
        expect(screen.getByText('Play in your browser')).toBeInTheDocument();
    });

    it('does not offer a dead CTA while browser games are disabled on the server', () => {
        __setNativeGamesEnabledForTests(false);
        renderLanding();
        const browser = screen.getByTestId('landing-browser-games');
        expect(within(browser).queryByRole('link')).not.toBeInTheDocument();
        expect(within(browser).getByText('Not enabled on this server yet')).toBeInTheDocument();
    });

    it('is available in German', async () => {
        await i18n.changeLanguage('de');
        renderLanding();
        expect(screen.getByText('Vier Gewinnt für 2 Spieler')).toBeInTheDocument();
        expect(screen.getByText('Beide Varianten nutzen Kaspa-basiertes Escrow und Settlement.')).toBeInTheDocument();
    });
});

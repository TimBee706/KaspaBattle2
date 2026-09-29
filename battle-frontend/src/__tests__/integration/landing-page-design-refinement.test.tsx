import { describe, it, expect, beforeEach } from 'vitest';
import { render, screen } from '@testing-library/react';
import { MemoryRouter } from 'react-router-dom';
import i18n from '../../i18n';
import { LandingPage } from '../../pages/LandingPage';

// Uses the REAL i18n instance (same pattern as unit/testnet-banner.test.tsx)
// so these assertions exercise the actual shipped EN/DE copy, not mocked
// translation keys — important here since several checks (FACEIT wording,
// "no official partner" phrasing) are about the real content itself.
describe('LandingPage — design refinement (FACEIT integration, glow cleanup)', () => {
    beforeEach(async () => {
        await i18n.changeLanguage('en');
    });

    function renderLanding() {
        return render(
            <MemoryRouter initialEntries={['/']}>
                <LandingPage />
            </MemoryRouter>
        );
    }

    it('renders without crashing and without any wallet/auth setup', () => {
        renderLanding();
        expect(screen.getByRole('heading', { level: 1 })).toBeInTheDocument();
    });

    it('names FACEIT visibly in the hero, above the fold, without claiming an unverified official partnership', () => {
        renderLanding();
        expect(screen.getByText(/Built for FACEIT matches/i)).toBeInTheDocument();
        expect(screen.queryByText(/official faceit partner/i)).not.toBeInTheDocument();
        expect(screen.queryByText(/powered by faceit/i)).not.toBeInTheDocument();
        expect(screen.queryByText(/in partnership with faceit/i)).not.toBeInTheDocument();
    });

    it('keeps the existing "Public Testnet Beta" warning next to the new FACEIT badge', () => {
        renderLanding();
        expect(screen.getByText('Public Testnet Beta')).toBeInTheDocument();
        expect(screen.getByText(/Built for FACEIT matches/i)).toBeInTheDocument();
    });

    it('renders "What is KaspaBattle?" without the previous large card wrapper', () => {
        renderLanding();
        const heading = screen.getByText('WHAT IS KASPABATTLE?');
        const section = heading.closest('section');
        expect(section).not.toBeNull();
        expect(section!.querySelector('.glass-panel')).toBeNull();
    });

    it('gives all six "how it works" steps identical, glow-free base styling (step 1 not singled out)', () => {
        const { container } = renderLanding();
        const stepsSection = container.querySelector('#how');
        expect(stepsSection).not.toBeNull();

        const stepCards = stepsSection!.querySelectorAll('.group');
        expect(stepCards).toHaveLength(6);

        const distinctClassNames = new Set(Array.from(stepCards).map((el) => el.className));
        expect(distinctClassNames.size).toBe(1);

        stepCards.forEach((card) => {
            expect(card.className).not.toContain('shadow-glow-primary');
        });
    });

    it('adds a semantic overline above each "Why Kaspa?" card title', () => {
        renderLanding();
        expect(screen.getByText('Settlement')).toBeInTheDocument();
        expect(screen.getByText('Performance')).toBeInTheDocument();
        expect(screen.getByText('Efficiency')).toBeInTheDocument();
        expect(screen.getByText('Network')).toBeInTheDocument();
    });

    it('removes the surrounding glow from the closing "Future of Esports" CTA in favor of an inner accent', () => {
        renderLanding();
        const heading = screen.getByText('THE FUTURE OF ESPORTS');
        const ctaSection = heading.closest('section');
        expect(ctaSection).not.toBeNull();
        expect(ctaSection!.className).not.toMatch(/shadow-glow/);

        const innerAccent = ctaSection!.querySelector('[aria-hidden="true"]');
        expect(innerAccent).not.toBeNull();
        expect(innerAccent!.className).toContain('pointer-events-none');
        expect(innerAccent!.className).toContain('radial-gradient');
    });

    it('ships the German translation too', async () => {
        await i18n.changeLanguage('de');
        renderLanding();
        expect(screen.getByText(/Für FACEIT-Matches entwickelt/i)).toBeInTheDocument();
        expect(screen.getByText('WAS IST KASPABATTLE?')).toBeInTheDocument();
    });
});

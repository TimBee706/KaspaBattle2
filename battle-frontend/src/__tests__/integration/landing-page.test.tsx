import type { ReactNode } from 'react';
import { describe, it, expect, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import { MemoryRouter } from 'react-router-dom';
import { LandingPage } from '../../pages/LandingPage';
import { publicLinks } from '../../config/publicLinks';

vi.mock('react-i18next', () => ({
    useTranslation: () => ({ t: (key: string) => key }),
    Trans: ({ children }: { children: ReactNode }) => <>{children}</>,
}));

// LandingPage renders at "/" for a fresh visitor with no wallet connected —
// useAuthStore's default (unauthenticated) state must be enough on its own.
describe('LandingPage (no wallet connected)', () => {
    it('renders without crashing and without any wallet/auth setup', () => {
        render(
            <MemoryRouter initialEntries={['/']}>
                <LandingPage />
            </MemoryRouter>
        );
        expect(screen.getByRole('heading', { level: 1 })).toBeInTheDocument();
    });

    it('primary CTA ("Enter Testnet") links to the lobby', () => {
        render(
            <MemoryRouter initialEntries={['/']}>
                <LandingPage />
            </MemoryRouter>
        );
        const cta = screen.getByRole('link', { name: 'hero.cta_primary' });
        expect(cta).toHaveAttribute('href', '/lobby');
    });

    it('secondary CTA ("Become a Tester") scrolls to the recruitment section', () => {
        render(
            <MemoryRouter initialEntries={['/']}>
                <LandingPage />
            </MemoryRouter>
        );
        const cta = screen.getByRole('link', { name: 'hero.cta_secondary' });
        expect(cta).toHaveAttribute('href', '#get-involved');
        expect(document.getElementById('get-involved')).toBeInTheDocument();
    });

    it('tertiary CTA ("View on GitHub") opens the verified repository in a new tab', () => {
        render(
            <MemoryRouter initialEntries={['/']}>
                <LandingPage />
            </MemoryRouter>
        );
        const cta = screen.getByRole('link', { name: 'hero.cta_github' });
        expect(cta).toHaveAttribute('href', publicLinks.githubRepository);
        expect(cta).toHaveAttribute('target', '_blank');
        expect(cta).toHaveAttribute('rel', 'noopener noreferrer');
    });
});

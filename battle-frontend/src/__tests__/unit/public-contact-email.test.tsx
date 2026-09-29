import { describe, it, expect, beforeEach, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import { MemoryRouter } from 'react-router-dom';

// PUBLIC_CONTACT_EMAIL is read once at module-load time from
// import.meta.env.VITE_PUBLIC_CONTACT_EMAIL (same caveat as
// unit/contact-card.test.tsx), so each case stubs the env and re-imports the
// relevant module fresh via vi.resetModules().
describe('info@kaspabattle.com is centrally configured (no hardcoded duplicates)', () => {
    beforeEach(() => {
        vi.resetModules();
        vi.unstubAllEnvs();
        vi.stubEnv('VITE_PUBLIC_CONTACT_EMAIL', 'info@kaspabattle.com');
    });

    it('Footer renders a mailto: link using the configured address', async () => {
        const { Footer } = await import('../../components/layout/Footer');
        render(
            <MemoryRouter>
                <Footer />
            </MemoryRouter>
        );
        const link = screen.getByText('info@kaspabattle.com').closest('a');
        expect(link).toHaveAttribute('href', 'mailto:info@kaspabattle.com');
    });

    it('SupportPage renders a mailto: link using the configured address (no hardcoded fallback)', async () => {
        // SupportPage reads i18n.language directly, so the real i18n singleton
        // must be (re-)initialized in this fresh module registry before render —
        // unlike Footer above, which only calls t() and doesn't need this.
        await import('../../i18n');
        const { SupportPage } = await import('../../pages/SupportPage');
        render(<SupportPage />);
        const links = screen.getAllByText('info@kaspabattle.com');
        expect(links.length).toBeGreaterThan(0);
        links.forEach((el) => {
            expect(el.closest('a')).toHaveAttribute('href', 'mailto:info@kaspabattle.com');
        });
    });
});

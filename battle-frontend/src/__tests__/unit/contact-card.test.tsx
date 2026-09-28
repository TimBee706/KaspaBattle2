import { describe, it, expect, beforeEach, vi } from 'vitest';
import { render, screen } from '@testing-library/react';

vi.mock('react-i18next', () => ({
    useTranslation: () => ({ t: (key: string) => key }),
}));

// PUBLIC_CONTACT_EMAIL is read once at module-load time from
// import.meta.env.VITE_PUBLIC_CONTACT_EMAIL, so each case stubs the env and
// re-imports the module fresh via vi.resetModules() rather than relying on
// a value captured before the stub was applied.
describe('ContactCard', () => {
    beforeEach(() => {
        vi.resetModules();
        vi.unstubAllEnvs();
    });

    it('renders a mailto: link when VITE_PUBLIC_CONTACT_EMAIL is set', async () => {
        vi.stubEnv('VITE_PUBLIC_CONTACT_EMAIL', 'info@kaspabattle.com');
        const { ContactCard } = await import('../../components/common/ContactCard');
        render(<ContactCard />);

        const link = screen.getByRole('link');
        expect(link).toHaveAttribute('href', 'mailto:info@kaspabattle.com');
    });

    it('falls back to a GitHub "new issue" link when no email is configured', async () => {
        vi.stubEnv('VITE_PUBLIC_CONTACT_EMAIL', '');
        const { ContactCard } = await import('../../components/common/ContactCard');
        render(<ContactCard />);

        const link = screen.getByRole('link');
        expect(link.getAttribute('href')).toContain('github.com/TimBee706/KaspaBattle2/issues');
        expect(link).toHaveAttribute('target', '_blank');
        expect(link).toHaveAttribute('rel', 'noopener noreferrer');
    });
});

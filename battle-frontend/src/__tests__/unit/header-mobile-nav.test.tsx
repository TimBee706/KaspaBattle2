import { describe, it, expect, beforeEach } from 'vitest';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { MemoryRouter } from 'react-router-dom';
import i18n from '../../i18n';
import { Header } from '../../components/layout/Header';
import { publicLinks } from '../../config/publicLinks';

// Uses the real i18n instance, same as unit/i18n.test.tsx.
describe('Header mobile navigation', () => {
    beforeEach(async () => {
        await i18n.changeLanguage('en');
    });

    it('includes a GitHub link and a link to the recruitment section', async () => {
        const user = userEvent.setup();
        render(
            <MemoryRouter>
                <Header />
            </MemoryRouter>
        );

        await user.click(screen.getByLabelText('Toggle menu'));

        // Exact name match: the desktop icon-only GitHub link's accessible
        // name is "View on GitHub" (aria-label); only the mobile dropdown's
        // link has visible text that resolves to exactly "GitHub".
        const githubLink = await screen.findByRole('link', { name: 'GitHub' });
        expect(githubLink).toHaveAttribute('href', publicLinks.githubRepository);
        expect(githubLink).toHaveAttribute('target', '_blank');
        expect(githubLink).toHaveAttribute('rel', 'noopener noreferrer');

        const recruitLink = screen.getByRole('link', { name: 'Help build KaspaBattle' });
        expect(recruitLink).toHaveAttribute('href', '/#get-involved');
    });
});

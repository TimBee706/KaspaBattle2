import { describe, it, expect, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import { SocialLinks } from '../../components/common/SocialLinks';
import { publicLinks } from '../../config/publicLinks';

vi.mock('react-i18next', () => ({
    useTranslation: () => ({ t: (key: string) => key }),
}));

describe('SocialLinks', () => {
    it('renders X, Instagram and GitHub with safe external link attributes', () => {
        render(<SocialLinks />);
        const links = screen.getAllByRole('link');

        expect(links.length).toBeGreaterThanOrEqual(3);
        for (const link of links) {
            expect(link).toHaveAttribute('target', '_blank');
            expect(link).toHaveAttribute('rel', 'noopener noreferrer');
        }
    });

    it('points the GitHub icon at the verified repository URL', () => {
        render(<SocialLinks />);
        const githubLink = screen.getByRole('link', { name: 'social.github' });
        expect(githubLink).toHaveAttribute('href', publicLinks.githubRepository);
    });

    it('does not render Reddit, since it could not be independently verified', () => {
        render(<SocialLinks />);
        const links = screen.getAllByRole('link');
        expect(links.some((l) => l.getAttribute('href') === publicLinks.reddit)).toBe(false);
    });
});

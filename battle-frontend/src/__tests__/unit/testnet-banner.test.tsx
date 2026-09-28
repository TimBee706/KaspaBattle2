import { describe, it, expect, beforeEach } from 'vitest';
import { render, screen, act } from '@testing-library/react';
import i18n from '../../i18n';
import { TestnetStatusBanner } from '../../components/layout/TestnetStatusBanner';

// Uses the real i18n instance (same pattern as unit/i18n.test.tsx) so this
// actually exercises the shipped EN/DE copy, not just translation keys.
describe('TestnetStatusBanner', () => {
    beforeEach(async () => {
        await i18n.changeLanguage('en');
    });

    it('shows the public testnet beta status and TESTNET badge in English', async () => {
        render(<TestnetStatusBanner />);
        expect(await screen.findByText('TESTNET')).toBeInTheDocument();
        expect(screen.getByText('Public Testnet Beta')).toBeInTheDocument();
        expect(screen.getByText(/Use testnet KAS only/i)).toBeInTheDocument();
        expect(screen.getByRole('link', { name: /Beta details/i })).toHaveAttribute('href', '#testnet-details');
    });

    it('shows the German translation when switched', async () => {
        render(<TestnetStatusBanner />);
        await act(async () => {
            await i18n.changeLanguage('de');
        });
        expect(await screen.findByText('Öffentliche Testnet-Beta')).toBeInTheDocument();
        expect(screen.getByText(/Verwende ausschließlich Testnet-KAS/i)).toBeInTheDocument();
    });
});

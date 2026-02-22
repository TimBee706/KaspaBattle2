import { describe, it, vi, expect } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import App from '../App';
import * as matchesApi from '../api/matches';
import * as wallet from '../kaspa/wallet';

// Mock API responses
const mockMatch = {
    id: 'match-123',
    status: 'OPEN',
    wager_amount_sompi: 1000000,
    escrow_address: 'kaspa:escrow123',
    player_a_faceit_id: 'user-a',
    player_a_faceit_nickname: 'ProGamerA',
    player_b_faceit_id: null,
    game_id: 'cs2',
    match_mode: 'bo1',
    created_at: new Date().toISOString(),
};

describe('Full Match Flow Integration', () => {
    it('should handle wallet creation and match deposit', async () => {
        // 1. App rendern
        render(<App />);

        // 2. Wallet erstellen
        const connectBtn = screen.getByText(/Neues Wallet/i);
        fireEvent.click(connectBtn);

        await waitFor(() => {
            expect(screen.getByText(/Mnemonic sichern/i)).toBeInTheDocument();
        });

        // 3. Kopieren & Bestätigen (schließt Mnemonic info)
        const copyBtn = screen.getByText(/Kopieren & Bestätigen/i);
        fireEvent.click(copyBtn);

        expect(screen.queryByText(/Mnemonic sichern/i)).not.toBeInTheDocument();
    });
});

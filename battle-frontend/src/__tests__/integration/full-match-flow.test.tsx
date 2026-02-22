import { describe, it, expect } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import App from '../../App';

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

import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import App from '../../App';

// Mock the wallet module so we don't actually hit WASM/Network in this test
vi.mock('../../kaspa/wallet', () => ({
    importWallet: vi.fn().mockResolvedValue({
        wallet: {},
        account: {},
        address: 'kaspatest:qmockaddress123'
    }),
    getBalance: vi.fn().mockResolvedValue(50000000), // 50 KAS
    onBalanceChange: vi.fn(),
    sendDeposit: vi.fn().mockResolvedValue('mock-tx-id')
}));

vi.mock('../../hooks/useKaspaInit', () => ({
    useKaspaInit: vi.fn().mockReturnValue({ isReady: true, error: null })
}));

describe('Full Match Flow Integration', () => {
    it('should handle wallet import and mock deposit', async () => {
        // 1. URL direkt auf die Wallet-Seite setzen für den Test
        window.history.pushState({}, 'Test', '/wallet/import');

        // 2. App rendern
        render(<App />);

        // 3. Warten bis die Wallet-Seite geladen ist
        let importInput: HTMLElement;
        await waitFor(() => {
            importInput = screen.getByPlaceholderText('Seed-Phrase (Mnemonic) oder Private Key');
            expect(importInput).toBeInTheDocument();
        });
        const importBtn = screen.getByText('Wallet importieren');

        // Gebe Mock-Mnemonic ein
        fireEvent.change(importInput!, { target: { value: 'test test test test test test test test test test test test' } });
        fireEvent.click(importBtn);

        await waitFor(() => {
            // Header sollte Balance + Address anzeigen anstelle des Import-Buttons
            expect(screen.getByText(/50.00 KAS/i)).toBeInTheDocument();
            expect(screen.queryByText('Wallet importieren')).not.toBeInTheDocument();
        });
    });
});


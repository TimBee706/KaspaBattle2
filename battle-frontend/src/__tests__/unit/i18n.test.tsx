import React from 'react';
import { render, screen, act } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { MemoryRouter } from 'react-router-dom';
import '@testing-library/jest-dom';

// Import initialized i18n instance to ensure configurations match
import i18n from '../../i18n';
import { Header } from '../../components/layout/Header';

describe('i18n configuration and Language Switcher', () => {
    beforeEach(() => {
        // Clear local storage to test default behavior
        localStorage.clear();
        // Reset language to English before each test explicitly (just in case)
        i18n.changeLanguage('en');
    });

    test('defaults to English', async () => {
        render(
            <MemoryRouter>
                <Header />
            </MemoryRouter>
        );
        
        // Wait for translations to load and verify English default
        const lobbyLink = await screen.findByText(/Lobby/i);
        expect(lobbyLink).toBeInTheDocument();
        
        // Verify the create challenge text is in English instead of German
        const createChallengeLink = await screen.findByText(/Create Challenge/i);
        expect(createChallengeLink).toBeInTheDocument();
        
        // Ensure "Challenge erstellen" is NOT in the document initially
        expect(screen.queryByText(/Challenge erstellen/i)).not.toBeInTheDocument();
    });

    test('switches to German when selected', async () => {
        const user = userEvent.setup();
        
        render(
            <MemoryRouter>
                <Header />
            </MemoryRouter>
        );
        
        // Make sure it starts in English
        expect(await screen.findByText(/Create Challenge/i)).toBeInTheDocument();
        
        // Find the language select component
        const languageSelect = await screen.findByRole('combobox', { name: /Language/i });
        
        // Select German from the dropdown
        await act(async () => {
            await user.selectOptions(languageSelect, 'de');
        });
        
        // Verify it switched successfully to German
        expect(await screen.findByText(/Challenge erstellen/i)).toBeInTheDocument();
        
        // Verify "Create Challenge" is gone
        expect(screen.queryByText(/Create Challenge/i)).not.toBeInTheDocument();
        
        // Verify local storage saved the preference
        expect(localStorage.getItem('preferred_language')).toBe('de');
    });
});

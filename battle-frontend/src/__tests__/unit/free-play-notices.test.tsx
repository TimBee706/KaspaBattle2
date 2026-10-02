import { describe, it, expect, beforeEach } from 'vitest';
import { render, screen } from '@testing-library/react';
import i18n from '../../i18n';
import { NodeHelpNotice } from '../../components/freeplay/NodeHelpNotice';
import { TestnetComingSoon } from '../../components/common/TestnetComingSoon';

describe('Free Play notices', () => {
    beforeEach(async () => {
        await i18n.changeLanguage('de');
    });

    it('landing notice announces the beta and links to the contact mailbox', async () => {
        render(<NodeHelpNotice />);
        const box = await screen.findByTestId('node-help-notice');
        expect(box.textContent).toMatch(/öffentlichen Beta/);
        expect(box.textContent).toMatch(/Free-Play/);
        expect(box.textContent).not.toMatch(/Kaspar|kaputt/i);
        const link = box.querySelector('a[href^="mailto:"]') as HTMLAnchorElement;
        expect(link.getAttribute('href')).toBe('mailto:info@kaspabattle.com');
    });

    it('testnet status is informational and non-blocking', async () => {
        render(<TestnetComingSoon />);
        const note = await screen.findByRole('note');
        expect(note.textContent).toContain('Kaspa-Testnet-Modus – demnächst verfügbar');
        expect(note.querySelector('button')).toBeNull();
    });
});

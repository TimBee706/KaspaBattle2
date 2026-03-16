import { describe, it, expect } from 'vitest';
import { sompiToKas, kasToSompi, formatKas, shortenAddress } from '../../utils/format';

describe('Format Utils', () => {
    it('should convert sompi to KAS correctly', () => {
        expect(sompiToKas(100_000_000)).toBe(1);
        expect(sompiToKas(250_000_000)).toBe(2.5);
    });

    it('should convert KAS to sompi correctly', () => {
        expect(kasToSompi(1)).toBe(100_000_000);
        expect(kasToSompi(2.5)).toBe(250_000_000);
    });

    it('should format KAS with decimals', () => {
        expect(formatKas(100_000_000)).toBe('1,00');
        expect(formatKas(255_000_000)).toBe('2,55');
    });

    it('should shorten Kaspa addresses with empty or invalid address cases', () => {
        const addr = 'kaspa:qz2ptjk67k2twpvhcqx2fpe3n24xklngrpsatdq4c4l5czll';
        // 'kaspa:' (6) + 'qz2p' (4) = 10 prefixLen => 'kaspa:qz2p'
        expect(shortenAddress(addr, 10, 4)).toBe('kaspa:qz2p...czll');
        expect(shortenAddress('too-short', 10, 4)).toBe('too-short');
        expect(shortenAddress(null)).toBe('—');
        expect(shortenAddress(undefined)).toBe('—');
        expect(shortenAddress('')).toBe('—');
    });
});

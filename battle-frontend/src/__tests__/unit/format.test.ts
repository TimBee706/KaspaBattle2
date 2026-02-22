import { describe, it, expect } from 'vitest';
import { sompiToKas, kasToSompi, formatKas, shortenAddress } from '../utils/format';

describe('Format Utils', () => {
    it('should convert sompi to KAS correctly', () => {
        expect(sompiToKas(100_000)).toBe(1);
        expect(sompiToKas(250_000)).toBe(2.5);
    });

    it('should convert KAS to sompi correctly', () => {
        expect(kasToSompi(1)).toBe(100_000);
        expect(kasToSompi(2.5)).toBe(250_000);
    });

    it('should format KAS with decimals', () => {
        expect(formatKas(100_000)).toBe('1,00');
        expect(formatKas(2_550_000)).toBe('25,50');
    });

    it('should shorten Kaspa addresses', () => {
        const addr = 'kaspa:qz2ptjk67k2twpvhcqx2fpe3n24xklngrpsatdq4c4l5czll';
        expect(shortenAddress(addr, 6, 4)).toBe('kaspa:...zlll');
        expect(shortenAddress('too-short')).toBe('too-short');
    });
});

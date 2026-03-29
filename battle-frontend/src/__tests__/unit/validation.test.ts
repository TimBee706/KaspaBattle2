import { describe, it, expect } from 'vitest';
import { validateWagerAmount, validateKaspaAddress, validateFaceitMatchId } from '../../utils/validation';

describe('Validation Utils', () => {
    it('should validate wager amounts', () => {
        expect(validateWagerAmount(10).valid).toBe(true);
        expect(validateWagerAmount(5).valid).toBe(false); // Min 10
        expect(validateWagerAmount(10001).valid).toBe(false); // Max 10000
        expect(validateWagerAmount(NaN).valid).toBe(false);
    });

    it('should validate Kaspa addresses', () => {
        expect(validateKaspaAddress('kaspa:qz2ptjk67k2twpvhcqx2fpe3n24xklngrpsatdq4c4l5czll')).toBe(true);
        expect(validateKaspaAddress('invalid-address')).toBe(false);
        expect(validateKaspaAddress('bitcoin:1BvBMSEYstWetqTFn5Au4m4GFg7xJaNVN2')).toBe(false);
    });

    it('should validate FaceIT match IDs as UUIDs', () => {
        expect(validateFaceitMatchId('550e8400-e29b-41d4-a716-446655440000').valid).toBe(true);
        expect(validateFaceitMatchId('')).toEqual({
            valid: false,
            error: 'Please enter a FACEIT match ID.',
        });
        expect(validateFaceitMatchId('not-a-uuid')).toEqual({
            valid: false,
            error: 'Please enter a valid FACEIT match ID in UUID format.',
        });
    });
});

import { describe, it, expect } from 'vitest';
import { safeNextPath } from './safeNextPath';

describe('safeNextPath', () => {
    it('accepts same-origin paths', () => {
        expect(safeNextPath('/free-play/abc')).toBe('/free-play/abc');
    });
    it('rejects open redirects', () => {
        for (const bad of ['//evil.com', '/\\evil.com', 'https://evil.com', 'javascript:1', null]) {
            expect(safeNextPath(bad as string | null)).toBe('/free-play');
        }
    });
});

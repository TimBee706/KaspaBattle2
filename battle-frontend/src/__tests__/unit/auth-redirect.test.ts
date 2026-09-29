import { describe, it, expect, vi, beforeEach } from 'vitest';
import { handleAuthRedirect, resetAuthRedirectGuard } from '../../auth/authRedirect';

function deps(over: Partial<Parameters<typeof handleAuthRedirect>[2]> = {}) {
    return {
        fetchUser: vi.fn().mockResolvedValue(undefined),
        fetchFaceitStatus: vi.fn().mockResolvedValue({ connected: true }),
        replaceUrl: vi.fn(),
        notifyError: vi.fn(),
        ...over,
    };
}

describe('handleAuthRedirect', () => {
    beforeEach(() => resetAuthRedirectGuard());

    it('does nothing without linked/error flags', async () => {
        const d = deps();
        expect(await handleAuthRedirect('', '/lobby', d)).toBe('none');
        expect(d.fetchUser).not.toHaveBeenCalled();
    });

    it('reloads /auth/me and /faceit/status after ?linked=1 and cleans the URL', async () => {
        const d = deps();
        expect(await handleAuthRedirect('?linked=1', '/lobby', d)).toBe('linked');
        expect(d.fetchUser).toHaveBeenCalledTimes(1);
        expect(d.fetchFaceitStatus).toHaveBeenCalledTimes(1);
        expect(d.replaceUrl).toHaveBeenCalledWith('/lobby');
    });

    it('runs only once even if invoked twice (React StrictMode)', async () => {
        const d = deps();
        await handleAuthRedirect('?linked=1', '/lobby', d);
        await handleAuthRedirect('?linked=1', '/lobby', d);
        expect(d.fetchUser).toHaveBeenCalledTimes(1);
    });

    it('does not claim success when the session cannot be loaded', async () => {
        const d = deps({ fetchUser: vi.fn().mockRejectedValue(new Error('401')) });
        expect(await handleAuthRedirect('?linked=1', '/lobby', d)).toBe('error');
        expect(d.notifyError).toHaveBeenCalledWith('session_not_established');
        expect(d.replaceUrl).toHaveBeenCalledWith('/');
    });

    it('surfaces backend error codes', async () => {
        const d = deps();
        expect(await handleAuthRedirect('?error=faceit_link_failed', '/', d)).toBe('error');
        expect(d.notifyError).toHaveBeenCalledWith('faceit_link_failed');
        expect(d.fetchUser).not.toHaveBeenCalled();
    });
});
